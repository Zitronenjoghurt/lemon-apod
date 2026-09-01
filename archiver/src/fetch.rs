use crate::archive::{ArchiveStore, Failure, Source};
use crate::client::{Client, Response};
use crate::config::Config;
use anyhow::{Context, Result};
use apod_core::{ApodDate, ApodWriter};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Stored {
        bytes: usize,
    },
    Updated {
        bytes: usize,
    },
    Unchanged,
    Absent,
    Rejected(String),
    Redirected {
        status: u16,
        location: Option<String>,
    },
}

pub async fn fetch_and_store(
    cfg: &Config,
    client: &Client,
    archive: &ArchiveStore,
    index: &ApodWriter,
    date: ApodDate,
) -> Result<Outcome> {
    let url = cfg.page_url(date);
    let now = chrono::Utc::now().timestamp();

    let body = match client.get(&url).await {
        Ok(Response::Body(body)) => body,
        Ok(Response::NotFound) => {
            if !crate::workers::still_publishing(&cfg.daily, date, now) {
                archive
                    .record_failure(
                        date,
                        Source::Legacy,
                        &url,
                        Failure::new(Some(404), "not published"),
                        now,
                    )
                    .await?;
            }
            return Ok(Outcome::Absent);
        }
        Ok(Response::Redirected { status, location }) => {
            let reason = match &location {
                Some(target) => format!("redirected with {status} to {target}"),
                None => format!("redirected with {status} without a Location header"),
            };
            archive
                .record_failure(
                    date,
                    Source::Legacy,
                    &url,
                    Failure::new(Some(status), &reason).landed_on(location.as_deref()),
                    now,
                )
                .await?;
            tracing::warn!(
                %date,
                status,
                location = location.as_deref().unwrap_or("none"),
                "the source redirected; refusing to follow it, the archived copy is untouched"
            );
            return Ok(Outcome::Redirected { status, location });
        }
        Ok(Response::Refused { status }) => {
            let error = anyhow::anyhow!("{url} returned {status}");
            archive
                .record_failure(
                    date,
                    Source::Legacy,
                    &url,
                    Failure::new(Some(status), &format!("{error:#}")),
                    now,
                )
                .await?;
            return Err(error.context(format!("fetching {date}")));
        }
        Err(error) => {
            archive
                .record_failure(
                    date,
                    Source::Legacy,
                    &url,
                    Failure::new(None, &format!("{error:#}")),
                    now,
                )
                .await?;
            return Err(error.context(format!("fetching {date}")));
        }
    };

    if let Err(reason) = sanity_check(&body, cfg.fetch_min_bytes) {
        archive
            .record_failure(
                date,
                Source::Legacy,
                &url,
                Failure::new(Some(200), &reason),
                now,
            )
            .await?;
        tracing::warn!(%date, %reason, "refusing to store the response; the archived copy is untouched");
        return Ok(Outcome::Rejected(reason));
    }

    let digest = sha256(&body);
    let previous = archive.get(date, Source::Legacy).await?;
    let existed = previous.as_ref().is_some_and(|record| record.is_success());

    if previous.as_ref().and_then(|r| r.sha256.as_deref()) == Some(digest.as_str())
        && cfg.html_path(date).exists()
    {
        archive.touch(date, Source::Legacy, now).await?;
        return Ok(Outcome::Unchanged);
    }

    write_atomically(&cfg.html_path(date), &body)?;
    archive
        .record_success(date, Source::Legacy, &url, &digest, body.len(), now)
        .await?;

    crate::entry::reindex(cfg, index, date).await?;

    Ok(if existed {
        Outcome::Updated { bytes: body.len() }
    } else {
        Outcome::Stored { bytes: body.len() }
    })
}

fn sanity_check(body: &[u8], min_bytes: usize) -> Result<(), String> {
    if body.len() < min_bytes {
        return Err(format!(
            "response was {} bytes, below the {min_bytes} byte minimum",
            body.len()
        ));
    }

    let (text, _) = apod_core::decode::decode_html(body);
    let haystack = text.to_ascii_lowercase();
    let plausible = ["astronomy picture", "explanation", "apod"]
        .iter()
        .any(|marker| haystack.contains(marker));

    if plausible {
        Ok(())
    } else {
        Err("response contains no APOD markers".to_owned())
    }
}

pub fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    let mut temp = path.as_os_str().to_owned();
    temp.push(format!(
        ".{}-{}.tmp",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let temp = std::path::PathBuf::from(temp);

    std::fs::write(&temp, bytes).with_context(|| format!("writing {}", temp.display()))?;
    if let Err(error) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(&temp);
        return Err(error).with_context(|| format!("moving into {}", path.display()));
    }

    Ok(())
}

