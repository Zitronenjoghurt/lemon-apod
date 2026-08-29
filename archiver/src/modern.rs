use crate::archive::{ArchiveStore, Failure, Source};
use crate::client::{Client, Response};
use crate::config::Config;
use crate::fetch::{sha256, write_atomically};
use anyhow::{Context, Result, bail, ensure};
use apod_core::{ApodDate, ApodWriter, parse};
use serde::Deserialize;
use serde_json::value::RawValue;
use std::collections::HashSet;
use std::time::{Duration, Instant};

const KIND: &str = "image-article";

#[derive(Debug, Deserialize)]
struct Rendered {
    rendered: String,
}

#[derive(Debug, Deserialize)]
struct Header {
    id: u64,
    date: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    categories: Vec<u32>,
    title: Rendered,
    #[serde(default)]
    migration_source_url: String,
}

impl Header {
    fn is_apod(&self, category: u32) -> bool {
        self.kind == KIND && self.categories.contains(&category)
    }

    fn published(&self) -> Option<ApodDate> {
        self.date.get(..10)?.parse().ok()
    }

    fn migrated_from(&self) -> Option<ApodDate> {
        ApodDate::from_legacy_filename(&self.migration_source_url)
    }

    fn key(&self) -> Option<ApodDate> {
        self.migrated_from().or_else(|| self.published())
    }

    fn title_date(&self) -> Option<ApodDate> {
        parse::modern_title(&self.title.rendered).date
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    bound: ApodDate,
    daily: bool,
}

impl Window {
    pub fn back_from(bound: ApodDate) -> Self {
        Self {
            bound,
            daily: false,
        }
    }

    pub fn only(date: ApodDate) -> Self {
        Self {
            bound: date,
            daily: true,
        }
    }

    fn url(&self, cfg: &Config) -> String {
        let mut url = format!(
            "{}?categories={}&per_page={}&before={}T00:00:00",
            cfg.modern_api_url.trim_end_matches('/'),
            cfg.modern_category,
            cfg.modern_per_page,
            self.bound.next()
        );
        if self.daily {
            url.push_str(&format!("&after={}T00:00:00", self.bound.prev()));
        }
        url
    }

