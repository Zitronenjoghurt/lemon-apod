use crate::archive::{Next, Source};
use crate::client::{Client, Limit, Response};
use crate::config::Config;
use crate::fetch::{self, sha256};
use crate::thumbs;
use anyhow::{Context, Result};
use apod_core::db::Db;
use apod_core::{ApodDate, Media, MediaKind};
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

const NAME_MAX: usize = 96;
const HASH_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Image,
    ImageAlt,
    LegacyImage,
    Video,
    Poster,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::ImageAlt => "image_alt",
            Self::LegacyImage => "legacy_image",
            Self::Video => "video",
            Self::Poster => "poster",
        }
    }

    fn wants_video(self) -> bool {
        matches!(self, Self::Video)
    }
}

pub fn source_of(url: &str) -> Source {
    match url.contains("science.nasa.gov") {
        true => Source::Modern,
        false => Source::Legacy,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub url: String,
    pub date: ApodDate,
    pub role: Role,
}

pub fn targets(
    entries: &[(ApodDate, Media)],
    origin_pairs: &[(ApodDate, String, String)],
) -> Vec<Target> {
    let mut out: Vec<Target> = Vec::with_capacity(entries.len() + origin_pairs.len());
    let mut seen: HashSet<String> = HashSet::with_capacity(entries.len() + origin_pairs.len());

    let mut entries: Vec<&(ApodDate, Media)> = entries.iter().collect();
    entries.sort_by_key(|(date, _)| *date);
    let mut origin_pairs: Vec<&(ApodDate, String, String)> = origin_pairs.iter().collect();
    origin_pairs.sort_by_key(|(date, _, _)| *date);

    for (date, media) in entries {
        let candidates = match media.kind {
            kind if kind.is_image() => {
                let mut candidates = Vec::with_capacity(2);
                if let Some(best) = media.best_url() {
                    candidates.push((best, Role::Image));
                }
                if let (Some(url), Some(hd)) = (&media.url, &media.hd_url)
                    && format_of(url) != format_of(hd)
                {
                    candidates.push((url.as_str(), Role::ImageAlt));
                }
                candidates
            }
            MediaKind::VideoMp4 => media
                .url
                .as_deref()
                .map(|url| vec![(url, Role::Video)])
                .unwrap_or_default(),
            MediaKind::YouTube | MediaKind::Vimeo => media
                .url
                .as_deref()
                .map(|url| vec![(url, Role::Poster)])
                .unwrap_or_default(),
            _ => Vec::new(),
        };

        for (url, role) in candidates {
            if seen.insert(url.to_owned()) {
                out.push(Target {
                    url: url.to_owned(),
                    date: *date,
                    role,
                });
            }
        }
    }

    for (date, legacy, _) in origin_pairs {
        if seen.insert(legacy.clone()) {
            out.push(Target {
                url: legacy.clone(),
                date: *date,
                role: Role::LegacyImage,
            });
        }
    }

    out
}

fn format_of(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or("");
    let ext = name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "jpeg" => "jpg".to_owned(),
        other => other.to_owned(),
    }
}

pub fn stored_path(date: ApodDate, file_name: &str) -> String {
    format!("{}/{date}/{file_name}", date.format("%Y/%m"))
}

fn named(url: &str, siblings: &[String], format: Format) -> String {
    let name = file_name(url, siblings);
    match name.contains('.') {
        true => name,
        false => format!("{name}.{}", format.extension()),
    }
}

pub fn file_name(url: &str, siblings: &[String]) -> String {
    let base = base_name(url);
    let clashes = siblings
        .iter()
        .any(|other| other != url && base_name(other) == base);

    if !clashes {
        return base;
    }

    let hash = &sha256(url.as_bytes())[..HASH_LEN];
    match base.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}-{hash}.{ext}"),
        None => format!("{base}-{hash}"),
    }
}

fn base_name(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let raw: String = path
        .rsplit('/')
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '\0'))
        .collect();

    if raw.is_empty() || raw == "." || raw == ".." {
        return format!("{}.bin", &sha256(url.as_bytes())[..HASH_LEN * 2]);
    }

    match raw.char_indices().nth(NAME_MAX) {
        None => raw,
        Some((at, _)) => raw[..at].to_owned(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Jpeg,
    Png,
    Gif,
    WebP,
    Tiff,
    Bmp,
    Mp4,
}

impl Format {
    fn content_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Gif => "image/gif",
            Self::WebP => "image/webp",
            Self::Tiff => "image/tiff",
            Self::Bmp => "image/bmp",
            Self::Mp4 => "video/mp4",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Gif => "gif",
            Self::WebP => "webp",
            Self::Tiff => "tif",
            Self::Bmp => "bmp",
            Self::Mp4 => "mp4",
        }
    }

    fn is_video(self) -> bool {
        matches!(self, Self::Mp4)
    }
}

