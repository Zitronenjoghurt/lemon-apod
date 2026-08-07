//! End to end coverage for `apod.db` against the real migrations in `core/migrations/`.
//!
//! The point of running these against the migrations rather than a schema written out by hand
//! in the test is that a hand-written copy drifts. The previous one had lost both indexes, two
//! of the three FTS triggers, the `meta` table and the `entry_media` foreign key, so the API's
//! tests were passing against a database the archiver never actually produces.

#![cfg(feature = "data-write")]

use apod_core::apod::{Filters, Order, SCHEMA_VERSION, Snippet};
use apod_core::db::DbConfig;
use apod_core::{ApodDate, ApodEntry, ApodReader, ApodWriter, Credit, Media, MediaKind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

fn temp_db() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "apod-db-it-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("apod.db")
}

fn entry(date: &str, title: &str, explanation: &str) -> ApodEntry {
    let date: ApodDate = date.parse().unwrap();
    ApodEntry {
        date,
        title: title.into(),
        title_raw: Some(format!("APOD: {title}")),
        explanation_html: format!("<b>{explanation}</b>"),
        explanation_text: explanation.into(),
        credits: vec![Credit {
            role: "Image Credit & Copyright".into(),
            html: "Jane Doe".into(),
            text: "Jane Doe".into(),
        }],
        has_copyright: true,
        license_url: None,
        tomorrow_teaser: Some("open water".into()),
        keywords: vec!["nebula".into()],
        media: Media::new(
            MediaKind::ImageJpg,
            Some("https://apod.nasa.gov/apod/image/x.jpg".into()),
            None,
        ),
        extra_media: Vec::new(),
        source_url: date.source_url(),
    }
}

async fn seeded(rows: &[(&str, &str, &str)]) -> (ApodWriter, PathBuf) {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();
    let entries: Vec<_> = rows
        .iter()
        .map(|(date, title, explanation)| entry(date, title, explanation))
        .collect();
    writer.upsert_all(&entries).await.unwrap();
    (writer, path)
}