pub fn sha256(bytes: &[u8]) -> String {
    use std::fmt::Write;

    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN_BYTES: usize = 512;

    #[test]
    fn hashes_stay_64_lowercase_hex_digits() {
        assert_eq!(
            sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn accepts_a_real_looking_page() {
        let body = format!(
            "<html><title>APOD: 2024 March 5</title><body><b>Explanation:</b> {}</body></html>",
            "prose ".repeat(120)
        );
        assert!(sanity_check(body.as_bytes(), MIN_BYTES).is_ok());
    }

    #[test]
    fn rejects_a_truncated_response() {
        assert!(sanity_check(b"<html>", MIN_BYTES).is_err());
    }

    #[test]
    fn rejects_an_error_page_served_with_200() {
        let body = "x".repeat(2048);
        let reason = sanity_check(body.as_bytes(), MIN_BYTES).unwrap_err();
        assert!(reason.contains("no APOD markers"), "{reason}");
    }

    #[test]
    fn writes_atomically_leaving_no_temp_file() {
        let dir = std::env::temp_dir().join("apod-atomic-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("2024/03/2024-03-05.html");

        write_atomically(&path, b"hello").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello");

        write_atomically(&path, b"replaced").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"replaced");
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name != "2024-03-05.html")
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );

        let jpg = dir.join("2024/03/2024-03-05/x.jpg");
        let png = dir.join("2024/03/2024-03-05/x.png");
        write_atomically(&jpg, b"jpeg bytes").unwrap();
        write_atomically(&png, b"png bytes").unwrap();
        assert_eq!(std::fs::read(&jpg).unwrap(), b"jpeg bytes");
        assert_eq!(std::fs::read(&png).unwrap(), b"png bytes");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn two_writers_racing_for_one_path_do_not_pull_the_file_from_each_other() {
        let dir = std::env::temp_dir().join(format!("apod-atomic-race-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("2026/08/2026-08-28.html");

        std::thread::scope(|scope| {
            let writers: Vec<_> = (0..8)
                .map(|_| scope.spawn(|| write_atomically(&path, b"the same bytes")))
                .collect();
            for writer in writers {
                writer
                    .join()
                    .unwrap()
                    .expect("a second writer must not lose its temp file to the first");
            }
        });

        assert_eq!(std::fs::read(&path).unwrap(), b"the same bytes");
        let left = std::fs::read_dir(path.parent().unwrap()).unwrap().count();
        assert_eq!(
            left, 1,
            "every temp file should have been renamed or removed"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn a_redirect_records_the_target_and_leaves_the_page_on_disk() {
        use crate::client::Redirects;
        use apod_core::db::{Db, DbConfig};
        use std::time::Duration;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let _ = socket.read(&mut [0u8; 2048]).await;
                    let _ = socket
                        .write_all(
                            b"HTTP/1.1 301 Moved Permanently\r\n\
                              Location: https://science.nasa.gov/apod/\r\n\
                              Content-Length: 0\r\n\r\n",
                        )
                        .await;
                });
            }
        });

        let dir = std::env::temp_dir().join(format!("apod-redirect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let mut cfg = Config::from_env().unwrap();
        cfg.html_dir = dir.join("html");
        cfg.source_base_url = format!("http://{address}");

        let date = ApodDate::from_ymd(2026, 8, 25).unwrap();
        write_atomically(&cfg.html_path(date), b"the archived legacy page").unwrap();

        let archive = ArchiveStore::open(&dir.join("archive.db")).await.unwrap();
        let index = ApodWriter::open(&dir.join("apod.db")).await.unwrap();
        let client =
            Client::new("apod-test", Duration::from_secs(10), 0, Redirects::Refuse).unwrap();

        let outcome = fetch_and_store(&cfg, &client, &archive, &index, date)
            .await
            .unwrap();

        assert_eq!(
            outcome,
            Outcome::Redirected {
                status: 301,
                location: Some("https://science.nasa.gov/apod/".to_owned())
            }
        );
        assert_eq!(
            std::fs::read(cfg.html_path(date)).unwrap(),
            b"the archived legacy page"
        );

        let record = archive.get(date, Source::Legacy).await.unwrap().unwrap();
        assert_eq!(record.http_status, Some(301));

        let db = Db::open(DbConfig::read_only(dir.join("archive.db")))
            .await
            .unwrap();
        let (error, final_url): (String, Option<String>) = sqlx::query_as(
            "SELECT error, final_url FROM fetches WHERE date_id = ?1 AND source = 'legacy'",
        )
        .bind(date.days())
        .fetch_one(db.reader())
        .await
        .unwrap();
        assert!(error.contains("301"), "{error}");
        assert!(error.contains("science.nasa.gov/apod/"), "{error}");
        assert_eq!(final_url.as_deref(), Some("https://science.nasa.gov/apod/"));

        db.close().await;
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
