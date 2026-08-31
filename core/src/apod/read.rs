use super::model::{
    FieldDivergence, Filters, KindCount, Listing, Order, Page, SearchResults, Stats,
};
use super::query::fts_query;
use super::{ENTRY_COLUMNS, MIN_SCHEMA_VERSION, SCHEMA_VERSION, SUMMARY_COLUMNS, SchemaError};
use crate::date::ApodDate;
use crate::db::{Db, DbConfig, DbError};
use crate::entry::{ApodEntry, ApodSummary, Matched, Provenance, SearchHit};
use crate::media::{KindFilter, Media, MediaKind, Thumb};
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
    Delimited {
        open: String,
        close: String,
    },
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

    pub async fn random(&self, kind: Option<&KindFilter>) -> ApodResult<Option<ApodDate>> {
        let mut sql = String::from("SELECT date_id FROM entries WHERE 1 = 1");
        let mut params: Vec<Param> = Vec::new();
        push_filters(
            &mut sql,
            &mut params,
            &Filters {
                kind: kind.cloned(),
                ..Filters::default()
            },
        );
        sql.push_str(" ORDER BY RANDOM() LIMIT 1");

        let days: Option<i64> = sqlx::query_scalar_with(AssertSqlSafe(sql), arguments(&params))
            .fetch_optional(self.db.reader())
            .await?;

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
            .map(|row| self.summary(row))
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

        rows.iter().map(|row| self.summary(row)).collect()
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
        if query.trim().is_empty() {
            return self.filtered(filters, offset, limit).await;
        }

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

        let columns: Vec<String> = SUMMARY_COLUMNS
            .split(", ")
            .map(|column| format!("entries.{column}"))
            .collect();
        let snippet_column = columns.len();

        let sql = format!(
            "SELECT {columns},
                    snippet(entries_fts, 1, char(2), char(3), '…', {tokens}),
                    highlight(entries_fts, 0, char(2), char(3)),
                    highlight(entries_fts, 2, char(2), char(3)),
                    highlight(entries_fts, 3, char(2), char(3))
             FROM entries_fts JOIN entries ON entries.date_id = entries_fts.rowid
             {where_clause}
             ORDER BY {ordering} LIMIT ? OFFSET ?",
            columns = columns.join(", "),
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
                let snippet: String = row.try_get(snippet_column)?;
                let title: String = row.try_get(snippet_column + 1)?;
                let credit: Option<String> = row.try_get(snippet_column + 2)?;
                let keywords: Option<String> = row.try_get(snippet_column + 3)?;

                let hit = |text: &Option<String>| {
                    text.as_deref()
                        .is_some_and(|text| text.contains(MATCH_OPEN))
                };
                let matched = Matched {
                    title: title.contains(MATCH_OPEN),
                    explanation: snippet.contains(MATCH_OPEN),
                    credit: hit(&credit),
                    keywords: hit(&keywords),
                };

                Ok(SearchHit {
                    entry: self.summary(row)?,
                    snippet: self.snippet.render(&snippet),
                    credit: matched
                        .credit
                        .then(|| self.snippet.render(credit.as_deref().unwrap_or_default())),
                    keywords: matched.keywords.then(|| {
                        self.snippet
                            .render(&keyword_list(keywords.as_deref().unwrap_or_default()))
                    }),
                    matched,
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

        let (copyright, licensed): (i64, i64) = sqlx::query_as(
            "SELECT COALESCE(SUM(has_copyright), 0), COUNT(license_url) FROM entries",
        )
        .fetch_one(self.db.reader())
        .await?;

        let gaps = match (first, latest) {
            (Some(first), Some(latest)) => (latest - first + 1) - entries,
            _ => 0,
        };

        let gap_dates: Vec<i64> = sqlx::query_scalar(
            "WITH RECURSIVE span(day) AS (
                 SELECT MIN(date_id) FROM entries
                 UNION ALL
                 SELECT day + 1 FROM span WHERE day < (SELECT MAX(date_id) FROM entries)
             )
             SELECT day FROM span
             WHERE day NOT IN (SELECT date_id FROM entries)
             ORDER BY day",
        )
        .fetch_all(self.db.reader())
        .await?;

        Ok(Stats {
            entries,
            thumbnails,
            first: first.map(|days| ApodDate::from_days(days as i32)),
            latest: latest.map(|days| ApodDate::from_days(days as i32)),
            by_media_kind,
            copyright,
            licensed,
            gaps,
            gap_dates: to_dates(gap_dates),
            text: self.text_summary().await?,
            resources: self.resource_summary().await?,
            pictures: self.picture_summary().await?,
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

    pub async fn provenance_counts(&self) -> ApodResult<Vec<(Provenance, i64)>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT provenance, COUNT(*) FROM entries GROUP BY provenance ORDER BY 2 DESC",
        )
        .fetch_all(self.db.reader())
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(name, count)| Some((name.parse().ok()?, count)))
            .collect())
    }

    async fn filtered(
        &self,
        filters: &Filters,
        offset: usize,
        limit: usize,
    ) -> ApodResult<SearchResults> {
        let mut where_clause = String::from("WHERE 1 = 1");
        let mut params: Vec<Param> = Vec::new();
        push_filters(&mut where_clause, &mut params, filters);

        let total: i64 = sqlx::query_scalar_with(
            AssertSqlSafe(format!("SELECT COUNT(*) FROM entries {where_clause}")),
            arguments(&params),
        )
        .fetch_one(self.db.reader())
        .await?;

        let sql = format!(
            "SELECT {SUMMARY_COLUMNS} FROM entries {where_clause}
             ORDER BY date_id DESC LIMIT ? OFFSET ?"
        );
        params.push(Param::Int(limit as i64));
        params.push(Param::Int(offset as i64));

        let rows = sqlx::query_with(AssertSqlSafe(sql), arguments(&params))
            .fetch_all(self.db.reader())
            .await?;

        Ok(SearchResults {
            items: rows
                .iter()
                .map(|row| {
                    Ok(SearchHit {
                        entry: self.summary(row)?,
                        snippet: String::new(),
                        matched: Matched::default(),
                        credit: None,
                        keywords: None,
                    })
                })
                .collect::<ApodResult<_>>()?,
            total,
        })
    }

    pub async fn divergence_counts(&self) -> ApodResult<Vec<(String, i64)>> {
        Ok(sqlx::query_as(
            "SELECT field, COUNT(*) FROM divergences GROUP BY field ORDER BY 2 DESC, 1",
        )
        .fetch_all(self.db.reader())
        .await?)
    }

    pub async fn entries_by_year(&self) -> ApodResult<Vec<(i32, i64)>> {
        Ok(sqlx::query_as(
            "SELECT CAST(substr(date, 1, 4) AS INTEGER) AS year, COUNT(*)
             FROM entries GROUP BY year ORDER BY year",
        )
        .fetch_all(self.db.reader())
        .await?)
    }

    pub async fn divergent_entries(&self) -> ApodResult<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(DISTINCT date_id) FROM divergences")
                .fetch_one(self.db.reader())
                .await?,
        )
    }

    pub async fn divergences(
        &self,
        field: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> ApodResult<Listing<FieldDivergence>> {
        let total: i64 = match field {
            Some(field) => {
                sqlx::query_scalar("SELECT COUNT(*) FROM divergences WHERE field = ?1")
                    .bind(field)
                    .fetch_one(self.db.reader())
                    .await?
            }
            None => {
                sqlx::query_scalar("SELECT COUNT(*) FROM divergences")
                    .fetch_one(self.db.reader())
                    .await?
            }
        };

        let rows = sqlx::query(
            "SELECT d.date_id, e.title, d.field, d.legacy_value, d.modern_value
             FROM divergences d JOIN entries e ON e.date_id = d.date_id
             WHERE (?1 IS NULL OR d.field = ?1)
             ORDER BY d.date_id DESC, d.field LIMIT ?2 OFFSET ?3",
        )
        .bind(field)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(self.db.reader())
        .await?;

        Ok(Listing {
            items: rows
                .iter()
                .map(read_divergence)
                .collect::<ApodResult<_>>()?,
            total,
        })
    }

    pub async fn entry_divergences(&self, date: ApodDate) -> ApodResult<Vec<FieldDivergence>> {
        let rows = sqlx::query(
            "SELECT d.date_id, e.title, d.field, d.legacy_value, d.modern_value
             FROM divergences d JOIN entries e ON e.date_id = d.date_id
             WHERE d.date_id = ?1 ORDER BY d.field",
        )
        .bind(date.days())
        .fetch_all(self.db.reader())
        .await?;

        rows.iter().map(read_divergence).collect()
    }

    pub async fn origin_pairs(&self) -> ApodResult<Vec<(ApodDate, String, String)>> {
        let rows: Vec<(i64, String, String)> = sqlx::query_as(
            "SELECT date_id, legacy_media_url, media_url FROM entries
             WHERE legacy_media_url IS NOT NULL AND media_url IS NOT NULL
               AND media_kind IN ('image_jpg', 'image_png', 'image_gif', 'image_tiff')
             ORDER BY date_id",
        )
        .fetch_all(self.db.reader())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(days, legacy, modern)| (ApodDate::from_days(days as i32), legacy, modern))
            .collect())
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
        thumb: Option<Thumb>,
    ) -> Media {
        let mut media = Media::new(
            MediaKind::from_str(kind).unwrap_or(MediaKind::None),
            url,
            hd_url,
        );
        media.set_thumb(thumb, self.thumb_base.as_deref());
        media
    }

    pub(super) fn summary(&self, row: &SqliteRow) -> ApodResult<ApodSummary> {
        Ok(ApodSummary {
            date: ApodDate::from_days(row.try_get::<i64, _>(0)? as i32),
            title: row.try_get(1)?,
            has_copyright: row.try_get(2)?,
            media: self.media(
                &row.try_get::<String, _>(3)?,
                row.try_get(4)?,
                row.try_get(5)?,
                read_thumb(row, 6)?,
            ),
            picture: read_picture(row, 9)?,
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
                read_thumb(row, 13)?,
            ),
            extra_media: Vec::new(),
            legacy_media_url: row.try_get(18)?,
            alt: row.try_get(19)?,
            authors: from_json(row.try_get(20)?),
            provenance: row.try_get::<String, _>(21)?.parse().unwrap_or_default(),
            source_url: row.try_get(16)?,
            picture: read_picture(row, 17)?,
            first_stored_at: row.try_get::<Option<i64>, _>(22)?.and_then(stored_at),
        })
    }
}

