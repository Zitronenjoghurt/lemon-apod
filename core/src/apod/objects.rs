use super::model::{CatalogCount, ObjectCount, ObjectOrder};
use super::read::{ApodReader, ApodResult};
use crate::date::ApodDate;
use crate::entry::ApodSummary;
use sqlx::sqlite::SqliteRow;
use sqlx::{AssertSqlSafe, Row};
use std::str::FromStr;

impl ObjectOrder {
    fn sql(self) -> &'static str {
        match self {
            Self::Entries => "entries DESC, id ASC",
            Self::Latest => "last DESC, entries DESC, id ASC",
            Self::Designation => "catalog ASC, number ASC",
        }
    }
}

impl FromStr for ObjectOrder {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Ok(match raw {
            "entries" => Self::Entries,
            "latest" => Self::Latest,
            "designation" => Self::Designation,
            _ => return Err(()),
        })
    }
}

const NUMBER: &str = "CAST(ltrim(replace(replace(object, 'Sh2-', ''), ' ', ''),
                                'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz/')
                      AS INTEGER)";

const COUNTS: &str = "COUNT(*) AS entries, MIN(date_id) AS first, MAX(date_id) AS last";

impl ApodReader {
    pub async fn objects(
        &self,
        query: Option<&str>,
        catalog: Option<&str>,
        order: ObjectOrder,
        offset: usize,
        limit: usize,
    ) -> ApodResult<Vec<ObjectCount>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT object AS id, MAX(catalog) AS catalog, {COUNTS}, {NUMBER} AS number
             FROM entry_objects
             {}
             GROUP BY object
             ORDER BY {}
             LIMIT ?3 OFFSET ?4",
            matching_objects(query.is_some(), catalog.is_some()),
            order.sql()
        )))
        .bind(like(query))
        .bind(catalog.unwrap_or_default())
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(self.db().reader())
        .await?;

        rows.iter().map(read).collect()
    }

    pub async fn object_count(
        &self,
        query: Option<&str>,
        catalog: Option<&str>,
    ) -> ApodResult<i64> {
        Ok(sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT COUNT(DISTINCT object) FROM entry_objects {}",
            matching_objects(query.is_some(), catalog.is_some())
        )))
        .bind(like(query))
        .bind(catalog.unwrap_or_default())
        .fetch_one(self.db().reader())
        .await?)
    }

    pub async fn object_catalogs(&self) -> ApodResult<Vec<CatalogCount>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT catalog, COUNT(DISTINCT object) AS objects FROM entry_objects
             GROUP BY catalog
             ORDER BY objects DESC, catalog ASC",
        )
        .fetch_all(self.db().reader())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(catalog, objects)| CatalogCount { catalog, objects })
            .collect())
    }

    pub async fn object(&self, id: &str) -> ApodResult<Option<ObjectCount>> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT object AS id, MAX(catalog) AS catalog, {COUNTS}
             FROM entry_objects WHERE object = ?1 GROUP BY object"
        )))
        .bind(id)
        .fetch_optional(self.db().reader())
        .await?;

        row.as_ref().map(read).transpose()
    }

    /// Nothing is filtered out: a bare NASA tag is sometimes a misattribution and sometimes the
    /// only place a second designation such as NGC 5457 for M101 appears.
    pub async fn showing(
        &self,
        id: &str,
        offset: usize,
        limit: usize,
    ) -> ApodResult<Vec<ApodSummary>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {} FROM entry_objects AS o
             JOIN entries AS e ON e.date_id = o.date_id
             WHERE o.object = ?1
             ORDER BY o.score DESC, o.date_id DESC
             LIMIT ?2 OFFSET ?3",
            super::summary_columns_qualified("e"),
        )))
        .bind(id)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(self.db().reader())
        .await?;

        rows.iter().map(|row| self.summary(row)).collect()
    }

    pub async fn objects_for(&self, date: ApodDate) -> ApodResult<Vec<ObjectCount>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT o.object AS id, o.catalog AS catalog,
                    (SELECT COUNT(*) FROM entry_objects AS every
                     WHERE every.object = o.object) AS entries,
                    (SELECT MIN(date_id) FROM entry_objects AS every
                     WHERE every.object = o.object) AS first,
                    (SELECT MAX(date_id) FROM entry_objects AS every
                     WHERE every.object = o.object) AS last
             FROM entry_objects AS o
             WHERE o.date_id = ?1
             ORDER BY o.score DESC, {NUMBER} ASC"
        )))
        .bind(date.days())
        .fetch_all(self.db().reader())
        .await?;

        rows.iter().map(read).collect()
    }

    pub async fn entries_with_objects(&self) -> ApodResult<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(DISTINCT date_id) FROM entry_objects")
                .fetch_one(self.db().reader())
                .await?,
        )
    }
}

fn where_clause(has_query: bool, has_catalog: bool) -> &'static str {
    match (has_query, has_catalog) {
        (true, true) => {
            "WHERE (object LIKE ?1 ESCAPE '\\' OR name LIKE ?1 ESCAPE '\\') AND catalog = ?2"
        }
        (true, false) => "WHERE (object LIKE ?1 ESCAPE '\\' OR name LIKE ?1 ESCAPE '\\')",
        (false, true) => "WHERE catalog = ?2",
        (false, false) => "",
    }
}

fn matching_objects(has_query: bool, has_catalog: bool) -> String {
    match where_clause(has_query, has_catalog) {
        "" => String::new(),
        filter => format!("WHERE object IN (SELECT object FROM entry_objects {filter})"),
    }
}

/// Bound even when there is no query, so the placeholders after it keep their numbers.
fn like(query: Option<&str>) -> String {
    query.map(super::catalogue::contains).unwrap_or_default()
}

fn read(row: &SqliteRow) -> ApodResult<ObjectCount> {
    Ok(ObjectCount {
        id: row.try_get("id")?,
        catalog: row.try_get("catalog")?,
        entries: row.try_get::<Option<i64>, _>("entries")?.unwrap_or(0),
        first: ApodDate::from_days(row.try_get::<i64, _>("first")? as i32),
        last: ApodDate::from_days(row.try_get::<i64, _>("last")? as i32),
    })
}
