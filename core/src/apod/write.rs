use super::read::{ApodReader, ApodResult, to_dates};
use crate::PARSER_VERSION;
use crate::date::ApodDate;
use crate::db::{Db, DbConfig};
use crate::entry::ApodEntry;
use crate::media::{Media, Thumb};
use crate::{resource, text};
use sqlx::migrate::Migrator;
use sqlx::{AssertSqlSafe, Row, Sqlite, Transaction};
use std::collections::HashMap;
use std::path::Path;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");
const ROW_BATCH: usize = 128;

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

    pub async fn set_thumb(&self, date: ApodDate, thumb: Option<&Thumb>) -> ApodResult<()> {
        sqlx::query(
            "UPDATE entries SET thumb_path = ?2, thumb_width = ?3, thumb_height = ?4
             WHERE date_id = ?1",
        )
        .bind(date.days())
        .bind(thumb.map(|thumb| thumb.path.as_str()))
        .bind(thumb.and_then(|thumb| thumb.width).map(i64::from))
        .bind(thumb.and_then(|thumb| thumb.height).map(i64::from))
        .execute(self.db().writer()?)
        .await?;
        Ok(())
    }

    pub async fn unmeasured_thumbs(&self) -> ApodResult<Vec<(ApodDate, String)>> {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT date_id, thumb_path FROM entries
             WHERE thumb_path IS NOT NULL AND thumb_width IS NULL
             ORDER BY date_id DESC",
        )
        .fetch_all(self.db().reader())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(days, path)| (ApodDate::from_days(days as i32), path))
            .collect())
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

    write_derived(tx, entry).await
}

async fn write_derived(tx: &mut Transaction<'_, Sqlite>, entry: &ApodEntry) -> ApodResult<()> {
    let date_id = entry.date.days();

    sqlx::query("DELETE FROM entry_words WHERE date_id = ?1")
        .bind(date_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM entry_resources WHERE date_id = ?1")
        .bind(date_id)
        .execute(&mut **tx)
        .await?;

    let counts = text::word_counts(&entry.explanation_text);
    for chunk in counts.iter().collect::<Vec<_>>().chunks(ROW_BATCH) {
        let mut insert = sqlx::query(AssertSqlSafe(format!(
            "INSERT INTO entry_words (date_id, word, n) VALUES {}",
            values(3, chunk.len())
        )));
        for (word, n) in chunk {
            insert = insert.bind(date_id).bind(*word).bind(i64::from(**n));
        }
        insert.execute(&mut **tx).await?;
    }

    let links = resource::links(entry);
    for chunk in links.chunks(ROW_BATCH) {
        let mut upsert = sqlx::query_as::<_, (i64, String)>(AssertSqlSafe(format!(
            "INSERT INTO resources (key, scheme, host) VALUES {}
             ON CONFLICT(key) DO UPDATE SET
               scheme = CASE WHEN excluded.scheme = 'https' THEN 'https' ELSE scheme END
             RETURNING id, key",
            values(3, chunk.len())
        )));
        for link in chunk {
            upsert = upsert.bind(&link.key).bind(&link.scheme).bind(&link.host);
        }
        let ids: HashMap<String, i64> = upsert
            .fetch_all(&mut **tx)
            .await?
            .into_iter()
            .map(|(id, key)| (key, id))
            .collect();

        let mut insert = sqlx::query(AssertSqlSafe(format!(
            "INSERT INTO entry_resources (date_id, resource_id, n, anchor, in_credit)
             VALUES {}",
            values(5, chunk.len())
        )));
        for link in chunk {
            insert = insert
                .bind(date_id)
                .bind(ids.get(&link.key).copied())
                .bind(i64::from(link.count))
                .bind(&link.anchor)
                .bind(link.in_credit);
        }
        insert.execute(&mut **tx).await?;
    }

    let stats = text::stats(&entry.explanation_text, &counts);
    sqlx::query(
        "INSERT INTO entry_stats (date_id, word_count, unique_words, char_count, sentences,
                                  link_count, resource_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(date_id) DO UPDATE SET
           word_count = excluded.word_count, unique_words = excluded.unique_words,
           char_count = excluded.char_count, sentences = excluded.sentences,
           link_count = excluded.link_count, resource_count = excluded.resource_count",
    )
    .bind(date_id)
    .bind(i64::from(stats.words))
    .bind(i64::from(stats.unique_words))
    .bind(i64::from(stats.chars))
    .bind(i64::from(stats.sentences))
    .bind(links.iter().map(|link| i64::from(link.count)).sum::<i64>())
    .bind(links.len() as i64)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn values(columns: usize, rows: usize) -> String {
    let row = format!("({})", vec!["?"; columns].join(", "));
    vec![row; rows].join(", ")
}