fn read_divergence(row: &SqliteRow) -> ApodResult<FieldDivergence> {
    Ok(FieldDivergence {
        date: ApodDate::from_days(row.try_get::<i64, _>(0)? as i32),
        title: row.try_get(1)?,
        field: row.try_get(2)?,
        legacy: row.try_get(3)?,
        modern: row.try_get(4)?,
    })
}

fn stored_at(seconds: i64) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::from_timestamp(seconds, 0)
}

fn read_picture(row: &SqliteRow, at: usize) -> ApodResult<Option<ApodDate>> {
    Ok(row
        .try_get::<Option<i64>, _>(at)?
        .map(|days| ApodDate::from_days(days as i32)))
}

pub(super) fn read_thumb(row: &SqliteRow, at: usize) -> ApodResult<Option<Thumb>> {
    let Some(path) = row.try_get::<Option<String>, _>(at)? else {
        return Ok(None);
    };

    Ok(Some(Thumb {
        path,
        width: row
            .try_get::<Option<i64>, _>(at + 1)?
            .map(|size| size as u32),
        height: row
            .try_get::<Option<i64>, _>(at + 2)?
            .map(|size| size as u32),
    }))
}

pub(crate) fn to_dates(days: Vec<i64>) -> Vec<ApodDate> {
    days.into_iter()
        .map(|days| ApodDate::from_days(days as i32))
        .collect()
}

