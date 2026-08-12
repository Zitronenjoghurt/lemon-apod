use super::SUMMARY_COLUMNS;
use super::model::{
    AnchorCount, HostCount, Listing, Order, Resource, ResourceFilters, ResourceOrder, ResourceRef,
    ResourceRefs,
};
use super::read::{ApodReader, ApodResult, Param, arguments};
use crate::date::ApodDate;
use crate::resource;
use sqlx::sqlite::SqliteRow;
use sqlx::{AssertSqlSafe, Row};

const RESOURCE_COLUMNS: &str = "r.id, r.scheme, r.key, r.host, r.label, r.refs, r.entries, \
                                r.credited, r.first_id, r.last_id";

const ANCHOR_LIMIT: i64 = 12;

impl ApodReader {
    pub async fn resources(
        &self,
        filters: &ResourceFilters,
        order: ResourceOrder,
        direction: Order,
        offset: usize,
        limit: usize,
    ) -> ApodResult<Listing<Resource>> {
        let mut where_clause = String::from("WHERE 1 = 1");
        let mut params: Vec<Param> = Vec::new();
        push_filters(&mut where_clause, &mut params, filters);

        let total: i64 = sqlx::query_scalar_with(
            AssertSqlSafe(format!("SELECT COUNT(*) FROM resources r {where_clause}")),
            arguments(&params),
        )
        .fetch_one(self.db().reader())
        .await?;

        let sql = format!(
            "SELECT {RESOURCE_COLUMNS} FROM resources r {where_clause}
             ORDER BY {ordering} LIMIT ? OFFSET ?",
            ordering = ordering(order, direction),
        );
        params.push(Param::Int(limit as i64));
        params.push(Param::Int(offset as i64));

        let rows = sqlx::query_with(AssertSqlSafe(sql), arguments(&params))
            .fetch_all(self.db().reader())
            .await?;

        Ok(Listing {
            items: rows.iter().map(read_resource).collect::<Result<_, _>>()?,
            total,
        })
    }

    pub async fn resource(
        &self,
        id: i64,
        offset: usize,
        limit: usize,
    ) -> ApodResult<Option<ResourceRefs>> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT {RESOURCE_COLUMNS} FROM resources r WHERE r.id = ?1"
        )))
        .bind(id)
        .fetch_optional(self.db().reader())
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        let resource = read_resource(&row)?;
        let total = resource.entries;

        let columns: Vec<String> = SUMMARY_COLUMNS
            .split(", ")
            .map(|column| format!("entries.{column}"))
            .collect();
        let extra = columns.len();

        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {columns}, er.anchor, er.in_credit, er.n
             FROM entry_resources er JOIN entries ON entries.date_id = er.date_id
             WHERE er.resource_id = ?1
             ORDER BY er.date_id DESC LIMIT ?2 OFFSET ?3",
            columns = columns.join(", "),
        )))
        .bind(id)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(self.db().reader())
        .await?;

        let items = rows
            .iter()
            .map(|row| {
                Ok(ResourceRef {
                    entry: self.summary(row)?,
                    anchor: row.try_get(extra)?,
                    in_credit: row.try_get(extra + 1)?,
                    count: row.try_get(extra + 2)?,
                })
            })
            .collect::<ApodResult<Vec<_>>>()?;

        Ok(Some(ResourceRefs {
            resource,
            items,
            total,
            anchors: self.resource_anchors(id).await?,
        }))
    }

    async fn resource_anchors(&self, id: i64) -> ApodResult<Vec<AnchorCount>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT anchor, COUNT(*) FROM entry_resources
             WHERE resource_id = ?1 AND anchor <> ''
             GROUP BY anchor ORDER BY COUNT(*) DESC, length(anchor) ASC, anchor ASC
             LIMIT ?2",
        )
        .bind(id)
        .bind(ANCHOR_LIMIT)
        .fetch_all(self.db().reader())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(anchor, entries)| AnchorCount { anchor, entries })
            .collect())
    }

    pub async fn resource_hosts(&self, limit: usize) -> ApodResult<Vec<HostCount>> {
        let rows = sqlx::query_as::<_, (String, i64, i64)>(
            "SELECT host, COUNT(*), SUM(refs) FROM resources
             GROUP BY host ORDER BY SUM(refs) DESC, host ASC LIMIT ?1",
        )
        .bind(limit as i64)
        .fetch_all(self.db().reader())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(host, resources, refs)| HostCount {
                host,
                resources,
                refs,
            })
            .collect())
    }
}

