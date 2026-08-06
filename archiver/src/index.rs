use anyhow::{Context, Result};
use apod_core::{ApodDate, ApodEntry, Media, MediaKind, PARSER_VERSION};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use std::path::Path;
use std::str::FromStr;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS entries (
  date_id           INTEGER PRIMARY KEY,
  date              TEXT NOT NULL UNIQUE,
  title             TEXT NOT NULL,
  title_raw         TEXT,
  explanation_html  TEXT NOT NULL,
  explanation_text  TEXT NOT NULL,
  credit_html       TEXT,
  credit_text       TEXT,
  has_copyright     INTEGER NOT NULL DEFAULT 0,
  tomorrow_teaser   TEXT,
  keywords          TEXT,
  media_kind        TEXT NOT NULL,
  media_url         TEXT,
  media_hd_url      TEXT,
  thumb_path        TEXT,
  source_url        TEXT NOT NULL,
  parser_version    INTEGER NOT NULL,
  parsed_at         INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_entries_kind ON entries(media_kind);
CREATE INDEX IF NOT EXISTS idx_entries_parser ON entries(parser_version);

CREATE TABLE IF NOT EXISTS entry_media (
  date_id INTEGER NOT NULL REFERENCES entries(date_id) ON DELETE CASCADE,
  idx     INTEGER NOT NULL,
  kind    TEXT NOT NULL,
  url     TEXT,
  hd_url  TEXT,
  PRIMARY KEY (date_id, idx)
);

CREATE VIRTUAL TABLE IF NOT EXISTS entries_fts USING fts5(
  title, explanation_text, credit_text, keywords,
  content = 'entries', content_rowid = 'date_id',
  tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS entries_ai AFTER INSERT ON entries BEGIN
  INSERT INTO entries_fts(rowid, title, explanation_text, credit_text, keywords)
  VALUES (new.date_id, new.title, new.explanation_text, new.credit_text, new.keywords);
END;

CREATE TRIGGER IF NOT EXISTS entries_ad AFTER DELETE ON entries BEGIN
  INSERT INTO entries_fts(entries_fts, rowid, title, explanation_text, credit_text, keywords)
  VALUES ('delete', old.date_id, old.title, old.explanation_text, old.credit_text, old.keywords);
END;

CREATE TRIGGER IF NOT EXISTS entries_au AFTER UPDATE ON entries BEGIN
  INSERT INTO entries_fts(entries_fts, rowid, title, explanation_text, credit_text, keywords)
  VALUES ('delete', old.date_id, old.title, old.explanation_text, old.credit_text, old.keywords);
  INSERT INTO entries_fts(rowid, title, explanation_text, credit_text, keywords)
  VALUES (new.date_id, new.title, new.explanation_text, new.credit_text, new.keywords);
END;

CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
"#;

const COLUMNS: &str = "date_id, title, title_raw, explanation_html, explanation_text, \
                       credit_html, credit_text, has_copyright, tomorrow_teaser, keywords, \
                       media_kind, media_url, media_hd_url, thumb_path, source_url";

pub struct IndexStore {
    conn: Connection,
}

impl IndexStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = super::db::open(path)?;
        conn.execute_batch(SCHEMA)
            .with_context(|| format!("initialising {}", path.display()))?;
        Ok(Self { conn })
    }

    pub fn upsert_all(&mut self, entries: &[ApodEntry]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for entry in entries {
            write_entry(&tx, entry)?;
        }
        tx.execute(
            "INSERT INTO meta (key, value) VALUES ('parser_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![PARSER_VERSION.to_string()],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn upsert(&mut self, entry: &ApodEntry) -> Result<()> {
        self.upsert_all(std::slice::from_ref(entry))
    }

    pub fn set_thumb(&self, date: ApodDate, thumb_path: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE entries SET thumb_path = ?2 WHERE date_id = ?1",
            params![date.days(), thumb_path],
        )?;
        Ok(())
    }

    pub fn get(&self, date: ApodDate) -> Result<Option<ApodEntry>> {
        let entry = self
            .conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM entries WHERE date_id = ?1"),
                params![date.days()],
                read_entry,
            )
            .optional()?;

        let Some(mut entry) = entry else {
            return Ok(None);
        };
        entry.extra_media = self.extra_media(date)?;
        Ok(Some(entry))
    }

    pub fn all_dates(&self) -> Result<Vec<ApodDate>> {
        self.dates("SELECT date_id FROM entries ORDER BY date_id DESC", [])
    }

    pub fn stale_dates(&self) -> Result<Vec<ApodDate>> {
        self.dates(
            "SELECT date_id FROM entries WHERE parser_version < ?1 ORDER BY date_id DESC",
            [PARSER_VERSION],
        )
    }

    pub fn missing_thumbs(&self) -> Result<Vec<(ApodDate, Media)>> {
        let mut stmt = self.conn.prepare(
            "SELECT date_id, media_kind, media_url, media_hd_url FROM entries
             WHERE thumb_path IS NULL
               AND media_kind IN ('image_jpg', 'image_png', 'image_gif', 'youtube', 'vimeo')
             ORDER BY date_id DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let date = ApodDate::from_days(row.get::<_, i64>(0)? as i32);
                let kind =
                    MediaKind::from_str(&row.get::<_, String>(1)?).unwrap_or(MediaKind::None);
                Ok((date, Media::new(kind, row.get(2)?, row.get(3)?)))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn media_for(&self, dates: &[ApodDate]) -> Result<Vec<(ApodDate, Media)>> {
        let mut out = Vec::with_capacity(dates.len());
        let mut stmt = self.conn.prepare(
            "SELECT media_kind, media_url, media_hd_url FROM entries WHERE date_id = ?1",
        )?;
        for &date in dates {
            let media = stmt
                .query_row(params![date.days()], |row| {
                    let kind =
                        MediaKind::from_str(&row.get::<_, String>(0)?).unwrap_or(MediaKind::None);
                    Ok(Media::new(kind, row.get(1)?, row.get(2)?))
                })
                .optional()?;
            if let Some(media) = media {
                out.push((date, media));
            }
        }
        Ok(out)
    }

    pub fn count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))?)
    }

    pub fn thumb_count(&self) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM entries WHERE thumb_path IS NOT NULL",
            [],
            |row| row.get(0),
        )?)
    }

    fn extra_media(&self, date: ApodDate) -> Result<Vec<Media>> {
        let mut stmt = self
            .conn
            .prepare("SELECT kind, url, hd_url FROM entry_media WHERE date_id = ?1 ORDER BY idx")?;
        let media = stmt
            .query_map(params![date.days()], |row| {
                let kind =
                    MediaKind::from_str(&row.get::<_, String>(0)?).unwrap_or(MediaKind::None);
                Ok(Media::new(kind, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(media)
    }

    fn dates<P: rusqlite::Params>(&self, sql: &str, params: P) -> Result<Vec<ApodDate>> {
        let mut stmt = self.conn.prepare(sql)?;
        let dates = stmt
            .query_map(params, |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|days| ApodDate::from_days(days as i32))
            .collect();
        Ok(dates)
    }
}

fn write_entry(tx: &Transaction<'_>, entry: &ApodEntry) -> Result<()> {
    let keywords = (!entry.keywords.is_empty())
        .then(|| serde_json::to_string(&entry.keywords))
        .transpose()?;

    tx.execute(
        "INSERT INTO entries (date_id, date, title, title_raw, explanation_html, explanation_text,
                              credit_html, credit_text, has_copyright, tomorrow_teaser, keywords,
                              media_kind, media_url, media_hd_url, source_url, parser_version,
                              parsed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
         ON CONFLICT(date_id) DO UPDATE SET
           date = excluded.date, title = excluded.title, title_raw = excluded.title_raw,
           explanation_html = excluded.explanation_html,
           explanation_text = excluded.explanation_text,
           credit_html = excluded.credit_html, credit_text = excluded.credit_text,
           has_copyright = excluded.has_copyright, tomorrow_teaser = excluded.tomorrow_teaser,
           keywords = excluded.keywords, media_kind = excluded.media_kind,
           media_url = excluded.media_url, media_hd_url = excluded.media_hd_url,
           source_url = excluded.source_url, parser_version = excluded.parser_version,
           parsed_at = excluded.parsed_at",
        params![
            entry.date.days(),
            entry.date.to_string(),
            entry.title,
            entry.title_raw,
            entry.explanation_html,
            entry.explanation_text,
            entry.credit_html,
            entry.credit_text,
            entry.has_copyright,
            entry.tomorrow_teaser,
            keywords,
            entry.media.kind.to_string(),
            entry.media.url,
            entry.media.hd_url,
            entry.source_url,
            PARSER_VERSION,
            chrono::Utc::now().timestamp(),
        ],
    )?;

    tx.execute(
        "DELETE FROM entry_media WHERE date_id = ?1",
        params![entry.date.days()],
    )?;
    for (idx, media) in entry.extra_media.iter().enumerate() {
        tx.execute(
            "INSERT INTO entry_media (date_id, idx, kind, url, hd_url) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                entry.date.days(),
                idx as i64,
                media.kind.to_string(),
                media.url,
                media.hd_url
            ],
        )?;
    }

    Ok(())
}

fn read_entry(row: &Row<'_>) -> rusqlite::Result<ApodEntry> {
    let date = ApodDate::from_days(row.get::<_, i64>(0)? as i32);
    let keywords: Option<String> = row.get(9)?;
    let kind = MediaKind::from_str(&row.get::<_, String>(10)?).unwrap_or(MediaKind::None);

    let mut media = Media::new(kind, row.get(11)?, row.get(12)?);
    media.thumb_url = row.get::<_, Option<String>>(13)?;

    Ok(ApodEntry {
        date,
        title: row.get(1)?,
        title_raw: row.get(2)?,
        explanation_html: row.get(3)?,
        explanation_text: row.get(4)?,
        credit_html: row.get(5)?,
        credit_text: row.get(6)?,
        has_copyright: row.get(7)?,
        tomorrow_teaser: row.get(8)?,
        keywords: keywords
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default(),
        media,
        extra_media: Vec::new(),
        source_url: row.get(14)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> IndexStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        IndexStore { conn }
    }

    fn entry(date: &str, title: &str) -> ApodEntry {
        let date: ApodDate = date.parse().unwrap();
        ApodEntry {
            date,
            title: title.into(),
            title_raw: Some(format!("APOD: {title}")),
            explanation_html: "The <b>Crab</b> Nebula is a supernova remnant.".into(),
            explanation_text: "The Crab Nebula is a supernova remnant.".into(),
            credit_html: Some("Jane Doe".into()),
            credit_text: Some("Jane Doe".into()),
            has_copyright: true,
            tomorrow_teaser: Some("open water".into()),
            keywords: vec!["nebula".into(), "supernova".into()],
            media: Media::new(
                MediaKind::ImageJpg,
                Some("https://apod.nasa.gov/apod/image/x.jpg".into()),
                None,
            ),
            extra_media: vec![Media::new(
                MediaKind::ImagePng,
                Some("https://apod.nasa.gov/apod/image/y.png".into()),
                None,
            )],
            source_url: date.source_url(),
        }
    }

    #[test]
    fn roundtrips_an_entry_including_extra_media() {
        let mut store = store();
        let original = entry("2024-03-05", "Crab Nebula");
        store.upsert(&original).unwrap();

        let loaded = store.get(original.date).unwrap().unwrap();
        assert_eq!(loaded.title, "Crab Nebula");
        assert_eq!(loaded.keywords, vec!["nebula", "supernova"]);
        assert!(loaded.has_copyright);
        assert_eq!(loaded.extra_media.len(), 1);
        assert_eq!(loaded.extra_media[0].kind, MediaKind::ImagePng);
    }

    #[test]
    fn reparsing_preserves_thumbnails() {
        let mut store = store();
        let mut original = entry("2024-03-05", "Crab Nebula");
        store.upsert(&original).unwrap();
        store
            .set_thumb(original.date, Some("2024/03/2024-03-05.webp"))
            .unwrap();

        original.title = "Crab Nebula, Corrected".into();
        store.upsert(&original).unwrap();

        let loaded = store.get(original.date).unwrap().unwrap();
        assert_eq!(loaded.title, "Crab Nebula, Corrected");
        assert_eq!(
            loaded.media.thumb_url.as_deref(),
            Some("2024/03/2024-03-05.webp")
        );
    }

    #[test]
    fn full_text_search_is_available_and_indexed() {
        let mut store = store();
        store.upsert(&entry("2024-03-05", "Crab Nebula")).unwrap();
        store.upsert(&entry("2024-03-06", "Orion")).unwrap();

        let hits: Vec<i64> = store
            .conn
            .prepare("SELECT rowid FROM entries_fts WHERE entries_fts MATCH 'supernova'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(hits.len(), 2, "both entries mention supernova");

        let by_title: Vec<i64> = store
            .conn
            .prepare("SELECT rowid FROM entries_fts WHERE entries_fts MATCH 'title:orion'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(by_title.len(), 1);
    }

    #[test]
    fn reindexes_on_update() {
        let mut store = store();
        let mut original = entry("2024-03-05", "Crab Nebula");
        store.upsert(&original).unwrap();

        original.explanation_text = "Now about galaxies instead.".into();
        store.upsert(&original).unwrap();

        let stale: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM entries_fts WHERE entries_fts MATCH 'remnant'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stale, 0, "the old explanation should no longer match");
    }

    #[test]
    fn tracks_missing_thumbnails() {
        let mut store = store();
        store.upsert(&entry("2024-03-05", "Crab Nebula")).unwrap();
        store.upsert(&entry("2024-03-06", "Orion")).unwrap();
        assert_eq!(store.missing_thumbs().unwrap().len(), 2);

        store
            .set_thumb("2024-03-05".parse().unwrap(), Some("x.webp"))
            .unwrap();
        assert_eq!(store.missing_thumbs().unwrap().len(), 1);
        assert_eq!(store.thumb_count().unwrap(), 1);
    }
}
