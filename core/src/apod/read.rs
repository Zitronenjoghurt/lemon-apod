use super::model::{Filters, KindCount, Order, Page, SearchResults, Stats};
use super::{ENTRY_COLUMNS, MIN_SCHEMA_VERSION, SCHEMA_VERSION, SUMMARY_COLUMNS, SchemaError};
use crate::date::ApodDate;
use crate::db::{Db, DbConfig, DbError};
use crate::entry::{ApodEntry, ApodSummary, SearchHit};
use crate::media::{Media, MediaKind};
use sqlx::sqlite::{SqliteArguments, SqliteRow};
use sqlx::{Arguments, AssertSqlSafe, Row};
use std::str::FromStr;

const MATCH_OPEN: &str = "\u{2}";
const MATCH_CLOSE: &str = "\u{3}";

#[derive(Debug, thiserror::Error)]
pub enum ApodError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error(transparent)]
    Schema(#[from] SchemaError),
    #[error("encoding a JSON column")]
    Encode(#[from] serde_json::Error),
}

impl From<sqlx::Error> for ApodError {
    fn from(source: sqlx::Error) -> Self {
        Self::Db(DbError::Query(source))
    }
}

pub type ApodResult<T> = Result<T, ApodError>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Snippet {
    #[default]
    Html,
    Delimited { open: String, close: String },
    Plain,
}

impl Snippet {
    pub fn render(&self, raw: &str) -> String {
        match self {
            Self::Html => raw
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace(MATCH_OPEN, "<mark>")
                .replace(MATCH_CLOSE, "</mark>"),
            Self::Delimited { open, close } => {
                raw.replace(MATCH_OPEN, open).replace(MATCH_CLOSE, close)
            }
            Self::Plain => raw.replace(MATCH_OPEN, "").replace(MATCH_CLOSE, ""),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApodReader {
    db: Db,
    thumb_base: Option<String>,
    snippet: Snippet,
}

impl ApodReader {
    pub async fn open(config: DbConfig) -> ApodResult<Self> {
        let path = config.path.display().to_string();
        let db = Db::open(config).await?;
        check_schema(&db, &path).await?;
        Ok(Self::from_db(db))
    }

    pub fn from_db(db: Db) -> Self {
        Self {
            db,
            thumb_base: None,
            snippet: Snippet::default(),
        }
    }

    pub fn with_thumb_base(mut self, base: impl Into<String>) -> Self {
        self.thumb_base = Some(base.into());
        self
    }

    pub fn with_snippet(mut self, snippet: Snippet) -> Self {
        self.snippet = snippet;
        self
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub async fn entry(&self, date: ApodDate) -> ApodResult<Option<ApodEntry>> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT {ENTRY_COLUMNS} FROM entries WHERE date_id = ?1"
        )))
        .bind(date.days())
        .fetch_optional(self.db.reader())
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let mut entry = self.read_entry(&row)?;
        entry.extra_media = self.extra_media(date).await?;
        Ok(Some(entry))
    }

    pub async fn latest(&self) -> ApodResult<Option<ApodEntry>> {
        let days: Option<i64> = sqlx::query_scalar("SELECT MAX(date_id) FROM entries")
            .fetch_one(self.db.reader())
            .await?;

        match days {
            Some(days) => self.entry(ApodDate::from_days(days as i32)).await,
            None => Ok(None),
        }
    }

    pub async fn random(&self, kind: Option<MediaKind>) -> ApodResult<Option<ApodDate>> {
        let days: Option<i64> =
            match kind {
                Some(kind) => sqlx::query_scalar(
                    "SELECT date_id FROM entries WHERE media_kind = ?1 ORDER BY RANDOM() LIMIT 1",
                )
                .bind(kind.to_string())
                .fetch_optional(self.db.reader())
                .await?,
                None => {
                    sqlx::query_scalar("SELECT date_id FROM entries ORDER BY RANDOM() LIMIT 1")
                        .fetch_optional(self.db.reader())
                        .await?
                }
            };

        Ok(days.map(|days| ApodDate::from_days(days as i32)))
    }

    pub async fn list(
        &self,
        filters: &Filters,
        cursor: Option<ApodDate>,
        limit: usize,
        order: Order,
    ) -> ApodResult<Page<ApodSummary>> {
        let mut sql = format!("SELECT {SUMMARY_COLUMNS} FROM entries WHERE 1 = 1");
        let mut params: Vec<Param> = Vec::new();
        push_filters(&mut sql, &mut params, filters);

        if let Some(cursor) = cursor {
            sql.push_str(match order {
                Order::Asc => " AND date_id >= ?",
                Order::Desc => " AND date_id <= ?",
            });
            params.push(Param::Int(cursor.days().into()));
        }

        sql.push_str(match order {
            Order::Asc => " ORDER BY date_id ASC LIMIT ?",
            Order::Desc => " ORDER BY date_id DESC LIMIT ?",
        });
        params.push(Param::Int(limit as i64 + 1));

        let rows = sqlx::query_with(AssertSqlSafe(sql), arguments(&params))
            .fetch_all(self.db.reader())
            .await?;

        let mut items = rows
            .iter()
            .map(|row| self.read_summary(row))
            .collect::<Result<Vec<_>, _>>()?;

        let next_cursor = (items.len() > limit).then(|| items.remove(limit).date);
        Ok(Page { items, next_cursor })
    }

    pub async fn on_this_day(&self, month: u32, day: u32) -> ApodResult<Vec<ApodSummary>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {SUMMARY_COLUMNS} FROM entries
             WHERE substr(date, 6) = ?1 ORDER BY date_id DESC"
        )))
        .bind(format!("{month:02}-{day:02}"))
        .fetch_all(self.db.reader())
        .await?;

        rows.iter().map(|row| self.read_summary(row)).collect()
    }

    pub async fn search(
        &self,
        query: &str,
        filters: &Filters,
        by_date: bool,
        offset: usize,
        limit: usize,
        snippet_tokens: usize,
    ) -> ApodResult<SearchResults> {
        let Some(match_query) = fts_query(query) else {
            return Ok(SearchResults {
                items: Vec::new(),
                total: 0,
            });
        };

        let mut where_clause = String::from("WHERE entries_fts MATCH ?");
        let mut params: Vec<Param> = vec![Param::Text(match_query)];
        push_filters(&mut where_clause, &mut params, filters);

        let total: i64 = sqlx::query_scalar_with(
            AssertSqlSafe(format!(
                "SELECT COUNT(*) FROM entries_fts
                 JOIN entries ON entries.date_id = entries_fts.rowid {where_clause}"
            )),
            arguments(&params),
        )
        .fetch_one(self.db.reader())
        .await?;

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

        params.push(Param::Int(limit as i64));
        params.push(Param::Int(offset as i64));

        let rows = sqlx::query_with(AssertSqlSafe(sql), arguments(&params))
            .fetch_all(self.db.reader())
            .await?;

        let items = rows
            .iter()
            .map(|row| {
                Ok(SearchHit {
                    entry: self.read_summary(row)?,
                    snippet: self.snippet.render(&row.try_get::<String, _>(7)?),
                })
            })
            .collect::<ApodResult<Vec<_>>>()?;

        Ok(SearchResults { items, total })
    }

    pub async fn stats(&self) -> ApodResult<Stats> {
        let (entries, thumbnails, first, latest): (i64, i64, Option<i64>, Option<i64>) =
            sqlx::query_as(
                "SELECT COUNT(*), COUNT(thumb_path), MIN(date_id), MAX(date_id) FROM entries",
            )
            .fetch_one(self.db.reader())
            .await?;

        let by_media_kind = sqlx::query_as::<_, (String, i64)>(
            "SELECT media_kind, COUNT(*) FROM entries GROUP BY media_kind ORDER BY COUNT(*) DESC",
        )
        .fetch_all(self.db.reader())
        .await?
        .into_iter()
        .map(|(kind, count)| KindCount { kind, count })
        .collect();

        Ok(Stats {
            entries,
            thumbnails,
            first: first.map(|days| ApodDate::from_days(days as i32)),
            latest: latest.map(|days| ApodDate::from_days(days as i32)),
            by_media_kind,
        })
    }

    pub async fn all_dates(&self) -> ApodResult<Vec<ApodDate>> {
        let days: Vec<i64> =
            sqlx::query_scalar("SELECT date_id FROM entries ORDER BY date_id DESC")
                .fetch_all(self.db.reader())
                .await?;
        Ok(to_dates(days))
    }

    pub async fn count(&self) -> ApodResult<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM entries")
            .fetch_one(self.db.reader())
            .await?)
    }

    pub async fn thumb_count(&self) -> ApodResult<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM entries WHERE thumb_path IS NOT NULL")
                .fetch_one(self.db.reader())
                .await?,
        )
    }

    async fn extra_media(&self, date: ApodDate) -> ApodResult<Vec<Media>> {
        let rows = sqlx::query(
            "SELECT kind, url, hd_url FROM entry_media WHERE date_id = ?1 ORDER BY idx",
        )
        .bind(date.days())
        .fetch_all(self.db.reader())
        .await?;

        rows.iter()
            .map(|row| {
                Ok(self.media(
                    &row.try_get::<String, _>(0)?,
                    row.try_get(1)?,
                    row.try_get(2)?,
                    None,
                ))
            })
            .collect()
    }

    pub(crate) fn media(
        &self,
        kind: &str,
        url: Option<String>,
        hd_url: Option<String>,
        thumb_path: Option<String>,
    ) -> Media {
        let mut media = Media::new(
            MediaKind::from_str(kind).unwrap_or(MediaKind::None),
            url,
            hd_url,
        );
        media.set_thumb(thumb_path, self.thumb_base.as_deref());
        media
    }

    fn read_summary(&self, row: &SqliteRow) -> ApodResult<ApodSummary> {
        Ok(ApodSummary {
            date: ApodDate::from_days(row.try_get::<i64, _>(0)? as i32),
            title: row.try_get(1)?,
            has_copyright: row.try_get(2)?,
            media: self.media(
                &row.try_get::<String, _>(3)?,
                row.try_get(4)?,
                row.try_get(5)?,
                row.try_get(6)?,
            ),
        })
    }

    fn read_entry(&self, row: &SqliteRow) -> ApodResult<ApodEntry> {
        Ok(ApodEntry {
            date: ApodDate::from_days(row.try_get::<i64, _>(0)? as i32),
            title: row.try_get(1)?,
            title_raw: row.try_get(2)?,
            explanation_html: row.try_get(3)?,
            explanation_text: row.try_get(4)?,
            credits: from_json(row.try_get(5)?),
            has_copyright: row.try_get(6)?,
            license_url: row.try_get(7)?,
            tomorrow_teaser: row.try_get(8)?,
            keywords: from_json(row.try_get(9)?),
            media: self.media(
                &row.try_get::<String, _>(10)?,
                row.try_get(11)?,
                row.try_get(12)?,
                row.try_get(13)?,
            ),
            extra_media: Vec::new(),
            source_url: row.try_get(14)?,
        })
    }
}

