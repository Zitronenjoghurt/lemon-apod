#![cfg(feature = "data-write")]

use apod_core::apod::{
    Changed, Filters, Order, PictureFilters, PictureOrder, ResourceFilters, ResourceOrder,
    SCHEMA_VERSION, Snippet, WordFilters, WordOrder,
};
use apod_core::db::DbConfig;
use apod_core::{
    ApodDate, ApodEntry, ApodReader, ApodWriter, Credit, KindFilter, Media, MediaKind, Provenance,
    Thumb,
};
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
        legacy_media_url: None,
        alt: None,
        authors: Vec::new(),
        provenance: Provenance::LegacyOnly,
        source_url: date.source_url(),
        picture: None,
    }
}

async fn seeded(rows: &[(&str, &str, &str)]) -> (ApodWriter, PathBuf) {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();
    let entries: Vec<apod_core::Merged> = rows
        .iter()
        .map(|(date, title, explanation)| entry(date, title, explanation).into())
        .collect();
    writer.upsert_all(&entries).await.unwrap();
    (writer, path)
}

#[tokio::test]
async fn every_read_query_matches_the_migrated_schema() {
    let (writer, path) = seeded(&[("2024-03-05", "Saturn", "The ringed planet.")]).await;
    let reader = writer.reader();

    let date = "2024-03-05".parse::<ApodDate>().unwrap();
    let filters = Filters {
        from: Some(date),
        to: Some(date),
        kind: Some(MediaKind::ImageJpg.into()),
        copyright: Some(true),
        lost: Some(false),
    };

    reader.entry(date).await.unwrap();
    reader.latest().await.unwrap();
    reader.random(None).await.unwrap();
    reader
        .random(Some(&MediaKind::ImageJpg.into()))
        .await
        .unwrap();
    reader
        .random(Some(&"video".parse().unwrap()))
        .await
        .unwrap();
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

    reader.text_summary().await.unwrap();
    reader.resource_summary().await.unwrap();
    reader.picture_summary().await.unwrap();
    reader.picture_dates(date).await.unwrap();
    reader.picture_pool(None).await.unwrap();
    reader.picture_pool(Some(date)).await.unwrap();
    reader.text_pool(None).await.unwrap();
    reader.text_pool(Some(date)).await.unwrap();
    reader.summaries(&[date]).await.unwrap();
    reader.given_words().await.unwrap();
    reader.timeline().await.unwrap();
    reader.coverage().await.unwrap();
    reader
        .resources(
            &ResourceFilters {
                query: Some("wikipedia".into()),
                host: Some("en.wikipedia.org".into()),
                min_refs: Some(1),
                credited: Some(false),
            },
            ResourceOrder::Refs,
            Order::Desc,
            0,
            10,
        )
        .await
        .unwrap();
    reader
        .resources(
            &ResourceFilters::default(),
            ResourceOrder::Address,
            Order::Asc,
            0,
            10,
        )
        .await
        .unwrap();
    reader.resource(1, 0, 10).await.unwrap();
    reader.resource_hosts(10).await.unwrap();
    reader
        .words(
            &WordFilters {
                query: Some("ring*".into()),
                min_total: Some(1),
                max_total: Some(100),
            },
            WordOrder::Total,
            Order::Desc,
            0,
            10,
        )
        .await
        .unwrap();
    reader
        .words(
            &WordFilters::default(),
            WordOrder::Alphabetical,
            Order::Asc,
            0,
            10,
        )
        .await
        .unwrap();
    reader.word("ringed", 5).await.unwrap();

    writer.stale_dates().await.unwrap();
    writer.missing_thumbs().await.unwrap();
    writer.unmeasured_thumbs().await.unwrap();
    writer.unhashed_thumbs().await.unwrap();
    writer.media_for(&[date]).await.unwrap();
    writer
        .set_thumb(date, Some(&Thumb::sized("x.webp", 480, 320)))
        .await
        .unwrap();
    writer.set_phash(date, Some(&[0; 32])).await.unwrap();
    writer.fingerprints().await.unwrap();
    writer.regroup_pictures().await.unwrap();

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

    ApodReader::open(DbConfig::read_only(&path)).await.unwrap();

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn a_reader_refuses_a_database_that_was_never_migrated() {
    let path = temp_db();
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
    writer.upsert_sole(&original).await.unwrap();

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
    writer.upsert_sole(&original).await.unwrap();

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
    writer.upsert_sole(&original).await.unwrap();
    writer
        .set_thumb(
            original.date,
            Some(&Thumb::sized("2024/03/2024-03-05.webp", 480, 300)),
        )
        .await
        .unwrap();

    original.title = "Crab Nebula, Corrected".into();
    writer.upsert_sole(&original).await.unwrap();

    let loaded = writer.reader().entry(original.date).await.unwrap().unwrap();
    assert_eq!(loaded.title, "Crab Nebula, Corrected");
    assert_eq!(
        loaded.media.thumb_path.as_deref(),
        Some("2024/03/2024-03-05.webp")
    );

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn updating_an_entry_drops_its_old_text_from_the_index() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    let mut original = entry("2024-03-05", "Crab Nebula", "A supernova remnant.");
    writer.upsert_sole(&original).await.unwrap();

    original.explanation_text = "Now about galaxies instead.".into();
    writer.upsert_sole(&original).await.unwrap();

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
async fn search_says_when_the_hit_was_somewhere_the_snippet_cannot_show() {
    // The fixture credits "Jane Doe" and keeps "nebula" in its keywords, neither of which the
    // explanation snippet can mark.
    let (writer, path) = seeded(&[("2024-03-05", "Saturn", "The ringed planet.")]).await;
    let reader = writer.reader();

    let by_credit = reader
        .search("Doe", &Filters::default(), false, 0, 30, 32)
        .await
        .unwrap();
    let hit = &by_credit.items[0];
    assert!(hit.matched.credit, "the query only appears in the credit");
    assert!(!hit.matched.explanation && !hit.matched.title);
    assert!(
        hit.matched.only_beyond_the_prose(),
        "so the reader has to be told where it did match"
    );
    assert_eq!(hit.credit.as_deref(), Some("Jane <mark>Doe</mark>"));
    assert!(!hit.snippet.contains("<mark>"), "nothing to mark in there");

    let by_keyword = reader
        .search("nebula", &Filters::default(), false, 0, 30, 32)
        .await
        .unwrap();
    let hit = &by_keyword.items[0];
    assert!(hit.matched.keywords);
    assert_eq!(
        hit.keywords.as_deref(),
        Some("<mark>nebula</mark>"),
        "keywords arrive as a readable list, not as the JSON they are stored in"
    );

    let by_prose = reader
        .search("ringed", &Filters::default(), false, 0, 30, 32)
        .await
        .unwrap();
    let hit = &by_prose.items[0];
    assert!(hit.matched.explanation);
    assert!(!hit.matched.only_beyond_the_prose());
    assert_eq!(hit.credit, None, "no need to explain a visible match");
    assert_eq!(hit.keywords, None);

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

#[tokio::test]
async fn a_thumbnail_is_a_path_until_a_reader_is_told_how_to_serve_it() {
    let (writer, path) = seeded(&[("2024-03-05", "One", "a")]).await;
    let date: ApodDate = "2024-03-05".parse().unwrap();
    writer
        .set_thumb(date, Some(&Thumb::new(date.thumb_path())))
        .await
        .unwrap();

    let bare = writer.reader().entry(date).await.unwrap().unwrap();
    assert_eq!(
        bare.media.thumb_path.as_deref(),
        Some("2024/03/2024-03-05.webp")
    );
    assert_eq!(bare.media.thumb_url, None, "no base, no URL");
    assert_eq!(
        (bare.media.thumb_width, bare.media.thumb_height),
        (None, None),
        "a thumbnail recorded without a size stays unmeasured"
    );

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
        .set_thumb("2024-03-05".parse().unwrap(), Some(&Thumb::new("x.webp")))
        .await
        .unwrap();
    assert_eq!(writer.missing_thumbs().await.unwrap().len(), 1);
    assert_eq!(writer.reader().thumb_count().await.unwrap(), 1);

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn thumbnail_sizes_round_trip_and_the_unmeasured_ones_are_findable() {
    let (writer, path) = seeded(&[("2024-03-05", "One", "a"), ("2024-03-06", "Two", "b")]).await;
    let (measured, unmeasured): (ApodDate, ApodDate) =
        ("2024-03-05".parse().unwrap(), "2024-03-06".parse().unwrap());

    writer
        .set_thumb(measured, Some(&Thumb::sized("a.webp", 480, 271)))
        .await
        .unwrap();
    writer
        .set_thumb(unmeasured, Some(&Thumb::new("b.webp")))
        .await
        .unwrap();

    let entry = writer.reader().entry(measured).await.unwrap().unwrap();
    assert_eq!(
        (entry.media.thumb_width, entry.media.thumb_height),
        (Some(480), Some(271))
    );

    let page = writer
        .reader()
        .list(&Filters::default(), None, 10, Order::Desc)
        .await
        .unwrap();
    let summary = page
        .items
        .iter()
        .find(|item| item.date == measured)
        .unwrap();
    assert_eq!(summary.media.thumb_height, Some(271));

    let pending = writer.unmeasured_thumbs().await.unwrap();
    assert_eq!(pending.len(), 1, "only the one without a size is pending");
    assert_eq!(pending[0], (unmeasured, "b.webp".to_owned()));

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn a_kind_filter_selects_a_whole_group_of_kinds() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    let kinds = [
        ("2024-03-05", MediaKind::ImageJpg),
        ("2024-03-06", MediaKind::VideoMp4),
        ("2024-03-07", MediaKind::YouTube),
        ("2024-03-08", MediaKind::Vimeo),
    ];
    for (date, kind) in kinds {
        let mut row = entry(date, "Something", "A moving picture.");
        row.media = Media::new(kind, Some("https://example.test/x".into()), None);
        writer.upsert_sole(&row).await.unwrap();
    }

    let video = Filters {
        kind: Some("video".parse::<KindFilter>().unwrap()),
        ..Filters::default()
    };

    let listed = writer
        .reader()
        .list(&video, None, 10, Order::Desc)
        .await
        .unwrap();
    assert_eq!(
        listed.items.len(),
        3,
        "mp4, YouTube and Vimeo are all videos"
    );

    let found = writer
        .reader()
        .search("moving", &video, false, 0, 10, 32)
        .await
        .unwrap();
    assert_eq!(
        found.total, 3,
        "search has to filter the same way listing does"
    );

    let images = Filters {
        kind: Some("image".parse::<KindFilter>().unwrap()),
        ..Filters::default()
    };
    assert_eq!(
        writer
            .reader()
            .list(&images, None, 10, Order::Desc)
            .await
            .unwrap()
            .items
            .len(),
        1
    );

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn search_syntax_reaches_fts5_intact() {
    let (writer, path) = seeded(&[
        ("2024-03-05", "A Young Star Cluster", "Pretty."),
        (
            "2024-03-06",
            "A Star and a Globular Cluster",
            "Two separate things.",
        ),
        ("2024-03-07", "Hubble Sees a Galaxy", "Distant."),
        ("2024-03-08", "Webb Sees a Galaxy", "Also distant."),
    ])
    .await;

    let reader = writer.reader();
    let count = async |query: &str| {
        reader
            .search(query, &Filters::default(), true, 0, 30, 32)
            .await
            .unwrap()
            .total
    };

    assert_eq!(count("star cluster").await, 2, "bare words match anywhere");
    assert_eq!(
        count(r#""star cluster""#).await,
        1,
        "a quoted phrase has to be adjacent"
    );
    assert_eq!(count("galaxy").await, 2);
    assert_eq!(count("galaxy -hubble").await, 1, "exclusion");
    assert_eq!(count("cluster OR galaxy").await, 4, "either");
    assert_eq!(count("clust*").await, 2, "explicit prefix");
    assert_eq!(
        count(r#"" OR entries_fts MATCH ""#).await,
        0,
        "input that looks like syntax is matched as text, not run as syntax"
    );

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn writing_an_entry_catalogues_its_words_and_its_links() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    let mut row = entry(
        "2024-03-05",
        "Saturn",
        "The ringed planet is a ringed planet.",
    );
    row.explanation_html = r#"The <a href="https://en.wikipedia.org/wiki/Saturn">ringed</a>
                              planet, seen <a href="http://example.com/pic">again</a>."#
        .into();
    row.credits[0].html = r#"<a href="https://example.com/pic">Jane Doe</a>"#.into();
    writer.upsert_sole(&row).await.unwrap();

    let reader = writer.reader();

    let ringed = reader.word("ringed", 5).await.unwrap().unwrap();
    assert_eq!(ringed.word.total, 2, "twice in the one explanation");
    assert_eq!(ringed.word.entries, 1);
    assert_eq!(ringed.first.unwrap().to_string(), "2024-03-05");
    assert_eq!(ringed.by_year[0].year, 2024);
    assert_eq!(ringed.top_entries[0].count, 2);

    assert!(
        reader.word("the", 5).await.unwrap().is_some(),
        "common words are the point of a word catalogue, not noise to drop"
    );

    let catalogue = reader
        .resources(
            &ResourceFilters::default(),
            ResourceOrder::Refs,
            Order::Desc,
            0,
            10,
        )
        .await
        .unwrap();
    let urls: Vec<&str> = catalogue.items.iter().map(|r| r.url.as_str()).collect();
    assert_eq!(
        urls,
        vec![
            "https://example.com/pic",
            "https://en.wikipedia.org/wiki/Saturn"
        ],
        "the example is linked twice, and https wins over the http spelling of it"
    );

    let by_name = reader
        .resources(
            &ResourceFilters::default(),
            ResourceOrder::Label,
            Order::Asc,
            0,
            10,
        )
        .await
        .unwrap();
    assert_eq!(
        by_name
            .items
            .iter()
            .map(|r| r.label.as_deref().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["again", "ringed"],
        "sorting by name reads the link text, not the address"
    );

    let example = &catalogue.items[0];
    assert_eq!(example.refs, 2);
    assert_eq!(example.key, "example.com/pic", "stored without its scheme");
    assert_eq!(example.entries, 1, "one entry, however many links in it");
    assert_eq!(example.credited, 1);
    assert_eq!(example.host, "example.com");
    assert_eq!(
        example.label.as_deref(),
        Some("again"),
        "one entry records one wording, the first it used"
    );

    let summary = reader.text_summary().await.unwrap();
    assert_eq!(summary.measured, 1);
    assert_eq!(summary.total_words, 7);
    assert_eq!(summary.max_words, 7);
    assert_eq!(summary.longest.unwrap().title, "Saturn");

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn rewriting_an_entry_takes_its_old_words_and_links_out_of_the_totals() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    let mut row = entry("2024-03-05", "Saturn", "A ringed planet.");
    row.explanation_html = r#"<a href="https://example.com/old">old</a>"#.into();
    row.credits.clear();
    writer.upsert_sole(&row).await.unwrap();

    row.explanation_text = "A distant galaxy.".into();
    row.explanation_html = r#"<a href="https://example.com/new">new</a>"#.into();
    writer.upsert_sole(&row).await.unwrap();

    let reader = writer.reader();
    assert!(
        reader.word("ringed", 5).await.unwrap().is_none(),
        "a word nothing uses any more is gone from the catalogue, not left at zero"
    );
    assert_eq!(
        reader.word("galaxy", 5).await.unwrap().unwrap().word.total,
        1
    );

    let catalogue = reader
        .resources(
            &ResourceFilters::default(),
            ResourceOrder::Refs,
            Order::Desc,
            0,
            10,
        )
        .await
        .unwrap();
    assert_eq!(catalogue.total, 1, "the resource it dropped went with it");
    assert_eq!(catalogue.items[0].url, "https://example.com/new");

    let summary = reader.text_summary().await.unwrap();
    assert_eq!(summary.total_words, 3, "counted once, not twice");
    assert_eq!(summary.distinct_words, 3);

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn deleting_an_entry_drains_it_from_every_total() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    let mut row = entry("2024-03-05", "Saturn", "A ringed planet.");
    row.explanation_html = r#"<a href="https://example.com/x">x</a>"#.into();
    writer.upsert_sole(&row).await.unwrap();

    let db = writer.reader().db();
    sqlx::query("DELETE FROM entries WHERE date_id = ?1")
        .bind(row.date.days())
        .execute(db.writer().unwrap())
        .await
        .unwrap();

    for table in [
        "entry_words",
        "words",
        "entry_resources",
        "resources",
        "entry_stats",
    ] {
        let left: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
                .fetch_one(db.reader())
                .await
                .unwrap();
        assert_eq!(left, 0, "{table} outlived the entry it was derived from");
    }

    db.close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn one_resource_gathers_references_from_every_entry_that_links_it() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    for (date, anchor) in [
        ("2024-03-05", "the Crab"),
        ("2024-03-06", "Crab Nebula"),
        ("2024-03-07", "Crab Nebula"),
    ] {
        let mut row = entry(date, "Crab", "A supernova remnant.");
        row.explanation_html =
            format!(r#"<a href="https://en.wikipedia.org/wiki/Crab_Nebula">{anchor}</a>"#);
        row.credits.clear();
        writer.upsert_sole(&row).await.unwrap();
    }

    let reader = writer.reader();
    let catalogue = reader
        .resources(
            &ResourceFilters::default(),
            ResourceOrder::Refs,
            Order::Desc,
            0,
            10,
        )
        .await
        .unwrap();

    assert_eq!(catalogue.total, 1);
    let crab = &catalogue.items[0];
    assert_eq!(crab.refs, 3);
    assert_eq!(crab.entries, 3);
    assert_eq!(crab.first.unwrap().to_string(), "2024-03-05");
    assert_eq!(crab.last.unwrap().to_string(), "2024-03-07");
    assert_eq!(
        crab.label.as_deref(),
        Some("Crab Nebula"),
        "the wording APOD used most often names it"
    );

    let found = reader
        .resources(
            &ResourceFilters {
                query: Some("the crab".into()),
                ..ResourceFilters::default()
            },
            ResourceOrder::Refs,
            Order::Desc,
            0,
            10,
        )
        .await
        .unwrap();
    assert_eq!(
        found.total, 1,
        "searching reaches every wording, not only the most common one"
    );

    let refs = reader.resource(crab.id, 0, 10).await.unwrap().unwrap();
    assert_eq!(refs.total, 3);
    assert_eq!(refs.items.len(), 3);
    assert_eq!(refs.items[0].entry.date.to_string(), "2024-03-07");
    assert_eq!(refs.items[0].anchor, "Crab Nebula");

    let hosts = reader.resource_hosts(10).await.unwrap();
    assert_eq!(hosts[0].host, "en.wikipedia.org");
    assert_eq!((hosts[0].resources, hosts[0].refs), (1, 3));

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn the_word_list_reaches_both_ends_of_thirty_years_of_vocabulary() {
    let (writer, path) = seeded(&[
        ("2024-03-05", "One", "the ringed planet the planet"),
        ("2024-03-06", "Two", "the ringed moon"),
        ("2025-03-05", "Three", "a singular unrepeatable hapax"),
    ])
    .await;
    let reader = writer.reader();

    let most = reader
        .words(&WordFilters::default(), WordOrder::Total, Order::Desc, 0, 3)
        .await
        .unwrap();
    assert_eq!(most.items[0].word, "the");
    assert_eq!(most.items[0].total, 3);
    assert_eq!(most.items[0].entries, 2);

    let least = reader
        .words(
            &WordFilters::default(),
            WordOrder::Total,
            Order::Asc,
            0,
            100,
        )
        .await
        .unwrap();
    assert_eq!(least.items[0].total, 1, "the far end is one query away");
    assert_eq!(least.total, most.total, "both ends of the same list");

    let prefix = reader
        .words(
            &WordFilters {
                query: Some("ring*".into()),
                ..WordFilters::default()
            },
            WordOrder::Total,
            Order::Desc,
            0,
            10,
        )
        .await
        .unwrap();
    assert_eq!(prefix.items.len(), 1);
    assert_eq!(prefix.items[0].word, "ringed");

    let substring = reader
        .words(
            &WordFilters {
                query: Some("plan".into()),
                ..WordFilters::default()
            },
            WordOrder::Total,
            Order::Desc,
            0,
            10,
        )
        .await
        .unwrap();
    assert_eq!(substring.items[0].word, "planet");

    let timeline = reader.timeline().await.unwrap();
    assert_eq!(timeline.years.len(), 2);
    assert_eq!(timeline.years[0].year, 2024);
    assert_eq!(timeline.years[0].entries, 2);
    assert_eq!(
        timeline.years[1].new_words, 4,
        "2025 brought four of its own"
    );

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

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
    writer.upsert_sole(&original).await.unwrap();

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

#[tokio::test]
async fn a_rerun_picture_is_findable_from_any_of_its_dates() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    let runs = [
        ("2019-09-20", "image/1909/saturn.jpg", [1u8; 32]),
        ("2021-09-11", "image/2109/saturn.jpg", [1u8; 32]),
        ("2022-11-26", "image/2211/saturn.jpg", [1u8; 32]),
        ("2023-01-01", "image/2301/orion.jpg", [9u8; 32]),
    ];
    for (date, url, phash) in runs {
        let mut row = entry(date, "Saturn at Night", "The ringed planet.");
        row.media = Media::new(MediaKind::ImageJpg, Some(url.to_owned()), None);
        writer.upsert_sole(&row).await.unwrap();
        writer
            .set_thumb(
                row.date,
                Some(&Thumb::sized(row.date.thumb_path(), 480, 320)),
            )
            .await
            .unwrap();
        writer.set_phash(row.date, Some(&phash)).await.unwrap();
    }

    let groups = writer.regroup_pictures().await.unwrap();
    assert_eq!(groups.len(), 1, "one picture ran three times, one ran once");
    assert_eq!(groups[0].id().to_string(), "2019-09-20");

    let reader = writer.reader();
    for date in ["2019-09-20", "2021-09-11", "2022-11-26"] {
        let dates = reader
            .picture_dates(date.parse().unwrap())
            .await
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        assert_eq!(
            dates,
            vec!["2019-09-20", "2021-09-11", "2022-11-26"],
            "every date of a rerun has to reach the other two, {date} did not"
        );
    }

    let alone: ApodDate = "2023-01-01".parse().unwrap();
    assert_eq!(
        reader.picture_dates(alone).await.unwrap(),
        vec![alone],
        "a picture that ran once answers with its own date"
    );

    let pool: Vec<String> = reader
        .picture_pool(None)
        .await
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        pool,
        vec!["2019-09-20", "2023-01-01"],
        "the games deal each picture once, on the day it first ran, or a round can ask which of \
         two identical pictures came first"
    );

    let before: Vec<String> = reader
        .picture_pool(Some("2022-01-01".parse().unwrap()))
        .await
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        before,
        vec!["2019-09-20"],
        "a daily only knows the reruns that had already happened"
    );

    let summary = reader.picture_summary().await.unwrap();
    assert_eq!((summary.pictures, summary.entries), (1, 3));
    assert_eq!(summary.hashed, 4);
    assert_eq!(summary.most_shown_times, 3);
    assert_eq!(summary.most_shown.unwrap().to_string(), "2019-09-20");

    let mut earlier = entry("2018-01-01", "Saturn at Night", "The ringed planet.");
    earlier.media = Media::new(
        MediaKind::ImageJpg,
        Some("image/1801/saturn.jpg".to_owned()),
        None,
    );
    writer.upsert_sole(&earlier).await.unwrap();
    writer
        .set_phash(earlier.date, Some(&[1u8; 32]))
        .await
        .unwrap();

    let groups = writer.regroup_pictures().await.unwrap();
    assert_eq!(groups[0].dates.len(), 4);
    assert_eq!(groups[0].id().to_string(), "2018-01-01");
    assert_eq!(
        reader.picture_dates(alone).await.unwrap(),
        vec![alone],
        "the entry that was never a rerun is still not one"
    );

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn an_encore_reports_what_each_appearance_changed() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    // Same picture four times: republished untouched, then retitled, then rewritten onto a
    // rescanned file. A fifth entry is a different picture and must stay out of the group.
    let shown = [
        (
            "2001-03-04",
            "Saturn at Night",
            "The ringed planet.",
            "a.jpg",
        ),
        (
            "2005-06-07",
            "Saturn at Night",
            "The ringed planet.",
            "a.jpg",
        ),
        (
            "2011-08-09",
            "Saturn After Dark",
            "The ringed planet.",
            "a.jpg",
        ),
        (
            "2019-10-11",
            "Saturn After Dark",
            "Rewritten prose.",
            "b.jpg",
        ),
    ];
    for (date, title, prose, file) in shown {
        let mut row = entry(date, title, prose);
        row.media = Media::new(MediaKind::ImageJpg, Some(file.to_owned()), None);
        row.credits = vec![Credit {
            role: "Image Credit".into(),
            html: "NASA".into(),
            text: "NASA".into(),
        }];
        writer.upsert_sole(&row).await.unwrap();
        writer.set_phash(row.date, Some(&[3u8; 32])).await.unwrap();
    }

    let mut other = entry("2020-01-01", "Orion", "A different picture.");
    other.media = Media::new(MediaKind::ImageJpg, Some("orion.jpg".to_owned()), None);
    writer.upsert_sole(&other).await.unwrap();
    writer
        .set_phash(other.date, Some(&[200u8; 32]))
        .await
        .unwrap();

    writer.regroup_pictures().await.unwrap();
    let reader = writer.reader();

    let listing = reader
        .pictures(
            &PictureFilters::default(),
            PictureOrder::Appearances,
            Order::Desc,
            0,
            10,
        )
        .await
        .unwrap();
    assert_eq!(
        listing.total, 1,
        "only one picture was shown more than once"
    );
    let picture = &listing.items[0];
    assert_eq!(picture.id.to_string(), "2001-03-04");
    assert_eq!(picture.appearances, 4);
    assert_eq!(picture.titles, 2, "it appeared under two titles");
    assert_eq!(picture.first.to_string(), "2001-03-04");
    assert_eq!(picture.last.to_string(), "2019-10-11");
    assert_eq!(
        picture.title, "Saturn at Night",
        "a picture is named after its first appearance"
    );

    // Any of the dates addresses the picture, not only the one it is named after.
    for date in ["2001-03-04", "2011-08-09", "2019-10-11"] {
        let shown = reader
            .picture_appearances(date.parse().unwrap())
            .await
            .unwrap()
            .unwrap_or_else(|| panic!("{date} should reach its picture"));
        assert_eq!(shown.picture.id.to_string(), "2001-03-04");
        assert_eq!(shown.items.len(), 4);
    }

    let shown = reader
        .picture_appearances("2001-03-04".parse().unwrap())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        shown.items[0].changed,
        Changed::default(),
        "the first appearance changed nothing, there was nothing to change from"
    );
    assert_eq!(shown.items[0].since_previous_days, None);

    assert_eq!(
        shown.items[1].changed,
        Changed::default(),
        "a straight republication changed nothing"
    );
    assert_eq!(shown.items[1].since_previous_days, Some(1556));

    assert_eq!(
        shown.items[2].changed,
        Changed {
            title: true,
            ..Changed::default()
        },
        "only the title moved"
    );

    assert_eq!(
        shown.items[3].changed,
        Changed {
            title: false,
            explanation: true,
            credit: false,
            file: true,
        },
        "new prose on a new file, under the title it already had"
    );

    assert!(
        reader
            .picture_appearances("2020-01-01".parse().unwrap())
            .await
            .unwrap()
            .is_none(),
        "a picture shown once is not an encore"
    );

    let retitled = reader
        .pictures(
            &PictureFilters {
                retitled: Some(false),
                ..PictureFilters::default()
            },
            PictureOrder::Appearances,
            Order::Desc,
            0,
            10,
        )
        .await
        .unwrap();
    assert_eq!(
        retitled.total, 0,
        "this one was retitled, so it filters out"
    );

    let searched = reader
        .pictures(
            &PictureFilters {
                query: Some("after dark".into()),
                ..PictureFilters::default()
            },
            PictureOrder::Appearances,
            Order::Desc,
            0,
            10,
        )
        .await
        .unwrap();
    assert_eq!(
        searched.total, 1,
        "searching reaches the titles of every appearance, not only the first"
    );

    // The marker a listing badges on, and what the entry page decides with.
    let summary = reader.entry("2011-08-09".parse().unwrap()).await.unwrap();
    assert_eq!(
        summary.unwrap().picture.map(|d| d.to_string()).as_deref(),
        Some("2001-03-04")
    );
    let single = reader.entry("2020-01-01".parse().unwrap()).await.unwrap();
    assert_eq!(single.unwrap().picture, None);

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn only_pictures_worth_playing_reach_a_game() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    let rows = [
        ("2020-01-01", MediaKind::ImageJpg, true),
        ("2020-01-02", MediaKind::ImageJpg, false),
        ("2020-01-03", MediaKind::YouTube, true),
        ("2020-01-04", MediaKind::ImagePng, true),
    ];
    for (date, kind, thumbed) in rows {
        let mut row = entry(date, "Something", "A picture of something.");
        row.media = Media::new(kind, Some(format!("https://example.test/{date}")), None);
        writer.upsert_sole(&row).await.unwrap();
        if thumbed {
            writer
                .set_thumb(
                    row.date,
                    Some(&Thumb::sized(row.date.thumb_path(), 480, 320)),
                )
                .await
                .unwrap();
        }
    }

    let pool = writer.reader().picture_pool(None).await.unwrap();
    let dates: Vec<String> = pool.iter().map(ToString::to_string).collect();
    assert_eq!(
        dates,
        vec!["2020-01-01", "2020-01-04"],
        "no thumbnail means nothing to show, and a video thumbnail is a frame rather than the picture"
    );

    let before = writer
        .reader()
        .picture_pool(Some("2020-01-04".parse().unwrap()))
        .await
        .unwrap();
    assert_eq!(
        before.len(),
        1,
        "a day's puzzle can only draw on what was already published"
    );

    // The word game draws from its own pool, and it has to exclude videos for the same reason:
    // solving an explanation should reveal the picture, not a frame off a YouTube still.
    let prose = "The nebula glows across the field of view in this deep exposure. ".repeat(12);
    for (date, kind) in [
        ("2021-01-01", MediaKind::ImageJpg),
        ("2021-01-02", MediaKind::YouTube),
        ("2021-01-03", MediaKind::Vimeo),
    ] {
        let mut row = entry(date, "Something", &prose);
        row.media = Media::new(kind, Some(format!("https://example.test/{date}")), None);
        writer.upsert_sole(&row).await.unwrap();
        writer
            .set_thumb(
                row.date,
                Some(&Thumb::sized(row.date.thumb_path(), 480, 320)),
            )
            .await
            .unwrap();
    }

    let words: Vec<String> = writer
        .reader()
        .text_pool(None)
        .await
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        words,
        vec!["2021-01-01"],
        "a video's thumbnail is not the picture the explanation describes"
    );

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn a_word_puzzle_only_takes_explanations_worth_redacting() {
    let short = std::iter::repeat_n("nebula", 60)
        .collect::<Vec<_>>()
        .join(" ");
    let long = std::iter::repeat_n("nebula", 260)
        .collect::<Vec<_>>()
        .join(" ");
    let right = std::iter::repeat_n("nebula", 100)
        .collect::<Vec<_>>()
        .join(" ");

    let (writer, path) = seeded(&[
        ("2020-01-01", "Short", &short),
        ("2020-01-02", "Long", &long),
        ("2020-01-03", "Right", &right),
    ])
    .await;

    for date in ["2020-01-01", "2020-01-02", "2020-01-03"] {
        let date: ApodDate = date.parse().unwrap();
        writer
            .set_thumb(date, Some(&Thumb::sized(date.thumb_path(), 480, 320)))
            .await
            .unwrap();
    }

    let pool = writer.reader().text_pool(None).await.unwrap();
    assert_eq!(pool.len(), 1);
    assert_eq!(pool[0].to_string(), "2020-01-03");

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn coverage_counts_each_month_and_leaves_the_empty_ones_out() {
    let (writer, path) = seeded(&[
        ("1995-06-16", "First", "the very first one"),
        ("1995-06-20", "Second", "four days later"),
        ("1995-08-01", "Third", "a month with a gap before it"),
        ("2024-03-05", "Saturn", "the ringed planet"),
    ])
    .await;

    let months = writer.reader().coverage().await.unwrap().months;

    let counted: Vec<_> = months
        .iter()
        .map(|month| (month.year, month.month, month.entries))
        .collect();

    assert_eq!(
        counted,
        vec![(1995, 6, 2), (1995, 8, 1), (2024, 3, 1)],
        "July 1995 has nothing in it and should not appear at all"
    );

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn a_rerun_that_changed_only_its_credit_label_is_reported_as_changed() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    let runs = [
        ("1996-02-29", "Photo Credit", "image/9602/aurora.jpg"),
        (
            "2000-02-29",
            "Photo Credit & Copyright",
            "image/9602/aurora.jpg",
        ),
        (
            "2004-02-29",
            "Photo Credit & Copyright",
            "image/0402/aurora.jpg",
        ),
    ];

    for (date, role, url) in runs {
        let mut row = entry(date, "Aurora over Norway", "Green curtains over a fjord.");
        row.credits = vec![Credit {
            role: role.into(),
            html: "Rune Rysstad".into(),
            text: "Rune Rysstad".into(),
        }];
        row.media = Media::new(MediaKind::ImageJpg, Some(url.to_owned()), None);
        writer.upsert_sole(&row).await.unwrap();
        writer
            .set_thumb(
                row.date,
                Some(&Thumb::sized(row.date.thumb_path(), 480, 320)),
            )
            .await
            .unwrap();
        writer.set_phash(row.date, Some(&[7u8; 32])).await.unwrap();
    }
    writer.regroup_pictures().await.unwrap();

    let reader = writer.reader();
    let found = reader
        .picture_appearances("1996-02-29".parse().unwrap())
        .await
        .unwrap()
        .expect("three runs of one picture is a picture");

    let changed: Vec<_> = found
        .items
        .iter()
        .map(|item| (item.entry.date.to_string(), &item.changed))
        .collect();
    assert_eq!(changed.len(), 3);

    assert!(
        !changed[0].1.any(),
        "the first run had nothing before it to differ from"
    );

    let second = changed[1].1;
    assert!(
        second.credit,
        "Photo Credit became Photo Credit & Copyright, which is the run claiming a copyright"
    );
    assert!(!second.file, "the same file, so nothing to say about it");
    assert!(!second.title && !second.explanation);

    let third = changed[2].1;
    assert!(third.file, "a different file behind the same words");
    assert!(
        !third.credit,
        "the credit block is identical to the run before, so it must not be flagged"
    );

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn a_rerun_that_moved_only_its_high_resolution_file_is_reported_as_changed() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    let runs = [
        (
            "2015-05-05",
            "image/1505/m27_small.jpg",
            "image/1505/m27_big.jpg",
        ),
        (
            "2018-08-08",
            "image/1505/m27_small.jpg",
            "image/1808/m27_huge.jpg",
        ),
    ];

    for (date, url, hd) in runs {
        let mut row = entry(date, "The Dumbbell Nebula", "A planetary nebula.");
        row.media = Media::new(
            MediaKind::ImageJpg,
            Some(url.to_owned()),
            Some(hd.to_owned()),
        );
        writer.upsert_sole(&row).await.unwrap();
        writer
            .set_thumb(
                row.date,
                Some(&Thumb::sized(row.date.thumb_path(), 480, 320)),
            )
            .await
            .unwrap();
        writer.set_phash(row.date, Some(&[3u8; 32])).await.unwrap();
    }
    writer.regroup_pictures().await.unwrap();

    let found = writer
        .reader()
        .picture_appearances("2018-08-08".parse().unwrap())
        .await
        .unwrap()
        .unwrap();

    assert!(
        found.items[1].changed.file,
        "the display url never moved, but the picture a reader is shown did"
    );

    writer.reader().db().close().await;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn an_entry_is_lost_by_the_same_rule_whether_sql_or_rust_is_asked() {
    let path = temp_db();
    let writer = ApodWriter::open(&path).await.unwrap();

    let mut gone = entry("2023-01-24", "Rocky", "A picture nobody kept.");
    gone.media = Media::new(
        MediaKind::ImageJpg,
        Some("https://apod.nasa.gov/apod/image/2301/rocky.jpg".into()),
        None,
    );

    let thumbed = entry("2023-01-25", "Kept", "A picture we made a copy of.");

    let mut elsewhere = entry("2023-01-26", "Fermilab", "Somebody else's host.");
    elsewhere.media = Media::new(
        MediaKind::ImageGif,
        Some("http://nusoft.fnal.gov/nova/public/img/echo.gif".into()),
        None,
    );

    let mut nothing = entry("2023-01-27", "Nothing", "An entry with no picture.");
    nothing.media = Media::new(MediaKind::None, None, None);

    let dates: Vec<ApodDate> = [&gone, &thumbed, &elsewhere, &nothing]
        .iter()
        .map(|entry| entry.date)
        .collect();

    let entries: Vec<apod_core::Merged> = vec![
        gone.into(),
        thumbed.clone().into(),
        elsewhere.into(),
        nothing.into(),
    ];
    writer.upsert_all(&entries).await.unwrap();
    writer
        .set_thumb(thumbed.date, Some(&Thumb::new(thumbed.date.thumb_path())))
        .await
        .unwrap();

    let reader = writer.reader();

    let mut lost: Vec<ApodDate> = Vec::new();
    let mut left: Vec<ApodDate> = Vec::new();
    for date in dates {
        let entry = reader.entry(date).await.unwrap().unwrap();
        match entry.media.is_lost() {
            true => lost.push(date),
            false => left.push(date),
        }
    }

    assert_eq!(
        lost,
        ["2023-01-24".parse::<ApodDate>().unwrap()],
        "only the dead host with nothing made from it is lost"
    );

    for (wanted, expected) in [(true, &lost), (false, &left)] {
        let filters = Filters {
            lost: Some(wanted),
            ..Filters::default()
        };
        let page = reader.list(&filters, None, 50, Order::Asc).await.unwrap();
        let found: Vec<ApodDate> = page.items.iter().map(|item| item.date).collect();

        assert_eq!(&found, expected, "lost = {wanted}");
    }
}

#[tokio::test]
async fn a_search_with_no_terms_is_the_filters_on_their_own() {
    let (writer, _) = seeded(&[
        ("2024-03-05", "Saturn", "The ringed planet."),
        ("2024-03-06", "Jupiter", "The banded planet."),
        ("2024-03-07", "Mars", "The red planet."),
    ])
    .await;
    let reader = writer.reader();

    let all = reader
        .search("", &Filters::default(), false, 0, 10, 32)
        .await
        .unwrap();
    assert_eq!(all.total, 3);
    assert_eq!(
        all.items
            .iter()
            .map(|hit| hit.entry.date)
            .collect::<Vec<_>>(),
        [
            "2024-03-07".parse::<ApodDate>().unwrap(),
            "2024-03-06".parse().unwrap(),
            "2024-03-05".parse().unwrap()
        ],
        "newest first, since there is nothing to be relevant to"
    );
    assert!(all.items.iter().all(|hit| hit.snippet.is_empty()));

    let narrowed = reader
        .search(
            "   ",
            &Filters {
                from: Some("2024-03-06".parse().unwrap()),
                ..Filters::default()
            },
            false,
            0,
            10,
            32,
        )
        .await
        .unwrap();
    assert_eq!(narrowed.total, 2, "whitespace is not a query either");

    let nothing_to_match = reader
        .search("-saturn", &Filters::default(), false, 0, 10, 32)
        .await
        .unwrap();
    assert_eq!(
        nothing_to_match.total, 0,
        "a query made only of exclusions still matches nothing, and is not everything"
    );
}
