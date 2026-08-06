use anyhow::{Context, Result};
use apod_core::{ApodDate, ApodEntry, ApodSummary, Media, MediaKind, SearchHit};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, Row, types::Value};
use serde::Serialize;
use std::path::Path;
use std::str::FromStr;

const HIGHLIGHT_OPEN: &str = "\u{2}";
const HIGHLIGHT_CLOSE: &str = "\u{3}";

const ENTRY_COLUMNS: &str = "date_id, title, title_raw, explanation_html, explanation_text, \
                             credit_html, credit_text, has_copyright, tomorrow_teaser, keywords, \
                             media_kind, media_url, media_hd_url, thumb_path, source_url";

const SUMMARY_COLUMNS: &str =
    "date_id, title, has_copyright, media_kind, media_url, media_hd_url, thumb_path";

#[derive(Clone)]
pub struct Store {
    pool: Pool<SqliteConnectionManager>,
}

#[derive(Debug, Clone, Default)]
pub struct Filters {
    pub from: Option<ApodDate>,
    pub to: Option<ApodDate>,
    pub kind: Option<MediaKind>,
    pub copyright: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Asc,
    Desc,
}

#[derive(Debug, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<ApodDate>,
}

#[derive(Debug, Serialize)]
pub struct SearchResults {
    pub items: Vec<SearchHit>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct Stats {
    pub entries: i64,
    pub thumbnails: i64,
    pub first: Option<ApodDate>,
    pub latest: Option<ApodDate>,
    pub by_media_kind: Vec<KindCount>,
}

#[derive(Debug, Serialize)]
pub struct KindCount {
    pub kind: String,
    pub count: i64,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let manager = SqliteConnectionManager::file(path).with_init(|conn| {
            conn.execute_batch(
                "PRAGMA query_only = ON;
                 PRAGMA busy_timeout = 5000;",
            )
        });

        let pool = Pool::builder()
            .build(manager)
            .with_context(|| format!("opening {}", path.display()))?;

        pool.get()?
            .query_row("SELECT COUNT(*) FROM entries", [], |row| {
                row.get::<_, i64>(0)
            })
            .with_context(|| {
                format!(
                    "{} has no entries table. Has the archiver run?",
                    path.display()
                )
            })?;

        Ok(Self { pool })
    }