pub(crate) fn to_dates(days: Vec<i64>) -> Vec<ApodDate> {
    days.into_iter()
        .map(|days| ApodDate::from_days(days as i32))
        .collect()
}

async fn check_schema(db: &Db, path: &str) -> ApodResult<()> {
    let found = db
        .applied_version()
        .await?
        .ok_or_else(|| SchemaError::Unmigrated {
            path: path.to_owned(),
        })?;

    if !(MIN_SCHEMA_VERSION..=SCHEMA_VERSION).contains(&found) {
        return Err(SchemaError::Unsupported {
            path: path.to_owned(),
            found,
        }
        .into());
    }

    Ok(())
}

#[derive(Debug, Clone)]
enum Param {
    Int(i64),
    Text(String),
}

fn arguments(params: &[Param]) -> SqliteArguments {
    let mut args = SqliteArguments::default();
    for param in params {
        let bound = match param {
            Param::Int(value) => args.add(*value),
            Param::Text(value) => args.add(value.clone()),
        };
        bound.expect("binding an integer or a string cannot fail");
    }
    args
}

fn push_filters(sql: &mut String, params: &mut Vec<Param>, filters: &Filters) {
    if let Some(from) = filters.from {
        sql.push_str(" AND entries.date_id >= ?");
        params.push(Param::Int(from.days().into()));
    }
    if let Some(to) = filters.to {
        sql.push_str(" AND entries.date_id <= ?");
        params.push(Param::Int(to.days().into()));
    }
    if let Some(kind) = filters.kind {
        sql.push_str(" AND entries.media_kind = ?");
        params.push(Param::Text(kind.to_string()));
    }
    if let Some(copyright) = filters.copyright {
        sql.push_str(" AND entries.has_copyright = ?");
        params.push(Param::Int(copyright.into()));
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

fn from_json<T: serde::de::DeserializeOwned + Default>(raw: Option<String>) -> T {
    raw.and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let raw = format!("5 < 7 {MATCH_OPEN}nebula{MATCH_CLOSE} <script>");
        assert_eq!(
            Snippet::Html.render(&raw),
            "5 &lt; 7 <mark>nebula</mark> &lt;script&gt;"
        );
    }

    #[test]
    fn renders_snippets_for_consumers_that_are_not_browsers() {
        let raw = format!("a {MATCH_OPEN}nebula{MATCH_CLOSE} b");
        assert_eq!(
            Snippet::Delimited {
                open: "**".into(),
                close: "**".into()
            }
            .render(&raw),
            "a **nebula** b"
        );
        assert_eq!(Snippet::Plain.render(&raw), "a nebula b");
    }

    #[test]
    fn a_plain_snippet_leaves_markup_characters_as_the_author_wrote_them() {
        let raw = format!("5 < 7 {MATCH_OPEN}x{MATCH_CLOSE}");
        assert_eq!(Snippet::Plain.render(&raw), "5 < 7 x");
    }
}