/// Keywords are stored as a JSON array, and FTS highlights that text verbatim. Readers want the
/// list, not its punctuation, and the match markers sit inside the values so they survive this.
fn keyword_list(raw: &str) -> String {
    raw.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split("\",\"")
        .map(|word| word.trim().trim_matches('"'))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
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
pub(super) enum Param {
    Int(i64),
    Text(String),
}

pub(super) fn arguments(params: &[Param]) -> SqliteArguments {
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
    if let Some(kinds) = filters.kind.as_ref().map(KindFilter::kinds) {
        let placeholders = vec!["?"; kinds.len()].join(", ");
        sql.push_str(&format!(" AND entries.media_kind IN ({placeholders})"));
        params.extend(kinds.iter().map(|kind| Param::Text(kind.to_string())));
    }
    if let Some(copyright) = filters.copyright {
        sql.push_str(" AND entries.has_copyright = ?");
        params.push(Param::Int(copyright.into()));
    }
    if let Some(lost) = filters.lost {
        sql.push_str(match lost {
            true => " AND ",
            false => " AND NOT ",
        });
        sql.push_str(&lost_media_sql());
    }
}

fn lost_media_sql() -> String {
    format!(
        "(entries.thumb_path IS NULL AND entries.media_kind <> 'none' \
         AND (entries.media_url IS NULL OR {dead_url}) \
         AND (entries.media_hd_url IS NULL OR {dead_hd}))",
        dead_url = crate::entry::decommissioned_sql("entries.media_url"),
        dead_hd = crate::entry::decommissioned_sql("entries.media_hd_url"),
    )
}

pub(super) fn from_json<T: serde::de::DeserializeOwned + Default>(raw: Option<String>) -> T {
    raw.and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

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