    fn covered(&self, oldest: ApodDate, short: bool) -> Option<(ApodDate, ApodDate)> {
        let low = match (self.daily, short) {
            (true, _) => self.bound,
            (false, true) => ApodDate::START,
            (false, false) => oldest.next(),
        };
        (low <= self.bound).then_some((low, self.bound))
    }
}

fn absent_in(
    covered: (ApodDate, ApodDate),
    present: &HashSet<ApodDate>,
    recorded: &HashSet<ApodDate>,
) -> Vec<ApodDate> {
    let (low, high) = covered;
    high.iter_desc()
        .take_while(|date| *date >= low)
        .filter(|date| !present.contains(date) && !recorded.contains(date))
        .collect()
}

pub fn delay(cfg: &Config, elapsed: Duration) -> Duration {
    elapsed
        .saturating_mul(cfg.modern_delay_multiplier)
        .max(cfg.modern_delay_min)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pass {
    pub records: usize,
    pub stored: usize,
    pub unchanged: usize,
    pub absent: usize,
    pub warned: usize,
    pub misfiled: usize,
    pub oldest: Option<ApodDate>,
    pub elapsed: Duration,
}

pub async fn fetch_window(
    cfg: &Config,
    client: &Client,
    archive: &ArchiveStore,
    index: &ApodWriter,
    window: Window,
) -> Result<Pass> {
    let url = window.url(cfg);
    let started = Instant::now();
    let response = client.get(&url).await;
    let elapsed = started.elapsed();

    let body = match response {
        Ok(Response::Body(body)) => body,
        Ok(Response::NotFound) => {
            bail!("{url} returned 404, so that is not the collection endpoint")
        }
        Ok(Response::Redirected { status, location }) => bail!(
            "{url} redirected with {status} to {}; refusing to follow it",
            location.as_deref().unwrap_or("nowhere")
        ),
        Ok(Response::Refused { status }) => bail!("{url} returned {status}"),
        Err(error) => return Err(error.context(format!("requesting {url}"))),
    };

    let raw: Vec<&RawValue> = serde_json::from_slice(&body)
        .with_context(|| format!("{url} did not answer with a list of records"))?;

    let mut pass = Pass {
        elapsed,
        ..Pass::default()
    };
    if raw.is_empty() {
        ensure!(
            window.daily,
            "{url} answered with no records at all, so the archive cannot be walked any further \
             back"
        );
        return Ok(pass);
    }

    let records: Vec<(Header, &RawValue)> = raw
        .iter()
        .filter_map(
            |element| match serde_json::from_str::<Header>(element.get()) {
                Ok(header) => Some((header, *element)),
                Err(error) => {
                    tracing::warn!("skipping a record that could not be read: {error}");
                    None
                }
            },
        )
        .collect();

    ensure!(
        records
            .iter()
            .any(|(header, _)| header.is_apod(cfg.modern_category) && header.title_date().is_some()),
        "{url} answered with {} records and not one of them is an APOD entry, so this is a \
         failure to record rather than an empty archive",
        raw.len()
    );

    let now = chrono::Utc::now().timestamp();
    let mut present: HashSet<ApodDate> = HashSet::with_capacity(records.len());
    let mut oldest: Option<ApodDate> = None;

    for (header, element) in &records {
        if !header.is_apod(cfg.modern_category) {
            tracing::warn!(
                id = header.id,
                kind = %header.kind,
                "skipping a record that is not an APOD entry"
            );
            pass.warned += 1;
            continue;
        }

        let (Some(published), Some(key)) = (header.published(), header.key()) else {
            tracing::warn!(
                id = header.id,
                date = %header.date,
                "skipping a record whose date field is not a date"
            );
            pass.warned += 1;
            continue;
        };
        oldest = Some(oldest.map_or(published, |seen: ApodDate| seen.min(published)));

        if key != published {
            tracing::warn!(
                id = header.id,
                %key,
                published = %published,
                source = %header.migration_source_url,
                "the migration filed this record under a publish date its own source page \
                 contradicts; the legacy filename is the key"
            );
            pass.warned += 1;
            pass.misfiled += 1;
        }

        match header.title_date() {
            Some(titled) if titled != key => {
                tracing::warn!(
                    id = header.id,
                    %key,
                    titled = %titled,
                    title = %header.title.rendered,
                    "the title names a different date than the record is filed under; filing it \
                     under the key"
                );
                pass.warned += 1;
            }
            None => {
                tracing::warn!(
                    id = header.id,
                    %key,
                    title = %header.title.rendered,
                    "the title carries no date to check the key against"
                );
                pass.warned += 1;
            }
            _ => {}
        }

        if !present.insert(key) {
            tracing::warn!(
                id = header.id,
                %key,
                title = %header.title.rendered,
                "a second record claims this date; keeping the first and storing neither of the \
                 two under another key"
            );
            pass.warned += 1;
            continue;
        }

        pass.records += 1;
        match store(
            cfg,
            archive,
            index,
            key,
            header.id,
            element.get().as_bytes(),
            now,
        )
        .await?
        {
            true => pass.stored += 1,
            false => pass.unchanged += 1,
        }
    }

    pass.oldest = oldest;

    let Some(oldest) = oldest else {
        return Ok(pass);
    };
    let Some(covered) = window.covered(oldest, raw.len() < cfg.modern_per_page as usize) else {
        return Ok(pass);
    };

    let recorded = archive
        .recorded_between(Source::Modern, covered.0, covered.1)
        .await?;
    for date in absent_in(covered, &present, &recorded) {
        archive
            .record_failure(
                date,
                Source::Modern,
                &url,
                Failure::new(
                    Some(404),
                    "the modern archive holds no record for this date",
                ),
                now,
            )
            .await?;
        pass.absent += 1;
    }

    Ok(pass)
}

async fn store(
    cfg: &Config,
    archive: &ArchiveStore,
    index: &ApodWriter,
    date: ApodDate,
    id: u64,
    bytes: &[u8],
    now: i64,
) -> Result<bool> {
    let url = format!("{}/{id}", cfg.modern_api_url.trim_end_matches('/'));
    let digest = sha256(bytes);
    let previous = archive.get(date, Source::Modern).await?;

    if previous.as_ref().and_then(|row| row.sha256.as_deref()) == Some(digest.as_str())
        && cfg.json_path(date).exists()
    {
        archive.touch(date, Source::Modern, now).await?;
        return Ok(false);
    }

    write_atomically(&cfg.json_path(date), bytes)?;
    archive
        .record_success(date, Source::Modern, &url, &digest, bytes.len(), now)
        .await?;
    crate::entry::reindex(cfg, index, date).await?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::Redirects;
    use apod_core::ApodWriter;
    use std::path::Path;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const MANHATTANHENGE: &str = r#"{"id": 1,
     "date":"2007-07-16T00:05:00","type":"image-article","categories":[22766],
     "migration_source_url":"https://apod.nasa.gov/apod/ap070713.html",
     "title":{"rendered":"APOD: 2007 July 16 - Manhattanhenge: A New York Sunset"}}"#;

    const LAGOON: &str = r#"{"id":2,"date":"2007-07-16T00:06:00","type":"image-article",
     "categories":[22766],
     "migration_source_url":"https://apod.nasa.gov/apod/ap070716.html",
     "title":{"rendered":"APOD: 2007 July 16 – The Lagoon Nebula in Gas, Dust, and Stars"}}"#;