    pub fn entry(&self, date: ApodDate) -> Result<Option<ApodEntry>> {
        let conn = self.pool.get()?;
        let entry = conn
            .query_row(
                &format!("SELECT {ENTRY_COLUMNS} FROM entries WHERE date_id = ?1"),
                [date.days()],
                read_entry,
            )
            .optional()?;

        let Some(mut entry) = entry else {
            return Ok(None);
        };

        let mut stmt = conn
            .prepare("SELECT kind, url, hd_url FROM entry_media WHERE date_id = ?1 ORDER BY idx")?;
        entry.extra_media = stmt
            .query_map([date.days()], |row| {
                Ok(media(
                    &row.get::<_, String>(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    None,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Some(entry))
    }

    pub fn latest(&self) -> Result<Option<ApodEntry>> {
        let date = self
            .pool
            .get()?
            .query_row("SELECT MAX(date_id) FROM entries", [], |row| {
                row.get::<_, Option<i64>>(0)
            })?
            .map(|days| ApodDate::from_days(days as i32));

        match date {
            Some(date) => self.entry(date),
            None => Ok(None),
        }
    }

    pub fn random(&self, kind: Option<MediaKind>) -> Result<Option<ApodDate>> {
        let conn = self.pool.get()?;
        let date = match kind {
            Some(kind) => conn
                .query_row(
                    "SELECT date_id FROM entries WHERE media_kind = ?1 ORDER BY RANDOM() LIMIT 1",
                    [kind.to_string()],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?,
            None => conn
                .query_row(
                    "SELECT date_id FROM entries ORDER BY RANDOM() LIMIT 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?,
        };

        Ok(date.map(|days| ApodDate::from_days(days as i32)))
    }

    pub fn list(
        &self,
        filters: &Filters,
        cursor: Option<ApodDate>,
        limit: usize,
        order: Order,
    ) -> Result<Page<ApodSummary>> {
        let mut sql = format!("SELECT {SUMMARY_COLUMNS} FROM entries WHERE 1 = 1");
        let mut params: Vec<Value> = Vec::new();
        push_filters(&mut sql, &mut params, filters);

        if let Some(cursor) = cursor {
            sql.push_str(match order {
                Order::Asc => " AND date_id >= ?",
                Order::Desc => " AND date_id <= ?",
            });
            params.push(Value::Integer(cursor.days().into()));
        }

        sql.push_str(match order {
            Order::Asc => " ORDER BY date_id ASC LIMIT ?",
            Order::Desc => " ORDER BY date_id DESC LIMIT ?",
        });
        params.push(Value::Integer(limit as i64 + 1));

        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(&sql)?;
        let mut items: Vec<ApodSummary> = stmt
            .query_map(rusqlite::params_from_iter(params), read_summary)?
            .collect::<Result<Vec<_>, _>>()?;

        let next_cursor = (items.len() > limit).then(|| items.remove(limit).date);
        Ok(Page { items, next_cursor })
    }

    pub fn on_this_day(&self, month: u32, day: u32) -> Result<Vec<ApodSummary>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {SUMMARY_COLUMNS} FROM entries
             WHERE substr(date, 6) = ?1 ORDER BY date_id DESC"
        ))?;

        let items = stmt
            .query_map([format!("{month:02}-{day:02}")], read_summary)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(items)
    }

    pub fn search(
        &self,
        query: &str,
        filters: &Filters,
        by_date: bool,
        offset: usize,
        limit: usize,
        snippet_tokens: usize,
    ) -> Result<SearchResults> {
        let Some(match_query) = fts_query(query) else {
            return Ok(SearchResults {
                items: Vec::new(),
                total: 0,
            });
        };

        let mut where_clause = String::from("WHERE entries_fts MATCH ?");
        let mut params: Vec<Value> = vec![Value::Text(match_query)];
        push_filters(&mut where_clause, &mut params, filters);

        let conn = self.pool.get()?;
        let total: i64 = conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM entries_fts
                 JOIN entries ON entries.date_id = entries_fts.rowid {where_clause}"
            ),
            rusqlite::params_from_iter(params.iter()),
            |row| row.get(0),
        )?;

        let ordering = if by_date {
            "entries.date_id DESC"
        } else {
            "bm25(entries_fts, 10.0, 1.0, 2.0, 5.0) ASC"
        };

        let sql = format!(
            "SELECT {columns},
                    snippet(entries_fts, 1, char(2), char(3), '…', {tokens})
             FROM entries_fts JOIN entries ON entries.date_id = entries_fts.rowid
             {where_clause}
             ORDER BY {ordering} LIMIT ? OFFSET ?",
            columns = SUMMARY_COLUMNS
                .split(", ")
                .map(|column| format!("entries.{column}"))
                .collect::<Vec<_>>()
                .join(", "),
            tokens = snippet_tokens.clamp(1, 64),
        );

        let mut ordered = params;
        ordered.push(Value::Integer(limit as i64));
        ordered.push(Value::Integer(offset as i64));

        let mut stmt = conn.prepare(&sql)?;
        let items = stmt
            .query_map(rusqlite::params_from_iter(ordered), |row| {
                Ok(SearchHit {
                    entry: read_summary(row)?,
                    snippet: highlight(&row.get::<_, String>(7)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(SearchResults { items, total })
    }

    pub fn stats(&self) -> Result<Stats> {
        let conn = self.pool.get()?;

        let (entries, thumbnails, first, latest) = conn.query_row(
            "SELECT COUNT(*), COUNT(thumb_path), MIN(date_id), MAX(date_id) FROM entries",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            },
        )?;

        let mut stmt = conn.prepare(
            "SELECT media_kind, COUNT(*) FROM entries GROUP BY media_kind ORDER BY COUNT(*) DESC",
        )?;
        let by_media_kind = stmt
            .query_map([], |row| {
                Ok(KindCount {
                    kind: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Stats {
            entries,
            thumbnails,
            first: first.map(|days| ApodDate::from_days(days as i32)),
            latest: latest.map(|days| ApodDate::from_days(days as i32)),
            by_media_kind,
        })
    }

    pub fn all_dates(&self) -> Result<Vec<ApodDate>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT date_id FROM entries ORDER BY date_id DESC")?;
        let dates = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|days| ApodDate::from_days(days as i32))
            .collect();
        Ok(dates)
    }
}

fn push_filters(sql: &mut String, params: &mut Vec<Value>, filters: &Filters) {
    if let Some(from) = filters.from {
        sql.push_str(" AND entries.date_id >= ?");
        params.push(Value::Integer(from.days().into()));
    }
    if let Some(to) = filters.to {
        sql.push_str(" AND entries.date_id <= ?");
        params.push(Value::Integer(to.days().into()));
    }
    if let Some(kind) = filters.kind {
        sql.push_str(" AND entries.media_kind = ?");
        params.push(Value::Text(kind.to_string()));
    }
    if let Some(copyright) = filters.copyright {
        sql.push_str(" AND entries.has_copyright = ?");
        params.push(Value::Integer(copyright.into()));
    }
}

fn fts_query(raw: &str) -> Option<String> {
    let terms: Vec<String> = raw
        .split_whitespace()
        .map(|term| {
            term.chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'')
                .collect::<String>()
        })
        .filter(|term| !term.is_empty())
        .collect();

    if terms.is_empty() {
        return None;
    }

    let last = terms.len() - 1;
    Some(
        terms
            .iter()
            .enumerate()
            .map(|(index, term)| {
                let escaped = term.replace('"', "");
                if index == last {
                    format!("\"{escaped}\"*")
                } else {
                    format!("\"{escaped}\"")
                }
            })
            .collect::<Vec<_>>()
            .join(" AND "),
    )
}

fn highlight(snippet: &str) -> String {
    snippet
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace(HIGHLIGHT_OPEN, "<mark>")
        .replace(HIGHLIGHT_CLOSE, "</mark>")
}

fn media(kind: &str, url: Option<String>, hd_url: Option<String>, thumb: Option<String>) -> Media {
    let mut media = Media::new(
        MediaKind::from_str(kind).unwrap_or(MediaKind::None),
        url,
        hd_url,
    );
    media.thumb_url = thumb.map(|path| format!("/thumbs/{path}"));
    media
}

fn read_summary(row: &Row<'_>) -> rusqlite::Result<ApodSummary> {
    Ok(ApodSummary {
        date: ApodDate::from_days(row.get::<_, i64>(0)? as i32),
        title: row.get(1)?,
        has_copyright: row.get(2)?,
        media: media(
            &row.get::<_, String>(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
        ),
    })
}

fn read_entry(row: &Row<'_>) -> rusqlite::Result<ApodEntry> {
    let keywords: Option<String> = row.get(9)?;

    Ok(ApodEntry {
        date: ApodDate::from_days(row.get::<_, i64>(0)? as i32),
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
        media: media(
            &row.get::<_, String>(10)?,
            row.get(11)?,
            row.get(12)?,
            row.get(13)?,
        ),
        extra_media: Vec::new(),
        source_url: row.get(14)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

    const TEST_SCHEMA: &str = "
        CREATE TABLE entries (
          date_id INTEGER PRIMARY KEY, date TEXT NOT NULL UNIQUE, title TEXT NOT NULL,
          title_raw TEXT, explanation_html TEXT NOT NULL, explanation_text TEXT NOT NULL,
          credit_html TEXT, credit_text TEXT, has_copyright INTEGER NOT NULL DEFAULT 0,
          tomorrow_teaser TEXT, keywords TEXT, media_kind TEXT NOT NULL, media_url TEXT,
          media_hd_url TEXT, thumb_path TEXT, source_url TEXT NOT NULL,
          parser_version INTEGER NOT NULL, parsed_at INTEGER NOT NULL);
        CREATE TABLE entry_media (
          date_id INTEGER NOT NULL, idx INTEGER NOT NULL, kind TEXT NOT NULL,
          url TEXT, hd_url TEXT, PRIMARY KEY (date_id, idx));
        CREATE VIRTUAL TABLE entries_fts USING fts5(
          title, explanation_text, credit_text, keywords,
          content = 'entries', content_rowid = 'date_id',
          tokenize = 'unicode61 remove_diacritics 2');
        CREATE TRIGGER entries_ai AFTER INSERT ON entries BEGIN
          INSERT INTO entries_fts(rowid, title, explanation_text, credit_text, keywords)
          VALUES (new.date_id, new.title, new.explanation_text, new.credit_text, new.keywords);
        END;
    ";

    fn temp_store(rows: &[(&str, &str, &str)]) -> Store {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "apod-api-test-{}-{}.db",
            std::process::id(),
            COUNTER.fetch_add(1, AtomicOrdering::Relaxed)
        ));
        let _ = std::fs::remove_file(&path);

        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(TEST_SCHEMA).unwrap();

        for (date, title, explanation) in rows {
            let parsed: ApodDate = date.parse().unwrap();
            conn.execute(
                "INSERT INTO entries (date_id, date, title, explanation_html, explanation_text,
                                      media_kind, media_url, thumb_path, source_url,
                                      parser_version, parsed_at)
                 VALUES (?1, ?2, ?3, ?4, ?4, 'image_jpg', 'https://x/y.jpg', ?5, ?6, 1, 0)",
                rusqlite::params![
                    parsed.days(),
                    date,
                    title,
                    explanation,
                    parsed.thumb_path(),
                    parsed.source_url()
                ],
            )
            .unwrap();
        }
        drop(conn);

        Store::open(&path).unwrap()
    }

    #[test]
    fn search_returns_the_rows_it_counts() {
        let store = temp_store(&[
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
        ]);

        let results = store
            .search("saturn", &Filters::default(), false, 0, 30, 32)
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
    }

    #[test]
    fn search_marks_the_match_in_the_snippet() {
        let store = temp_store(&[(
            "2024-03-05",
            "Orion",
            "A nebula in the constellation Orion.",
        )]);
        let results = store
            .search("nebula", &Filters::default(), false, 0, 30, 32)
            .unwrap();

        assert!(
            results.items[0].snippet.contains("<mark>nebula</mark>"),
            "{}",
            results.items[0].snippet
        );
    }

    #[test]
    fn search_honours_filters_and_paging() {
        let store = temp_store(&[
            ("2024-03-05", "Saturn One", "ringed"),
            ("2024-03-06", "Saturn Two", "ringed"),
            ("2025-03-05", "Saturn Three", "ringed"),
        ]);

        let filters = Filters {
            to: Some("2024-12-31".parse().unwrap()),
            ..Filters::default()
        };
        let results = store.search("saturn", &filters, true, 0, 30, 32).unwrap();
        assert_eq!(results.total, 2);

        let page = store.search("saturn", &filters, true, 1, 1, 32).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(
            page.total, 2,
            "total describes the whole result set, not the page"
        );
    }

    #[test]
    fn listing_pages_forward_on_a_date_cursor() {
        let store = temp_store(&[
            ("2024-03-05", "One", "a"),
            ("2024-03-06", "Two", "b"),
            ("2024-03-07", "Three", "c"),
        ]);

        let first = store
            .list(&Filters::default(), None, 2, Order::Desc)
            .unwrap();
        assert_eq!(first.items.len(), 2);
        assert_eq!(first.items[0].date.to_string(), "2024-03-07");
        assert_eq!(first.next_cursor.unwrap().to_string(), "2024-03-05");

        let second = store
            .list(&Filters::default(), first.next_cursor, 2, Order::Desc)
            .unwrap();
        assert_eq!(second.items.len(), 1);
        assert_eq!(second.items[0].date.to_string(), "2024-03-05");
        assert!(second.next_cursor.is_none(), "last page has no cursor");
    }

    #[test]
    fn thumbnails_come_back_as_urls_not_paths() {
        let store = temp_store(&[("2024-03-05", "One", "a")]);
        let entry = store.entry("2024-03-05".parse().unwrap()).unwrap().unwrap();

        assert_eq!(
            entry.media.thumb_url.as_deref(),
            Some("/thumbs/2024/03/2024-03-05.webp")
        );
    }

    #[test]
    fn builds_a_safe_prefix_query() {
        assert_eq!(
            fts_query("crab nebula"),
            Some(r#""crab" AND "nebula"*"#.into())
        );
    }

    #[test]
    fn neutralises_fts_syntax() {
        assert_eq!(
            fts_query(r#"crab" OR "x"#),
            Some(r#""crab" AND "OR" AND "x"*"#.into())
        );
        assert_eq!(fts_query("a*b"), Some(r#""ab"*"#.into()));
        assert_eq!(fts_query("(nebula)"), Some(r#""nebula"*"#.into()));
    }

    #[test]
    fn an_empty_query_matches_nothing() {
        assert_eq!(fts_query("   "), None);
        assert_eq!(fts_query("!!! ???"), None);
    }

    #[test]
    fn escapes_snippet_content_before_adding_marks() {
        let raw = format!("5 < 7 {HIGHLIGHT_OPEN}nebula{HIGHLIGHT_CLOSE} <script>");
        assert_eq!(
            highlight(&raw),
            "5 &lt; 7 <mark>nebula</mark> &lt;script&gt;"
        );
    }

    #[test]
    fn thumbnail_paths_become_urls() {
        let media = media(
            "image_jpg",
            None,
            None,
            Some("2024/03/2024-03-05.webp".into()),
        );
        assert_eq!(
            media.thumb_url.as_deref(),
            Some("/thumbs/2024/03/2024-03-05.webp")
        );
    }
}