/// The conformance pass: run every read query once against the real schema. It does not
/// assert on the data, it asserts that the SQL and the migrations still agree. A renamed or
/// dropped column fails here rather than in production.
#[tokio::test]
async fn every_read_query_matches_the_migrated_schema() {
    let (writer, path) = seeded(&[("2024-03-05", "Saturn", "The ringed planet.")]).await;
    let reader = writer.reader();

    let date = "2024-03-05".parse::<ApodDate>().unwrap();
    let filters = Filters {
        from: Some(date),
        to: Some(date),
        kind: Some(MediaKind::ImageJpg),
        copyright: Some(true),
    };

    reader.entry(date).await.unwrap();
    reader.latest().await.unwrap();
    reader.random(None).await.unwrap();
    reader.random(Some(MediaKind::ImageJpg)).await.unwrap();
    reader
        .list(&filters, Some(date), 10, Order::Asc)
        .await
        .unwrap();
    reader.list(&filters, None, 10, Order::Desc).await.unwrap();
    reader.on_this_day(3, 5).await.unwrap();
    reader
        .search("saturn", &filters, false, 0, 10, 32)
        .await
        .unwrap();
    reader
        .search("saturn", &filters, true, 0, 10, 32)
        .await
        .unwrap();
    reader.stats().await.unwrap();
    reader.all_dates().await.unwrap();
    reader.count().await.unwrap();
    reader.thumb_count().await.unwrap();

    writer.stale_dates().await.unwrap();
    writer.missing_thumbs().await.unwrap();
    writer.media_for(&[date]).await.unwrap();
    writer.set_thumb(date, Some("x.webp")).await.unwrap();

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn migrations_stamp_the_version_readers_check_for() {
    let (writer, path) = seeded(&[]).await;
    assert_eq!(
        writer.reader().db().applied_version().await.unwrap(),
        Some(SCHEMA_VERSION)
    );

    // A reader opening the same file must accept it.
    ApodReader::open(DbConfig::read_only(&path)).await.unwrap();

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn a_reader_refuses_a_database_that_was_never_migrated() {
    let path = temp_db();
    // A file with the tables but no migration history: what a hand-rolled copy looks like.
    let db = apod_core::Db::open(DbConfig::read_write(&path))
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE entries (date_id INTEGER PRIMARY KEY)")
        .execute(db.writer().unwrap())
        .await
        .unwrap();
    db.close().await;

    let error = ApodReader::open(DbConfig::read_only(&path))
        .await
        .unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains("never been migrated"),
        "expected a startup-time schema complaint, got: {message}"
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn roundtrips_an_entry_including_extra_media() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    let mut original = entry("2024-03-05", "Crab Nebula", "A supernova remnant.");
    original.extra_media = vec![Media::new(
        MediaKind::ImagePng,
        Some("https://apod.nasa.gov/apod/image/y.png".into()),
        None,
    )];
    writer.upsert(&original).await.unwrap();

    let loaded = writer.reader().entry(original.date).await.unwrap().unwrap();
    assert_eq!(loaded.title, "Crab Nebula");
    assert_eq!(loaded.keywords, vec!["nebula"]);
    assert!(loaded.has_copyright);
    assert_eq!(loaded.credits, original.credits);
    assert_eq!(loaded.extra_media.len(), 1);
    assert_eq!(loaded.extra_media[0].kind, MediaKind::ImagePng);

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn credit_text_indexes_every_role_but_stores_them_apart() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    let mut original = entry("2024-03-05", "Crab Nebula", "A supernova remnant.");
    original.credits.push(Credit {
        role: "Text".into(),
        html: "Ada Lovelace".into(),
        text: "Ada Lovelace".into(),
    });
    writer.upsert(&original).await.unwrap();

    // Searching a name from the second credit has to hit, which only works if credit_text
    // concatenated both roles.
    let hits = writer
        .reader()
        .search("lovelace", &Filters::default(), false, 0, 10, 32)
        .await
        .unwrap();
    assert_eq!(hits.total, 1, "every credited name should be indexed");

    let loaded = writer.reader().entry(original.date).await.unwrap().unwrap();
    assert_eq!(loaded.credits.len(), 2);
    assert_eq!(loaded.credits[1].role, "Text");

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn reparsing_preserves_thumbnails() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    let mut original = entry("2024-03-05", "Crab Nebula", "A supernova remnant.");
    writer.upsert(&original).await.unwrap();
    writer
        .set_thumb(original.date, Some("2024/03/2024-03-05.webp"))
        .await
        .unwrap();

    original.title = "Crab Nebula, Corrected".into();
    writer.upsert(&original).await.unwrap();

    let loaded = writer.reader().entry(original.date).await.unwrap().unwrap();
    assert_eq!(loaded.title, "Crab Nebula, Corrected");
    assert_eq!(
        loaded.media.thumb_path.as_deref(),
        Some("2024/03/2024-03-05.webp")
    );

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// The `entries_au` trigger. This is one of the two triggers the hand-copied schema had lost,
/// so nothing was checking it from the read side.
#[tokio::test]
async fn updating_an_entry_drops_its_old_text_from_the_index() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    let mut original = entry("2024-03-05", "Crab Nebula", "A supernova remnant.");
    writer.upsert(&original).await.unwrap();

    original.explanation_text = "Now about galaxies instead.".into();
    writer.upsert(&original).await.unwrap();

    let stale = writer
        .reader()
        .search("remnant", &Filters::default(), false, 0, 10, 32)
        .await
        .unwrap();
    assert_eq!(stale.total, 0, "the old explanation should no longer match");

    let fresh = writer
        .reader()
        .search("galaxies", &Filters::default(), false, 0, 10, 32)
        .await
        .unwrap();
    assert_eq!(fresh.total, 1);

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn search_returns_the_rows_it_counts() {
    let (writer, path) = seeded(&[
        (
            "2024-03-05",
            "Saturn at Opposition",
            "The ringed planet is close.",
        ),
        (
            "2024-03-06",
            "Orion Rising",
            "Nothing about the ringed planet here.",
        ),
        (
            "2024-03-07",
            "A Distant Galaxy",
            "Saturn is not in this one either.",
        ),
    ])
    .await;

    let results = writer
        .reader()
        .search("saturn", &Filters::default(), false, 0, 30, 32)
        .await
        .unwrap();

    assert_eq!(results.total, 2);
    assert_eq!(
        results.items.len(),
        2,
        "items must match the reported total"
    );
    assert!(
        results
            .items
            .iter()
            .any(|hit| hit.entry.title == "Saturn at Opposition")
    );

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn search_marks_the_match_in_the_snippet() {
    let (writer, path) = seeded(&[(
        "2024-03-05",
        "Orion",
        "A nebula in the constellation Orion.",
    )])
    .await;

    let html = writer
        .reader()
        .search("nebula", &Filters::default(), false, 0, 30, 32)
        .await
        .unwrap();
    assert!(
        html.items[0].snippet.contains("<mark>nebula</mark>"),
        "{}",
        html.items[0].snippet
    );

    // The same query through a consumer that is not a browser.
    let discord = ApodReader::open(DbConfig::read_only(&path))
        .await
        .unwrap()
        .with_snippet(Snippet::Delimited {
            open: "**".into(),
            close: "**".into(),
        });
    let bold = discord
        .search("nebula", &Filters::default(), false, 0, 30, 32)
        .await
        .unwrap();
    assert!(
        bold.items[0].snippet.contains("**nebula**"),
        "{}",
        bold.items[0].snippet
    );
    assert!(!bold.items[0].snippet.contains("<mark>"));

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn search_honours_filters_and_paging() {
    let (writer, path) = seeded(&[
        ("2024-03-05", "Saturn One", "ringed"),
        ("2024-03-06", "Saturn Two", "ringed"),
        ("2025-03-05", "Saturn Three", "ringed"),
    ])
    .await;

    let filters = Filters {
        to: Some("2024-12-31".parse().unwrap()),
        ..Filters::default()
    };
    let results = writer
        .reader()
        .search("saturn", &filters, true, 0, 30, 32)
        .await
        .unwrap();
    assert_eq!(results.total, 2);

    let page = writer
        .reader()
        .search("saturn", &filters, true, 1, 1, 32)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.total, 2,
        "total describes the whole result set, not the page"
    );

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn listing_pages_forward_on_a_date_cursor() {
    let (writer, path) = seeded(&[
        ("2024-03-05", "One", "a"),
        ("2024-03-06", "Two", "b"),
        ("2024-03-07", "Three", "c"),
    ])
    .await;

    let first = writer
        .reader()
        .list(&Filters::default(), None, 2, Order::Desc)
        .await
        .unwrap();
    assert_eq!(first.items.len(), 2);
    assert_eq!(first.items[0].date.to_string(), "2024-03-07");
    assert_eq!(first.next_cursor.unwrap().to_string(), "2024-03-05");

    let second = writer
        .reader()
        .list(&Filters::default(), first.next_cursor, 2, Order::Desc)
        .await
        .unwrap();
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].date.to_string(), "2024-03-05");
    assert!(second.next_cursor.is_none(), "last page has no cursor");

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// The bug the old split could not catch: whether `thumb_url` held a URL or a path depended on
/// which crate did the read. Now the path is always the path, and the URL only appears for a
/// reader that was told what it serves them under.
#[tokio::test]
async fn a_thumbnail_is_a_path_until_a_reader_is_told_how_to_serve_it() {
    let (writer, path) = seeded(&[("2024-03-05", "One", "a")]).await;
    let date: ApodDate = "2024-03-05".parse().unwrap();
    writer
        .set_thumb(date, Some(&date.thumb_path()))
        .await
        .unwrap();

    let bare = writer.reader().entry(date).await.unwrap().unwrap();
    assert_eq!(
        bare.media.thumb_path.as_deref(),
        Some("2024/03/2024-03-05.webp")
    );
    assert_eq!(bare.media.thumb_url, None, "no base, no URL");

    let serving = ApodReader::open(DbConfig::read_only(&path))
        .await
        .unwrap()
        .with_thumb_base("/thumbs/");
    let served = serving.entry(date).await.unwrap().unwrap();
    assert_eq!(
        served.media.thumb_url.as_deref(),
        Some("/thumbs/2024/03/2024-03-05.webp")
    );
    assert_eq!(
        served.media.thumb_path.as_deref(),
        Some("2024/03/2024-03-05.webp"),
        "the path stays available even when a URL was built from it"
    );

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn tracks_missing_thumbnails() {
    let (writer, path) = seeded(&[("2024-03-05", "One", "a"), ("2024-03-06", "Two", "b")]).await;
    assert_eq!(writer.missing_thumbs().await.unwrap().len(), 2);

    writer
        .set_thumb("2024-03-05".parse().unwrap(), Some("x.webp"))
        .await
        .unwrap();
    assert_eq!(writer.missing_thumbs().await.unwrap().len(), 1);
    assert_eq!(writer.reader().thumb_count().await.unwrap(), 1);

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// `entry_media` cascades on delete, which the hand-copied schema had dropped the foreign key
/// for. Enforcement also needs `PRAGMA foreign_keys = ON`, which the Db primitive sets.
#[tokio::test]
async fn deleting_an_entry_takes_its_extra_media_with_it() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    let mut original = entry("2024-03-05", "Multi", "two images");
    original.extra_media = vec![Media::new(
        MediaKind::ImagePng,
        Some("https://apod.nasa.gov/apod/image/y.png".into()),
        None,
    )];
    writer.upsert(&original).await.unwrap();

    let db = writer.reader().db();
    sqlx::query("DELETE FROM entries WHERE date_id = ?1")
        .bind(original.date.days())
        .execute(db.writer().unwrap())
        .await
        .unwrap();

    let orphans: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entry_media")
        .fetch_one(db.reader())
        .await
        .unwrap();
    assert_eq!(orphans, 0, "extra media outlived its entry");

    db.close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