fn sniff(bytes: &[u8]) -> Option<Format> {
    let starts = |prefix: &[u8]| bytes.starts_with(prefix);

    if starts(&[0xFF, 0xD8, 0xFF]) {
        return Some(Format::Jpeg);
    }
    if starts(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some(Format::Png);
    }
    if starts(b"GIF87a") || starts(b"GIF89a") {
        return Some(Format::Gif);
    }
    if starts(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Some(Format::WebP);
    }
    if starts(b"II\x2A\x00") || starts(b"MM\x00\x2A") {
        return Some(Format::Tiff);
    }
    if starts(b"BM") {
        return Some(Format::Bmp);
    }
    if bytes.get(4..8) == Some(b"ftyp") {
        return Some(Format::Mp4);
    }

    None
}

fn verified(bytes: &[u8], role: Role) -> Result<Format, String> {
    let Some(format) = sniff(bytes) else {
        return Err(format!(
            "the first bytes are not a picture or a video, so the {} byte body is something \
             else served with a 200",
            bytes.len()
        ));
    };

    if format.is_video() == role.wants_video() {
        Ok(format)
    } else {
        Err(format!(
            "a {} was served where the entry says {}",
            format.content_type(),
            role.as_str()
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Stored { bytes: usize },
    Adopted { bytes: usize },
    Missing,
    Rejected(String),
    Failed(String),
}

pub async fn fetch_and_store(
    cfg: &Config,
    client: &Client,
    store: &MediaStore,
    target: &Target,
    siblings: &[String],
) -> Result<Outcome> {
    let now = chrono::Utc::now().timestamp();

    if let Some(outcome) = adopt(cfg, store, target, siblings, now).await? {
        return Ok(outcome);
    }

    let sources = match sources(cfg, client, target).await {
        Ok(sources) => sources,
        Err(error) => {
            let reason = format!("{error:#}");
            store.record_failure(target, None, &reason, now).await?;
            return Ok(Outcome::Failed(reason));
        }
    };

    let limit = Limit {
        max_bytes: cfg.media.max_bytes,
        timeout: cfg.media.timeout,
    };
    let mut last: Option<(Option<u16>, String)> = None;

    for source in &sources {
        match client.get_limited(source, limit).await {
            Ok(Response::Body(bytes)) => match verified(&bytes, target.role) {
                Ok(format) => {
                    let path = stored_path(target.date, &named(source, siblings, format));
                    fetch::write_atomically(&cfg.media_path(&path), &bytes)?;
                    store
                        .record_stored(
                            target,
                            &path,
                            &sha256(&bytes),
                            bytes.len(),
                            format.content_type(),
                            now,
                        )
                        .await?;
                    return Ok(Outcome::Stored { bytes: bytes.len() });
                }
                Err(reason) => last = Some((Some(200), reason)),
            },
            Ok(Response::NotFound) => last = Some((Some(404), format!("{source} is gone"))),
            Ok(Response::Refused { status }) => {
                last = Some((Some(status), format!("{source} returned {status}")))
            }
            Ok(Response::Redirected { status, location }) => {
                last = Some((
                    Some(status),
                    match location {
                        Some(target) => format!("{source} redirected with {status} to {target}"),
                        None => format!("{source} redirected with {status}"),
                    },
                ))
            }
            Err(error) => last = Some((None, format!("{error:#}"))),
        }
    }

    let (status, reason) = last.unwrap_or((None, format!("nothing to fetch for {}", target.url)));
    store.record_failure(target, status, &reason, now).await?;

    Ok(match status {
        Some(404 | 410) => Outcome::Missing,
        Some(200) => Outcome::Rejected(reason),
        _ => Outcome::Failed(reason),
    })
}

async fn adopt(
    cfg: &Config,
    store: &MediaStore,
    target: &Target,
    siblings: &[String],
    now: i64,
) -> Result<Option<Outcome>> {
    if target.role == Role::Poster {
        return Ok(None);
    }

    let path = stored_path(target.date, &file_name(&target.url, siblings));
    let on_disk = cfg.media_path(&path);

    let Ok(bytes) = std::fs::read(&on_disk) else {
        return Ok(None);
    };
    let Ok(format) = verified(&bytes, target.role) else {
        return Ok(None);
    };

    store
        .record_stored(
            target,
            &path,
            &sha256(&bytes),
            bytes.len(),
            format.content_type(),
            now,
        )
        .await?;

    Ok(Some(Outcome::Adopted { bytes: bytes.len() }))
}

async fn sources(cfg: &Config, client: &Client, target: &Target) -> Result<Vec<String>> {
    if target.role != Role::Poster {
        return Ok(vec![target.url.clone()]);
    }

    let media = Media::new(
        MediaKind::from_url(&target.url),
        Some(target.url.clone()),
        None,
    );
    thumbs::poster_urls(cfg, client, &media.thumb_source()).await
}

#[derive(Debug, Clone, Copy)]
struct Attempted {
    stored: bool,
    http_status: Option<u16>,
    attempts: u32,
    last_checked_at: i64,
}

impl Attempted {
    fn settled(&self) -> bool {
        self.stored || matches!(self.http_status, Some(404 | 410 | 300..=399))
    }

    fn due_in(&self, backoff_max: Duration, now: i64) -> i64 {
        crate::archive::backoff(self.attempts, backoff_max).as_secs() as i64
            - now.saturating_sub(self.last_checked_at)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Counts {
    pub stored: i64,
    pub missing: i64,
    pub failed: i64,
    pub bytes: i64,
}

#[derive(Debug, Clone)]
pub struct MediaStore {
    db: Db,
}

impl MediaStore {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn next_target(
        &self,
        targets: &[Target],
        backoff_max: Duration,
        now: i64,
    ) -> Result<Next<Target>> {
        let attempted = self.attempted().await?;

        if let Some(target) = targets
            .iter()
            .filter(|target| !attempted.contains_key(&target.url))
            .max_by_key(|target| target.date)
        {
            return Ok(Next::Fetch(target.clone()));
        }

        let mut soonest: Option<i64> = None;
        let mut due: Option<(i64, &Target)> = None;

        for (record, target) in targets
            .iter()
            .filter_map(|target| Some((attempted.get(&target.url)?, target)))
            .filter(|(record, _)| !record.settled())
        {
            let due_in = record.due_in(backoff_max, now);
            match due_in <= 0 {
                true if due.is_none_or(|(waited, _)| record.last_checked_at < waited) => {
                    due = Some((record.last_checked_at, target))
                }
                true => {}
                false => soonest = Some(soonest.map_or(due_in, |soonest: i64| soonest.min(due_in))),
            }
        }

        if let Some((_, target)) = due {
            return Ok(Next::Fetch(target.clone()));
        }

        Ok(match soonest {
            Some(seconds) => Next::Waiting(Duration::from_secs(seconds.max(0) as u64)),
            None => Next::Complete,
        })
    }

    async fn attempted(&self) -> Result<HashMap<String, Attempted>> {
        let rows =
            sqlx::query("SELECT url, path, http_status, attempts, last_checked_at FROM media")
                .fetch_all(self.db.reader())
                .await
                .context("reading media fetch state")?;

        Ok(rows
            .iter()
            .map(|row| {
                (
                    row.get::<String, _>("url"),
                    Attempted {
                        stored: row.get::<Option<String>, _>("path").is_some(),
                        http_status: row
                            .get::<Option<i64>, _>("http_status")
                            .map(|status| status as u16),
                        attempts: row.get::<i64, _>("attempts") as u32,
                        last_checked_at: row
                            .get::<Option<i64>, _>("last_checked_at")
                            .unwrap_or_default(),
                    },
                )
            })
            .collect())
    }

    pub async fn stored_path(&self, url: &str) -> Result<Option<String>> {
        sqlx::query_scalar("SELECT path FROM media WHERE url = ?1")
            .bind(url)
            .fetch_optional(self.db.reader())
            .await
            .map(Option::flatten)
            .context("looking up an archived media file")
    }

    pub async fn record_stored(
        &self,
        target: &Target,
        path: &str,
        sha256: &str,
        bytes: usize,
        content_type: &str,
        now: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO media (url, date_id, role, source, path, http_status, sha256, bytes,
                                content_type, fetched_at, last_checked_at, attempts, error)
             VALUES (?1, ?2, ?3, ?4, ?5, 200, ?6, ?7, ?8, ?9, ?9, 1, NULL)
             ON CONFLICT(url) DO UPDATE SET
               date_id = excluded.date_id, role = excluded.role, source = excluded.source,
               path = excluded.path,
               http_status = 200, sha256 = excluded.sha256, bytes = excluded.bytes,
               content_type = excluded.content_type, fetched_at = excluded.fetched_at,
               last_checked_at = excluded.last_checked_at, attempts = media.attempts + 1,
               error = NULL",
        )
        .bind(&target.url)
        .bind(target.date.days())
        .bind(target.role.as_str())
        .bind(source_of(&target.url).as_str())
        .bind(path)
        .bind(sha256)
        .bind(bytes as i64)
        .bind(content_type)
        .bind(now)
        .execute(self.db.writer()?)
        .await
        .context("recording a stored media file")?;
        Ok(())
    }

    pub async fn record_failure(
        &self,
        target: &Target,
        status: Option<u16>,
        error: &str,
        now: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO media (url, date_id, role, source, http_status, last_checked_at,
                                attempts, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)
             ON CONFLICT(url) DO UPDATE SET
               date_id = excluded.date_id, role = excluded.role, source = excluded.source,
               http_status = excluded.http_status, last_checked_at = excluded.last_checked_at,
               attempts = media.attempts + 1, error = excluded.error",
        )
        .bind(&target.url)
        .bind(target.date.days())
        .bind(target.role.as_str())
        .bind(source_of(&target.url).as_str())
        .bind(status.map(i64::from))
        .bind(now)
        .bind(error)
        .execute(self.db.writer()?)
        .await
        .context("recording a failed media fetch")?;
        Ok(())
    }

    pub async fn hashes(&self, urls: &[String]) -> Result<HashMap<String, String>> {
        let mut found = HashMap::with_capacity(urls.len());

        for chunk in urls.chunks(256) {
            let placeholders = vec!["?"; chunk.len()].join(", ");
            let mut query = sqlx::query_as::<_, (String, String)>(sqlx::AssertSqlSafe(format!(
                "SELECT url, sha256 FROM media
                     WHERE sha256 IS NOT NULL AND url IN ({placeholders})"
            )));
            for url in chunk {
                query = query.bind(url);
            }
            found.extend(
                query
                    .fetch_all(self.db.reader())
                    .await
                    .context("reading digests")?,
            );
        }

        Ok(found)
    }

    pub async fn counts(&self) -> Result<Counts> {
        let (stored, missing, failed, bytes): (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT
               COUNT(*) FILTER (WHERE path IS NOT NULL),
               COUNT(*) FILTER (WHERE path IS NULL AND http_status IN (404, 410)),
               COUNT(*) FILTER (WHERE path IS NULL AND (http_status IS NULL
                                                        OR http_status NOT IN (404, 410))),
               COALESCE(SUM(bytes), 0)
             FROM media",
        )
        .fetch_one(self.db.reader())
        .await
        .context("counting media")?;

        Ok(Counts {
            stored,
            missing,
            failed,
            bytes,
        })
    }
}

pub fn siblings(targets: &[Target], date: ApodDate) -> Vec<String> {
    targets
        .iter()
        .filter(|target| target.date == date)
        .map(|target| target.url.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use apod_core::db::{Db, DbConfig};
    use std::sync::atomic::{AtomicU32, Ordering};

    fn date(y: i32, m: u32, d: u32) -> ApodDate {
        ApodDate::from_ymd(y, m, d).unwrap()
    }

    fn entry(kind: MediaKind, url: Option<&str>, hd: Option<&str>) -> Media {
        Media::new(kind, url.map(str::to_owned), hd.map(str::to_owned))
    }

    fn roles(targets: &[Target]) -> Vec<(&str, &str)> {
        targets
            .iter()
            .map(|target| (target.url.as_str(), target.role.as_str()))
            .collect()
    }

    #[test]
    fn the_picture_is_the_best_size_the_page_offers() {
        let entries = vec![(
            date(2024, 3, 5),
            entry(
                MediaKind::ImageJpg,
                Some("https://apod.nasa.gov/apod/image/2403/small.jpg"),
                Some("https://apod.nasa.gov/apod/image/2403/big.jpg"),
            ),
        )];

        assert_eq!(
            roles(&targets(&entries, &[])),
            [("https://apod.nasa.gov/apod/image/2403/big.jpg", "image")],
            "the displayed copy is a derivative of the linked one and is not worth a request"
        );
    }

    #[test]
    fn a_second_format_is_kept_but_a_second_size_is_not() {
        let differs = vec![(
            date(2024, 3, 5),
            entry(
                MediaKind::ImageGif,
                Some("https://apod.nasa.gov/apod/image/2403/anim.gif"),
                Some("https://apod.nasa.gov/apod/image/2403/still.jpg"),
            ),
        )];
        assert_eq!(
            roles(&targets(&differs, &[])),
            [
                ("https://apod.nasa.gov/apod/image/2403/still.jpg", "image"),
                (
                    "https://apod.nasa.gov/apod/image/2403/anim.gif",
                    "image_alt"
                ),
            ],
        );

        let same = vec![(
            date(2024, 3, 5),
            entry(
                MediaKind::ImageJpg,
                Some("https://apod.nasa.gov/apod/image/2403/small.jpeg"),
                Some("https://apod.nasa.gov/apod/image/2403/big.jpg"),
            ),
        )];
        assert_eq!(
            targets(&same, &[]).len(),
            1,
            "jpeg and jpg are one format and must not read as two"
        );
    }

    #[test]
    fn a_url_is_archived_once_under_the_earliest_date_that_shows_it() {
        let shared = "https://apod.nasa.gov/apod/image/0208/earthlights.jpg";
        let entries = vec![
            (
                date(2006, 10, 5),
                entry(MediaKind::ImageJpg, None, Some(shared)),
            ),
            (
                date(2002, 8, 12),
                entry(MediaKind::ImageJpg, None, Some(shared)),
            ),
        ];

        for entries in [entries.clone(), entries.into_iter().rev().collect()] {
            let picked = targets(&entries, &[]);
            assert_eq!(picked.len(), 1, "one url is one file");
            assert_eq!(picked[0].date, date(2002, 8, 12));
            assert_eq!(
                stored_path(picked[0].date, "earthlights.jpg"),
                "2002/08/2002-08-12/earthlights.jpg"
            );
        }
    }

    #[test]
    fn the_legacy_copy_of_a_migrated_picture_is_archived_too() {
        let origin = "https://assets.science.nasa.gov/content/dam/x_4000.jpg";
        let legacy = "https://apod.nasa.gov/apod/image/2608/x_960.jpg";
        let entries = vec![(
            date(2026, 8, 25),
            entry(MediaKind::ImageJpg, None, Some(origin)),
        )];
        let pairs = vec![(date(2026, 8, 25), legacy.to_owned(), origin.to_owned())];

        let picked = targets(&entries, &pairs);
        let urls: Vec<&str> = picked.iter().map(|t| t.url.as_str()).collect();
        assert!(
            urls.contains(&origin) && urls.contains(&legacy),
            "both sides of the pair must be fetched or the decision 8 check has nothing to \
             compare: {urls:?}"
        );
        assert_eq!(
            picked
                .iter()
                .find(|t| t.url == legacy)
                .map(|t| t.role.as_str()),
            Some("legacy_image"),
            "the legacy copy is told apart from the one the entry points at"
        );

        let repeated = targets(&entries, &[pairs[0].clone(), pairs[0].clone()]);
        assert_eq!(
            repeated.len(),
            picked.len(),
            "one URL is one file however many pairs name it"
        );
    }

    #[test]
    fn videos_are_told_apart_from_the_platforms_that_host_them() {
        let entries = vec![
            (
                date(2024, 3, 5),
                entry(
                    MediaKind::VideoMp4,
                    Some("https://apod.nasa.gov/apod/image/2403/clip.mp4"),
                    None,
                ),
            ),
            (
                date(2024, 3, 6),
                entry(
                    MediaKind::YouTube,
                    Some("https://www.youtube.com/embed/abc123"),
                    None,
                ),
            ),
            (
                date(2024, 3, 7),
                entry(
                    MediaKind::Vimeo,
                    Some("https://player.vimeo.com/video/98765"),
                    None,
                ),
            ),
            (
                date(2024, 3, 8),
                entry(MediaKind::Embed, Some("https://example.com/game.php"), None),
            ),
            (date(2024, 3, 9), entry(MediaKind::None, None, None)),
        ];

        assert_eq!(
            roles(&targets(&entries, &[])),
            [
                ("https://apod.nasa.gov/apod/image/2403/clip.mp4", "video"),
                ("https://www.youtube.com/embed/abc123", "poster"),
                ("https://player.vimeo.com/video/98765", "poster"),
            ],
            "an interactive embed is not a picture and there is nothing to store for it"
        );
    }

    #[test]
    fn the_host_says_which_side_of_the_archive_a_file_came_from() {
        assert_eq!(
            source_of("https://assets.science.nasa.gov/content/dam/x/y.jpg"),
            Source::Modern
        );
        assert_eq!(
            source_of("https://apod.nasa.gov/apod/image/2403/m31.jpg"),
            Source::Legacy
        );
        assert_eq!(
            source_of("https://www.youtube.com/embed/abc123"),
            Source::Legacy,
            "an embed the legacy pages have always pointed at is not part of the migration"
        );
    }

    #[test]
    fn files_keep_nasas_own_name_under_a_date_directory() {
        let url = "https://apod.nasa.gov/apod/image/0001/fm1222_gendler_big.jpg";
        assert_eq!(
            stored_path(date(2000, 1, 13), &file_name(url, &[url.to_owned()])),
            "2000/01/2000-01-13/fm1222_gendler_big.jpg"
        );
    }

    #[test]
    fn two_urls_that_would_share_a_name_both_get_a_hash() {
        let one = "https://apod.nasa.gov/apod/image/2403/m31.jpg";
        let two = "https://apod.nasa.gov/apod/image/9911/m31.jpg";
        let both = vec![one.to_owned(), two.to_owned()];

        let (first, second) = (file_name(one, &both), file_name(two, &both));
        assert_ne!(first, second, "neither may overwrite the other");
        assert!(
            first.starts_with("m31-") && first.ends_with(".jpg"),
            "{first}"
        );
        assert!(
            second.starts_with("m31-") && second.ends_with(".jpg"),
            "{second}"
        );

        assert_eq!(
            file_name(one, &[one.to_owned()]),
            "m31.jpg",
            "the name is only decorated when it actually collides"
        );
    }

    #[test]
    fn a_served_name_with_no_extension_gets_the_one_the_bytes_imply() {
        let vimeo = "https://i.vimeocdn.com/video/288600431-4814bb9b-d_295x166";
        assert_eq!(
            named(vimeo, &[], Format::Jpeg),
            "288600431-4814bb9b-d_295x166.jpg",
            "an archive meant to outlive its sources should not hold files nothing will open"
        );

        let nasa = "https://apod.nasa.gov/apod/image/2403/m31.jpg";
        assert_eq!(
            named(nasa, &[], Format::Jpeg),
            "m31.jpg",
            "a name the server already gave is left exactly as it was served"
        );
        assert_eq!(
            named(
                "https://apod.nasa.gov/apod/image/2403/m31.gif",
                &[],
                Format::Jpeg
            ),
            "m31.gif",
            "and it is not corrected either, even when the bytes disagree with it"
        );
    }

    #[test]
    fn a_url_with_no_filename_still_lands_somewhere() {
        let name = file_name("https://example.com/", &[]);
        assert!(name.ends_with(".bin"), "{name}");
        assert!(!name.contains('/'), "{name}");
    }

    #[test]
    fn an_error_page_served_with_200_is_not_a_picture() {
        let html = b"<!DOCTYPE html><html><body>Not Found</body></html>";
        let reason = verified(html, Role::Image).unwrap_err();
        assert!(reason.contains("not a picture"), "{reason}");
    }

    #[test]
    fn the_bytes_have_to_be_the_kind_of_thing_the_entry_says_they_are() {
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0];
        let mp4 = *b"\0\0\0\x18ftypmp42";

        assert_eq!(verified(&jpeg, Role::Image), Ok(Format::Jpeg));
        assert_eq!(verified(&jpeg, Role::ImageAlt), Ok(Format::Jpeg));
        assert_eq!(verified(&jpeg, Role::Poster), Ok(Format::Jpeg));
        assert_eq!(verified(&mp4, Role::Video), Ok(Format::Mp4));

        assert!(
            verified(&mp4, Role::Image).is_err(),
            "a video is not the picture"
        );
        assert!(
            verified(&jpeg, Role::Video).is_err(),
            "a still is not the video"
        );
    }

    #[test]
    fn every_format_the_archive_holds_is_recognised() {
        assert_eq!(sniff(b"GIF89a...."), Some(Format::Gif));
        assert_eq!(sniff(b"GIF87a...."), Some(Format::Gif));
        assert_eq!(
            sniff(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
            Some(Format::Png)
        );
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some(Format::WebP));
        assert_eq!(sniff(b"not anything at all"), None);
        assert_eq!(sniff(b""), None);
    }

    fn record(status: Option<u16>, attempts: u32) -> Attempted {
        Attempted {
            stored: false,
            http_status: status,
            attempts,
            last_checked_at: 0,
        }
    }

    #[test]
    fn only_the_servers_own_answer_settles_a_url() {
        assert!(record(Some(404), 1).settled(), "the file is not there");
        assert!(record(Some(410), 1).settled(), "and it is not coming back");
        assert!(
            record(Some(301), 1).settled(),
            "a redirect answered the question"
        );

        assert!(
            !record(Some(500), 99).settled(),
            "a server having a bad day never becomes proof the file is gone, however long the \
             bad day runs"
        );
        assert!(
            !record(None, 99).settled(),
            "and neither does a download that never connected or was cut off partway"
        );
        assert!(
            !record(Some(200), 99).settled(),
            "a 200 that was not the picture is worth asking about again: an error page from a \
             cache is exactly what an outage looks like"
        );
    }

    #[test]
    fn a_host_that_is_not_answering_is_asked_less_and_less_often() {
        let ceiling = Duration::from_secs(6 * 3600);
        let first = record(None, 1).due_in(ceiling, 0);
        let fifth = record(None, 5).due_in(ceiling, 0);

        assert!(
            fifth > first,
            "each failure widens the wait, so an outage is not hammered: {first} then {fifth}"
        );
        assert!(
            record(None, 40).due_in(ceiling, 0) <= ceiling.as_secs() as i64,
            "and the wait stops widening rather than running away"
        );
        assert!(
            record(None, 1).due_in(ceiling, 10_000) < 0,
            "once the wait has passed it is due again"
        );
    }

    const CEILING: Duration = Duration::from_secs(6 * 3600);

    async fn store() -> MediaStore {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir()
            .join(format!(
                "apod-media-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ))
            .join("archive.db");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());

        crate::archive::ArchiveStore::open(&path).await.unwrap();
        MediaStore::new(Db::open(DbConfig::read_write(&path)).await.unwrap())
    }

    fn target(url: &str, date: ApodDate) -> Target {
        Target {
            url: url.to_owned(),
            date,
            role: Role::Image,
        }
    }

    #[tokio::test]
    async fn the_index_is_the_worklist_and_a_stored_file_leaves_it() {
        let store = store().await;
        let older = target("https://apod.nasa.gov/a.jpg", date(1995, 6, 16));
        let newer = target("https://apod.nasa.gov/b.jpg", date(1995, 6, 20));
        let queue = vec![older.clone(), newer.clone()];

        assert_eq!(
            store.next_target(&queue, CEILING, 0).await.unwrap(),
            Next::Fetch(newer.clone())
        );

        store
            .record_stored(
                &newer,
                "1995/06/1995-06-20/b.jpg",
                "hash",
                8192,
                "image/jpeg",
                1,
            )
            .await
            .unwrap();

        assert_eq!(
            store.next_target(&queue, CEILING, 0).await.unwrap(),
            Next::Fetch(older.clone())
        );
        assert_eq!(
            store.stored_path(&newer.url).await.unwrap().as_deref(),
            Some("1995/06/1995-06-20/b.jpg")
        );

        store
            .record_stored(
                &older,
                "1995/06/1995-06-16/a.jpg",
                "hash",
                4096,
                "image/jpeg",
                1,
            )
            .await
            .unwrap();
        assert_eq!(
            store.next_target(&queue, CEILING, 0).await.unwrap(),
            Next::Complete
        );

        let counts = store.counts().await.unwrap();
        assert_eq!((counts.stored, counts.missing, counts.failed), (2, 0, 0));
        assert_eq!(counts.bytes, 12_288);
    }

    #[tokio::test]
    async fn todays_picture_does_not_queue_behind_thirty_years_of_backlog() {
        let store = store().await;
        let backlog: Vec<Target> = (0..200)
            .map(|day| {
                target(
                    &format!("https://apod.nasa.gov/old{day}.jpg"),
                    ApodDate::from_days(day),
                )
            })
            .collect();
        let today = target("https://apod.nasa.gov/today.jpg", date(2026, 8, 31));

        let mut queue = backlog.clone();
        queue.push(today.clone());

        assert_eq!(
            store.next_target(&queue, CEILING, 0).await.unwrap(),
            Next::Fetch(today),
            "the archive walks back from today, so a new entry is never behind the whole archive"
        );
    }

    #[tokio::test]
    async fn a_file_that_is_gone_is_never_asked_for_again() {
        let store = store().await;
        let gone = target("https://apod.nasa.gov/gone.jpg", date(1995, 6, 16));
        let queue = vec![gone.clone()];

        store
            .record_failure(&gone, Some(404), "not there", 1)
            .await
            .unwrap();

        assert_eq!(
            store.next_target(&queue, CEILING, 10_000).await.unwrap(),
            Next::Complete
        );
        assert_eq!(store.counts().await.unwrap().missing, 1);
    }

    #[tokio::test]
    async fn a_failing_url_backs_off_rather_than_being_asked_every_few_seconds() {
        let store = store().await;
        let flaky = target("https://apod.nasa.gov/flaky.jpg", date(1995, 6, 16));
        let queue = vec![flaky.clone()];

        store
            .record_failure(&flaky, Some(500), "boom", 0)
            .await
            .unwrap();

        assert!(
            matches!(
                store.next_target(&queue, CEILING, 10).await.unwrap(),
                Next::Waiting(_)
            ),
            "one broken url must not become the only candidate and hold the front of the queue"
        );
        assert_eq!(
            store.next_target(&queue, CEILING, 10_000).await.unwrap(),
            Next::Fetch(flaky.clone()),
            "but once the wait has passed it is asked again"
        );

        store
            .record_failure(&flaky, Some(500), "boom", 10_000)
            .await
            .unwrap();
        assert!(
            matches!(
                store.next_target(&queue, CEILING, 10_060).await.unwrap(),
                Next::Waiting(_)
            ),
            "and the second failure buys a longer wait than the first"
        );
        assert_eq!(store.counts().await.unwrap().failed, 1);
    }

    #[tokio::test]
    async fn an_outage_never_turns_into_work_quietly_abandoned() {
        let store = store().await;
        let wanted = target("https://apod.nasa.gov/wanted.jpg", date(2026, 8, 31));
        let queue = vec![wanted.clone()];

        // A host that is down answers the same way a hundred times running. None of those
        // answers say the file is not there, so none of them may end the queue.
        for attempt in 0..100 {
            store
                .record_failure(&wanted, None, "connection reset", attempt * 100_000)
                .await
                .unwrap();
        }

        assert_eq!(
            store
                .next_target(&queue, CEILING, 100 * 100_000)
                .await
                .unwrap(),
            Next::Fetch(wanted),
            "when the host comes back the file is still owed, however long it was down"
        );
    }

    #[tokio::test]
    async fn a_retry_waits_behind_everything_that_has_not_been_tried_yet() {
        let store = store().await;
        let stale = target("https://apod.nasa.gov/stale.jpg", date(1995, 6, 16));
        let recent = target("https://apod.nasa.gov/recent.jpg", date(1995, 6, 20));
        let fresh = target("https://apod.nasa.gov/fresh.jpg", date(1995, 6, 21));
        let queue = vec![stale.clone(), recent.clone(), fresh.clone()];

        store
            .record_failure(&stale, Some(500), "boom", 10)
            .await
            .unwrap();
        store
            .record_failure(&recent, Some(500), "boom", 20)
            .await
            .unwrap();

        assert_eq!(
            store.next_target(&queue, CEILING, 10_000).await.unwrap(),
            Next::Fetch(fresh.clone()),
            "work that has never been attempted comes first"
        );

        store
            .record_stored(
                &fresh,
                "1995/06/1995-06-21/fresh.jpg",
                "h",
                1,
                "image/jpeg",
                30,
            )
            .await
            .unwrap();

        assert_eq!(
            store.next_target(&queue, CEILING, 10_000).await.unwrap(),
            Next::Fetch(stale),
            "then, among the ones now due, the one asked about longest ago"
        );
    }
}
