use crate::archive::ArchiveStore;
use crate::client::{Client, Response};
use crate::config::Config;
use crate::index::IndexStore;
use anyhow::{Context, Result};
use apod_core::{ApodDate, parse};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Stored { bytes: usize },
    Updated { bytes: usize },
    Unchanged,
    Absent,
    Rejected(String),
}

pub async fn fetch_and_store(
    cfg: &Config,
    client: &Client,
    archive: &mut ArchiveStore,
    index: &mut IndexStore,
    date: ApodDate,
) -> Result<Outcome> {
    let url = cfg.page_url(date);
    let now = chrono::Utc::now().timestamp();

    let body = match client.get(&url).await {
        Ok(Response::Body(body)) => body,
        Ok(Response::NotFound) => {
            archive.record_failure(date, &url, Some(404), "not published", now)?;
            return Ok(Outcome::Absent);
        }
        Err(error) => {
            archive.record_failure(date, &url, None, &format!("{error:#}"), now)?;
            return Err(error.context(format!("fetching {date}")));
        }
    };

    if let Err(reason) = sanity_check(&body, cfg.fetch_min_bytes) {
        archive.record_failure(date, &url, Some(200), &reason, now)?;
        tracing::warn!(%date, %reason, "refusing to store the response; the archived copy is untouched");
        return Ok(Outcome::Rejected(reason));
    }

    let digest = sha256(&body);
    let previous = archive.get(date)?;
    let existed = previous.as_ref().is_some_and(|record| record.is_success());

    if previous.as_ref().and_then(|r| r.sha256.as_deref()) == Some(digest.as_str())
        && cfg.html_path(date).exists()
    {
        archive.touch(date, now)?;
        return Ok(Outcome::Unchanged);
    }

    write_atomically(&cfg.html_path(date), &body)?;
    archive.record_success(date, &url, &digest, body.len(), now)?;

    match parse::parse_bytes(date, &body) {
        Ok(entry) => index.upsert(&entry)?,
        Err(error) => tracing::warn!(%date, "stored but could not parse: {error}"),
    }

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

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    let temp = path.with_extension("html.tmp");
    std::fs::write(&temp, bytes).with_context(|| format!("writing {}", temp.display()))?;
    std::fs::rename(&temp, path).with_context(|| format!("moving into {}", path.display()))?;

    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
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
        assert!(!path.with_extension("html.tmp").exists());

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