    const NATIVE: &str = r#"{"id":3,"date":"2026-08-26T00:05:00","type":"image-article",
     "categories":[22766],"migration_source_url":"",
     "title":{"rendered":"APOD: 2026 August 26 – Published Here, Never Migrated"}}"#;

    fn date(y: i32, m: u32, d: u32) -> ApodDate {
        ApodDate::from_ymd(y, m, d).unwrap()
    }

    fn temp(name: &str) -> std::path::PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "apod-modern-{name}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    async fn serving(body: String) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let body = body.clone();
                tokio::spawn(async move {
                    let _ = socket.read(&mut [0u8; 4096]).await;
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(body.as_bytes()).await;
                });
            }
        });

        format!("http://{address}/wp-json/wp/v2/image-article")
    }

    fn config(dir: &Path, api: &str, per_page: u32) -> Config {
        let mut cfg = Config::from_env().unwrap();
        cfg.json_dir = dir.join("json");
        cfg.modern_api_url = api.to_owned();
        cfg.modern_per_page = per_page;
        cfg
    }

    fn client() -> Client {
        Client::new("apod-test", Duration::from_secs(10), 0, Redirects::Refuse).unwrap()
    }

    fn header(json: &str) -> Header {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn the_legacy_filename_the_record_carries_is_the_key() {
        let lagoon = header(LAGOON);
        assert_eq!(lagoon.key(), Some(date(2007, 7, 16)));
        assert_eq!(lagoon.published(), Some(date(2007, 7, 16)));
        assert_eq!(lagoon.title_date(), Some(date(2007, 7, 16)));
    }

    #[test]
    fn a_record_the_migration_misfiled_lands_on_its_own_date_anyway() {
        let manhattanhenge = header(MANHATTANHENGE);
        assert_eq!(
            manhattanhenge.key(),
            Some(date(2007, 7, 13)),
            "ap070713.html is the only authoritative key this record ever had"
        );
        assert_eq!(
            manhattanhenge.published(),
            Some(date(2007, 7, 16)),
            "and reading it does not rewrite what the api actually said"
        );
        assert_ne!(
            manhattanhenge.key(),
            manhattanhenge.published(),
            "which is the disagreement worth reporting upstream"
        );
    }

    #[test]
    fn a_record_published_since_the_migration_is_keyed_by_its_own_date() {
        let native = header(NATIVE);
        assert_eq!(native.migrated_from(), None);
        assert_eq!(native.key(), Some(date(2026, 8, 26)));
    }

    #[test]
    fn a_source_url_that_is_not_a_legacy_page_falls_back_to_the_date() {
        for source in [
            "https://apod.nasa.gov/apod/calendar/allyears.html",
            "https://apod.nasa.gov/apod/ap07071.html",
            "https://apod.nasa.gov/apod/apnotadate.html",
            "not a url at all",
        ] {
            let raw = format!(
                r#"{{"id":4,"date":"2007-07-16T00:05:00","type":"image-article",
                    "categories":[22766],"migration_source_url":"{source}",
                    "title":{{"rendered":"APOD: 2007 July 16 - A Title"}}}}"#
            );
            let parsed = header(&raw);
            assert_eq!(parsed.migrated_from(), None, "{source}");
            assert_eq!(
                parsed.key(),
                Some(date(2007, 7, 16)),
                "an unreadable filename is not a reason to lose the record: {source}"
            );
        }
    }

    #[test]
    fn the_century_is_read_the_way_apod_spans_it() {
        let key = |name: &str| {
            header(&format!(
                r#"{{"id":5,"date":"2000-01-01T00:05:00","type":"image-article",
                    "categories":[22766],
                    "migration_source_url":"https://apod.nasa.gov/apod/{name}",
                    "title":{{"rendered":"APOD: 2000 January 1 - A Title"}}}}"#
            ))
            .migrated_from()
        };

        assert_eq!(
            key("ap950616.html"),
            Some(ApodDate::START),
            "the first entry"
        );
        assert_eq!(key("ap991231.html"), Some(date(1999, 12, 31)));
        assert_eq!(key("ap000101.html"), Some(date(2000, 1, 1)));
        assert_eq!(key("ap260826.html"), Some(date(2026, 8, 26)));
    }

    #[test]
    fn a_record_qualifies_only_as_an_apod_entry_in_the_apod_category() {
        assert!(header(LAGOON).is_apod(22766));
        assert!(!header(LAGOON).is_apod(99));
        assert!(
            !header(
                r#"{"id":4,"date":"2026-08-26T00:05:00","type":"news-article","categories":[22766],
                    "title":{"rendered":"APOD: 2026 August 26 - Not An Image Article"}}"#
            )
            .is_apod(22766)
        );
    }

    #[test]
    fn a_window_asks_for_the_bound_and_nothing_newer() {
        let cfg = config(
            &temp("url"),
            "https://example.test/wp/v2/image-article",
            100,
        );

        assert_eq!(
            Window::back_from(date(2023, 2, 13)).url(&cfg),
            "https://example.test/wp/v2/image-article?categories=22766&per_page=100\
             &before=2023-02-14T00:00:00",
            "records are stamped at midnight and `before` is exclusive, so the day after the \
             bound is what includes the bound; the day after that re-fetches a stored date"
        );

        assert_eq!(
            Window::only(date(2023, 2, 13)).url(&cfg),
            "https://example.test/wp/v2/image-article?categories=22766&per_page=100\
             &before=2023-02-14T00:00:00&after=2023-02-12T00:00:00",
            "the daily window brackets its own date from both sides"
        );
    }

    #[test]
    fn a_window_never_concludes_about_the_oldest_date_it_returned() {
        let window = Window::back_from(date(1995, 6, 25));
        assert_eq!(
            window.covered(date(1995, 6, 21), false),
            Some((date(1995, 6, 22), date(1995, 6, 25))),
            "a page boundary can cut the oldest date's records in two"
        );
        assert_eq!(
            window.covered(date(1995, 6, 21), true),
            Some((ApodDate::START, date(1995, 6, 25))),
            "a short page is the end of the archive, so everything older is evidence too"
        );
        assert_eq!(
            window.covered(date(1995, 6, 25), false),
            None,
            "a page holding one date says nothing about any other"
        );
    }

    #[test]
    fn the_daily_window_speaks_for_its_own_date_alone() {
        let window = Window::only(date(2026, 8, 26));
        assert_eq!(
            window.covered(date(2026, 8, 25), true),
            Some((date(2026, 8, 26), date(2026, 8, 26))),
            "a narrow request is short by construction and proves nothing about the archive"
        );
    }

    #[test]
    fn absence_is_the_gap_between_what_came_back_and_what_is_already_known() {
        let present = HashSet::from([date(1995, 6, 25), date(1995, 6, 22)]);
        let recorded = HashSet::from([date(1995, 6, 24)]);

        assert_eq!(
            absent_in((ApodDate::START, date(1995, 6, 25)), &present, &recorded),
            vec![
                date(1995, 6, 23),
                date(1995, 6, 21),
                date(1995, 6, 20),
                ApodDate::START
            ],
            "the known gaps are skipped and a date already recorded is left alone"
        );
        assert!(absent_in((date(1995, 6, 22), date(1995, 6, 22)), &present, &recorded).is_empty());
    }

    #[test]
    fn the_wait_grows_with_the_response_but_never_drops_below_the_floor() {
        let dir = temp("delay");
        let mut cfg = config(&dir, "https://example.com/api", 100);
        cfg.modern_delay_multiplier = 10;
        cfg.modern_delay_min = Duration::from_secs(60);

        assert_eq!(cfg_delay(&cfg, 1), Duration::from_secs(60));
        assert_eq!(cfg_delay(&cfg, 6), Duration::from_secs(60));
        assert_eq!(cfg_delay(&cfg, 9), Duration::from_secs(90));
    }

    fn cfg_delay(cfg: &Config, seconds: u64) -> Duration {
        delay(cfg, Duration::from_secs(seconds))
    }

    #[tokio::test]
    async fn the_bytes_on_disk_are_the_bytes_that_arrived() {
        const ELEMENT: &str = r#"{"id": 7,   "title":{"rendered":"APOD: 2026 August 26 – A Title"},
   "type":"image-article","date":"2026-08-26T00:05:00","categories":[22766],
   "odd":"a\"b\\c\/d","nested":{"deep":[1,2,3]}}"#;

        let dir = temp("bytes");
        let api = serving(format!("[\n  {ELEMENT}\n]")).await;
        let cfg = config(&dir, &api, 1);
        let archive = ArchiveStore::open(&dir.join("archive.db")).await.unwrap();
        let index = ApodWriter::open(&dir.join("apod.db")).await.unwrap();
        let day = date(2026, 8, 26);

        let pass = fetch_window(&cfg, &client(), &archive, &index, Window::back_from(day))
            .await
            .unwrap();
        assert_eq!((pass.records, pass.stored, pass.absent), (1, 1, 0));

        assert_eq!(
            std::fs::read(cfg.json_path(day)).unwrap(),
            ELEMENT.as_bytes(),
            "re-serialising would reorder keys and reformat, and nothing downstream would notice"
        );

        let record = archive.get(day, Source::Modern).await.unwrap().unwrap();
        assert!(record.is_success());
        assert_eq!(
            record.sha256,
            Some(crate::fetch::sha256(ELEMENT.as_bytes()))
        );

        let again = fetch_window(&cfg, &client(), &archive, &index, Window::back_from(day))
            .await
            .unwrap();
        assert_eq!(
            (again.stored, again.unchanged),
            (0, 1),
            "a record already on disk is not written again"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn both_records_that_claim_one_day_are_kept_apart() {
        let dir = temp("misfiled");
        let api = serving(format!("[{MANHATTANHENGE},{LAGOON}]")).await;
        let cfg = config(&dir, &api, 2);
        let archive = ArchiveStore::open(&dir.join("archive.db")).await.unwrap();
        let index = ApodWriter::open(&dir.join("apod.db")).await.unwrap();

        let pass = fetch_window(
            &cfg,
            &client(),
            &archive,
            &index,
            Window::back_from(date(2007, 7, 16)),
        )
        .await
        .unwrap();
        assert_eq!(
            pass.records, 2,
            "neither record may be lost to the collision"
        );
        assert_eq!(
            pass.misfiled, 1,
            "one of the two contradicts the publish date it was given"
        );
        assert_eq!(
            pass.warned, 2,
            "that record is both misfiled and titled with the wrong date"
        );

        for (day, marker) in [
            (date(2007, 7, 13), "Manhattanhenge"),
            (date(2007, 7, 16), "Lagoon"),
        ] {
            let stored = std::fs::read_to_string(cfg.json_path(day)).unwrap();
            assert!(stored.contains(marker), "{day} holds {stored}");
            assert!(
                archive
                    .get(day, Source::Modern)
                    .await
                    .unwrap()
                    .unwrap()
                    .is_success()
            );
        }

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn an_empty_answer_stops_the_backfill_and_settles_nothing() {
        let dir = temp("empty");
        let api = serving("[]".to_owned()).await;
        let cfg = config(&dir, &api, 5);
        let archive = ArchiveStore::open(&dir.join("archive.db")).await.unwrap();
        let index = ApodWriter::open(&dir.join("apod.db")).await.unwrap();
        let day = date(1995, 6, 25);

        let refused = fetch_window(&cfg, &client(), &archive, &index, Window::back_from(day))
            .await
            .unwrap_err();
        assert!(
            format!("{refused:#}").contains("no records at all"),
            "{refused:#}"
        );
        assert_eq!(archive.counts(Source::Modern).await.unwrap().absent, 0);

        let quiet = fetch_window(&cfg, &client(), &archive, &index, Window::only(day))
            .await
            .expect("a day APOD skipped is not a broken endpoint");
        assert_eq!((quiet.records, quiet.absent), (0, 0));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn a_page_holding_no_apod_entry_records_nothing_at_all() {
        let dir = temp("refused");
        let api = serving(
            r#"[{"id":9,"date":"2026-08-26T00:05:00","type":"news-article","categories":[123],
                 "title":{"rendered":"Something From The Rest Of The Site"}}]"#
                .to_owned(),
        )
        .await;
        let cfg = config(&dir, &api, 5);
        let archive = ArchiveStore::open(&dir.join("archive.db")).await.unwrap();
        let index = ApodWriter::open(&dir.join("apod.db")).await.unwrap();
        let day = date(2026, 8, 26);

        let refused = fetch_window(&cfg, &client(), &archive, &index, Window::back_from(day))
            .await
            .unwrap_err();
        assert!(
            format!("{refused:#}").contains("failure to record"),
            "{refused:#}"
        );

        assert!(archive.get(day, Source::Modern).await.unwrap().is_none());
        assert_eq!(
            archive.counts(Source::Modern).await.unwrap().absent,
            0,
            "a short page nobody could read is not evidence that the archive is empty"
        );
        assert!(!cfg.json_path(day).exists());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn a_date_the_migration_never_carried_across_is_settled_not_retried() {
        let dir = temp("absent");
        let api = serving(format!(
            "[{}]",
            r#"{"id":11,"date":"1995-06-22T00:05:00","type":"image-article","categories":[22766],
                "title":{"rendered":"APOD: 1995 June 22 - An Early Entry"}}"#
        ))
        .await;
        let cfg = config(&dir, &api, 5);
        let archive = ArchiveStore::open(&dir.join("archive.db")).await.unwrap();
        let index = ApodWriter::open(&dir.join("apod.db")).await.unwrap();
        let today = date(1995, 6, 25);

        let pass = fetch_window(&cfg, &client(), &archive, &index, Window::back_from(today))
            .await
            .unwrap();
        assert_eq!(pass.stored, 1);
        assert_eq!(
            pass.absent, 6,
            "every date the short page covered and did not hold, the known gaps aside"
        );

        for day in [today, date(1995, 6, 24), date(1995, 6, 23), ApodDate::START] {
            let record = archive.get(day, Source::Modern).await.unwrap().unwrap();
            assert!(record.is_absent(), "{day}");
        }
        assert_eq!(
            archive
                .next_target(
                    today,
                    Source::Modern,
                    Duration::from_secs(3600),
                    i64::MAX / 2
                )
                .await
                .unwrap(),
            crate::archive::Next::Complete,
            "a date the modern archive does not hold is not owed another request forever"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