fn ordering(order: ResourceOrder, direction: Order) -> &'static str {
    match (order, direction) {
        (ResourceOrder::Refs, Order::Desc) => "r.refs DESC, r.entries DESC, r.id ASC",
        (ResourceOrder::Refs, Order::Asc) => "r.refs ASC, r.entries ASC, r.id ASC",
        (ResourceOrder::Entries, Order::Desc) => "r.entries DESC, r.refs DESC, r.id ASC",
        (ResourceOrder::Entries, Order::Asc) => "r.entries ASC, r.refs ASC, r.id ASC",
        (ResourceOrder::First, Order::Desc) => "r.first_id DESC, r.id ASC",
        (ResourceOrder::First, Order::Asc) => "r.first_id ASC, r.id ASC",
        (ResourceOrder::Last, Order::Desc) => "r.last_id DESC, r.id ASC",
        (ResourceOrder::Last, Order::Asc) => "r.last_id ASC, r.id ASC",
        (ResourceOrder::Address, Order::Desc) => "r.key DESC",
        (ResourceOrder::Address, Order::Asc) => "r.key ASC",
        (ResourceOrder::Label, Order::Desc) => {
            "r.label IS NULL, r.label COLLATE NOCASE DESC, r.key ASC"
        }
        (ResourceOrder::Label, Order::Asc) => {
            "r.label IS NULL, r.label COLLATE NOCASE ASC, r.key ASC"
        }
    }
}

fn push_filters(sql: &mut String, params: &mut Vec<Param>, filters: &ResourceFilters) {
    if let Some(host) = &filters.host {
        sql.push_str(" AND r.host = ?");
        params.push(Param::Text(host.to_ascii_lowercase()));
    }
    if let Some(min_refs) = filters.min_refs {
        sql.push_str(" AND r.refs >= ?");
        params.push(Param::Int(min_refs));
    }
    if let Some(credited) = filters.credited {
        sql.push_str(if credited {
            " AND r.credited > 0"
        } else {
            " AND r.credited = 0"
        });
    }

    let Some(query) = filters
        .query
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
    else {
        return;
    };

    sql.push_str(
        " AND (r.key LIKE ? ESCAPE '\\' OR r.label LIKE ? ESCAPE '\\'
              OR r.id IN (SELECT resource_id FROM entry_resources
                           WHERE anchor LIKE ? ESCAPE '\\'))",
    );
    let pattern = contains(query);
    for _ in 0..3 {
        params.push(Param::Text(pattern.clone()));
    }
}

pub(super) fn contains(query: &str) -> String {
    format!("%{}%", escape_like(query))
}

pub(super) fn escape_like(query: &str) -> String {
    query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn read_resource(row: &SqliteRow) -> ApodResult<Resource> {
    let key: String = row.try_get(2)?;

    Ok(Resource {
        id: row.try_get(0)?,
        url: resource::url(&row.try_get::<String, _>(1)?, &key),
        key,
        host: row.try_get(3)?,
        label: row
            .try_get::<Option<String>, _>(4)?
            .filter(|label| !label.is_empty()),
        refs: row.try_get(5)?,
        entries: row.try_get(6)?,
        credited: row.try_get(7)?,
        first: row.try_get::<Option<i64>, _>(8)?.map(days),
        last: row.try_get::<Option<i64>, _>(9)?.map(days),
    })
}

pub(super) fn days(days: i64) -> ApodDate {
    ApodDate::from_days(days as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_wildcard_is_searched_for_rather_than_matched_with() {
        assert_eq!(contains("100%"), "%100\\%%");
        assert_eq!(contains("a_b"), "%a\\_b%");
        assert_eq!(contains("back\\slash"), "%back\\\\slash%");
    }
}
