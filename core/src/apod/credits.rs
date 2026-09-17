use super::model::{Contributor, CreditOrder, RoleCount};
use super::read::{ApodReader, ApodResult};
use crate::contributor::Kind;
use crate::date::ApodDate;
use crate::entry::ApodSummary;
use sqlx::sqlite::SqliteRow;
use sqlx::{AssertSqlSafe, Row};
use std::str::FromStr;

impl CreditOrder {
    fn sql(self) -> &'static str {
        match self {
            Self::Entries => "entries DESC, id ASC",
            Self::Latest => "last DESC, entries DESC, id ASC",
            Self::Name => "label COLLATE NOCASE ASC, id ASC",
        }
    }
}

impl FromStr for CreditOrder {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Ok(match raw {
            "entries" => Self::Entries,
            "latest" => Self::Latest,
            "name" => Self::Name,
            _ => return Err(()),
        })
    }
}

const STRONGEST_KIND: &str = "CASE
                      WHEN MAX(kind = 'group') THEN 'group'
                      WHEN MAX(kind = 'person') THEN 'person'
                      ELSE 'unknown'
                    END";

const LABEL: &str = "(SELECT name FROM entry_credits AS pick
                      WHERE pick.contributor = c.contributor
                      GROUP BY name
                      ORDER BY COUNT(*) DESC, length(name) ASC
                      LIMIT 1)";

const URL: &str = "(SELECT url FROM entry_credits AS pick
                    WHERE pick.contributor = c.contributor AND pick.url IS NOT NULL
                    GROUP BY url
                    ORDER BY COUNT(*) DESC, length(url) ASC
                    LIMIT 1)";

impl ApodReader {
    pub async fn contributors(
        &self,
        query: Option<&str>,
        kind: Option<Kind>,
        order: CreditOrder,
        offset: usize,
        limit: usize,
    ) -> ApodResult<Vec<Contributor>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT contributor AS id, {LABEL} AS label, {STRONGEST_KIND} AS kind,
                    COUNT(*) AS entries, MIN(date_id) AS first, MAX(date_id) AS last, {URL} AS url
             FROM entry_credits AS c
             {}
             GROUP BY contributor
             {}
             ORDER BY {}
             LIMIT ?2 OFFSET ?3",
            searching(query.is_some()),
            having(kind),
            order.sql()
        )))
        .bind(like(query))
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(self.db().reader())
        .await?;

        rows.iter().map(read).collect()
    }

    pub async fn contributor_count(
        &self,
        query: Option<&str>,
        kind: Option<Kind>,
    ) -> ApodResult<i64> {
        Ok(sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT COUNT(*) FROM
               (SELECT {STRONGEST_KIND} AS kind FROM entry_credits AS c {}
                GROUP BY contributor {})",
            searching(query.is_some()),
            having(kind)
        )))
        .bind(like(query))
        .fetch_one(self.db().reader())
        .await?)
    }

    pub async fn contributor_roles(&self, id: &str) -> ApodResult<Vec<RoleCount>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT role, COUNT(*) AS entries FROM entry_credits
             WHERE contributor = ?1
             GROUP BY role
             ORDER BY entries DESC, role ASC",
        )
        .bind(id)
        .fetch_all(self.db().reader())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(role, entries)| RoleCount { role, entries })
            .collect())
    }

    pub async fn contributor(&self, id: &str) -> ApodResult<Option<Contributor>> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT contributor AS id, {LABEL} AS label, {STRONGEST_KIND} AS kind,
                    COUNT(*) AS entries, MIN(date_id) AS first, MAX(date_id) AS last, {URL} AS url
             FROM entry_credits AS c
             WHERE contributor = ?1
             GROUP BY contributor"
        )))
        .bind(id)
        .fetch_optional(self.db().reader())
        .await?;

        row.as_ref().map(read).transpose()
    }

    pub async fn credited(
        &self,
        id: &str,
        offset: usize,
        limit: usize,
    ) -> ApodResult<Vec<(ApodSummary, String)>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {}, c.role AS role FROM entry_credits AS c
             JOIN entries AS e ON e.date_id = c.date_id
             WHERE c.contributor = ?1
             ORDER BY c.date_id DESC
             LIMIT ?2 OFFSET ?3",
            super::summary_columns_qualified("e")
        )))
        .bind(id)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(self.db().reader())
        .await?;

        rows.iter()
            .map(|row| Ok((self.summary(row)?, row.try_get("role")?)))
            .collect()
    }

    pub async fn credits_for(&self, date: ApodDate) -> ApodResult<Vec<Contributor>> {
        let rows = sqlx::query(
            "SELECT c.contributor AS id, c.name AS label, c.kind AS kind, c.url AS url,
                    (SELECT COUNT(*) FROM entry_credits AS every
                     WHERE every.contributor = c.contributor) AS entries,
                    (SELECT MIN(date_id) FROM entry_credits AS every
                     WHERE every.contributor = c.contributor) AS first,
                    (SELECT MAX(date_id) FROM entry_credits AS every
                     WHERE every.contributor = c.contributor) AS last
             FROM entry_credits AS c
             WHERE c.date_id = ?1
             ORDER BY entries DESC, c.contributor ASC",
        )
        .bind(date.days())
        .fetch_all(self.db().reader())
        .await?;

        rows.iter().map(read).collect()
    }

    pub async fn credited_entries(&self) -> ApodResult<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(DISTINCT date_id) FROM entry_credits")
                .fetch_one(self.db().reader())
                .await?,
        )
    }
}

fn searching(has_query: bool) -> &'static str {
    match has_query {
        true => "WHERE (c.contributor LIKE ?1 ESCAPE '\\' OR c.name LIKE ?1 ESCAPE '\\')",
        false => "",
    }
}

fn having(kind: Option<Kind>) -> &'static str {
    match kind {
        Some(Kind::Person) => "HAVING kind = 'person'",
        Some(Kind::Group) => "HAVING kind = 'group'",
        Some(Kind::Unknown) => "HAVING kind = 'unknown'",
        None => "",
    }
}

/// Bound even when there is no query, so the placeholders after it keep their numbers.
fn like(query: Option<&str>) -> String {
    query.map(super::catalogue::contains).unwrap_or_default()
}

fn read(row: &SqliteRow) -> ApodResult<Contributor> {
    Ok(Contributor {
        id: row.try_get("id")?,
        label: row.try_get("label")?,
        kind: row.try_get("kind")?,
        entries: row.try_get("entries")?,
        first: ApodDate::from_days(row.try_get::<i64, _>("first")? as i32),
        last: ApodDate::from_days(row.try_get::<i64, _>("last")? as i32),
        url: row.try_get("url")?,
    })
}
