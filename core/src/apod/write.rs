use super::read::{ApodReader, ApodResult, to_dates};
use crate::PARSER_VERSION;
use crate::date::ApodDate;
use crate::db::{Db, DbConfig};
use crate::entry::ApodEntry;
use crate::media::Media;
use sqlx::migrate::Migrator;
use sqlx::{Row, Sqlite, Transaction};
use std::path::Path;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Clone)]
pub struct ApodWriter {
    reader: ApodReader,
}

impl ApodWriter {
    pub async fn open(path: &Path) -> ApodResult<Self> {
        let db = Db::open(DbConfig::read_write(path)).await?;
        db.migrate(&MIGRATIONS).await?;
        Ok(Self {
            reader: ApodReader::from_db(db),
        })
    }

    pub fn reader(&self) -> &ApodReader {
        &self.reader
    }

    fn db(&self) -> &Db {
        self.reader.db()
    }

    pub async fn upsert_all(&self, entries: &[ApodEntry]) -> ApodResult<()> {
        let mut tx = self.db().writer()?.begin().await?;

        for entry in entries {
            write_entry(&mut tx, entry).await?;
        }

        sqlx::query(
            "INSERT INTO meta (key, value) VALUES ('parser_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(PARSER_VERSION.to_string())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn upsert(&self, entry: &ApodEntry) -> ApodResult<()> {
        self.upsert_all(std::slice::from_ref(entry)).await
    }

    pub async fn set_thumb(&self, date: ApodDate, thumb_path: Option<&str>) -> ApodResult<()> {
        sqlx::query("UPDATE entries SET thumb_path = ?2 WHERE date_id = ?1")
            .bind(date.days())
            .bind(thumb_path)
            .execute(self.db().writer()?)
            .await?;
        Ok(())
    }

    pub async fn stale_dates(&self) -> ApodResult<Vec<ApodDate>> {
        let days: Vec<i64> = sqlx::query_scalar(
            "SELECT date_id FROM entries WHERE parser_version < ?1 ORDER BY date_id DESC",
        )
        .bind(PARSER_VERSION)
        .fetch_all(self.db().reader())
        .await?;
        Ok(to_dates(days))
    }

    pub async fn missing_thumbs(&self) -> ApodResult<Vec<(ApodDate, Media)>> {
        let rows = sqlx::query(
            "SELECT date_id, media_kind, media_url, media_hd_url FROM entries
             WHERE thumb_path IS NULL
               AND media_kind IN ('image_jpg', 'image_png', 'image_gif',
                                  'video_mp4', 'youtube', 'vimeo')
             ORDER BY date_id DESC",
        )
        .fetch_all(self.db().reader())
        .await?;

        rows.iter()
            .map(|row| {
                Ok((
                    ApodDate::from_days(row.try_get::<i64, _>(0)? as i32),
                    self.reader.media(
                        &row.try_get::<String, _>(1)?,
                        row.try_get(2)?,
                        row.try_get(3)?,
                        None,
                    ),
                ))
            })
            .collect()
    }

    pub async fn media_for(&self, dates: &[ApodDate]) -> ApodResult<Vec<(ApodDate, Media)>> {
        let mut out = Vec::with_capacity(dates.len());

        for &date in dates {
            let row = sqlx::query(
                "SELECT media_kind, media_url, media_hd_url FROM entries WHERE date_id = ?1",
            )
            .bind(date.days())
            .fetch_optional(self.db().reader())
            .await?;

            if let Some(row) = row {
                out.push((
                    date,
                    self.reader.media(
                        &row.try_get::<String, _>(0)?,
                        row.try_get(1)?,
                        row.try_get(2)?,
                        None,
                    ),
                ));
            }
        }

        Ok(out)
    }
}

async fn write_entry(tx: &mut Transaction<'_, Sqlite>, entry: &ApodEntry) -> ApodResult<()> {
    let keywords = (!entry.keywords.is_empty())
        .then(|| serde_json::to_string(&entry.keywords))
        .transpose()?;
    let credits = (!entry.credits.is_empty())
        .then(|| serde_json::to_string(&entry.credits))
        .transpose()?;

    sqlx::query(
        "INSERT INTO entries (date_id, date, title, title_raw, explanation_html, explanation_text,
                              credits, credit_text, has_copyright, license_url, tomorrow_teaser,
                              keywords, media_kind, media_url, media_hd_url, source_url,
                              parser_version, parsed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
         ON CONFLICT(date_id) DO UPDATE SET
           date = excluded.date, title = excluded.title, title_raw = excluded.title_raw,
           explanation_html = excluded.explanation_html,
           explanation_text = excluded.explanation_text,
           credits = excluded.credits, credit_text = excluded.credit_text,
           has_copyright = excluded.has_copyright, license_url = excluded.license_url,
           tomorrow_teaser = excluded.tomorrow_teaser,
           keywords = excluded.keywords, media_kind = excluded.media_kind,
           media_url = excluded.media_url, media_hd_url = excluded.media_hd_url,
           source_url = excluded.source_url, parser_version = excluded.parser_version,
           parsed_at = excluded.parsed_at",
    )
    .bind(entry.date.days())
    .bind(entry.date.to_string())
    .bind(&entry.title)
    .bind(&entry.title_raw)
    .bind(&entry.explanation_html)
    .bind(&entry.explanation_text)
    .bind(credits)
    .bind(entry.credit_text())
    .bind(entry.has_copyright)
    .bind(&entry.license_url)
    .bind(&entry.tomorrow_teaser)
    .bind(keywords)
    .bind(entry.media.kind.to_string())
    .bind(&entry.media.url)
    .bind(&entry.media.hd_url)
    .bind(&entry.source_url)
    .bind(PARSER_VERSION)
    .bind(chrono::Utc::now().timestamp())
    .execute(&mut **tx)
    .await?;

    sqlx::query("DELETE FROM entry_media WHERE date_id = ?1")
        .bind(entry.date.days())
        .execute(&mut **tx)
        .await?;

    for (idx, media) in entry.extra_media.iter().enumerate() {
        sqlx::query(
            "INSERT INTO entry_media (date_id, idx, kind, url, hd_url) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(entry.date.days())
        .bind(idx as i64)
        .bind(media.kind.to_string())
        .bind(&media.url)
        .bind(&media.hd_url)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}
