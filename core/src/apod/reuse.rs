use super::SUMMARY_COLUMNS;
use super::model::{
    Appearance, Changed, Listing, Order, Picture, PictureAppearances, PictureFilters, PictureOrder,
};
use super::read::{ApodReader, ApodResult, Param, arguments, from_json};
use crate::date::ApodDate;
use crate::entry::Credit;
use sqlx::sqlite::SqliteRow;
use sqlx::{AssertSqlSafe, Row};

const PICTURE_COLUMNS: &str = "g.picture_group, g.shown, g.first_id, g.last_id, g.titles, \
                               e.title, e.media_kind, e.media_url, e.media_hd_url, \
                               e.thumb_path, e.thumb_width, e.thumb_height";

const GROUPS: &str = "(SELECT picture_group, COUNT(*) AS shown, MIN(date_id) AS first_id, \
                             MAX(date_id) AS last_id, COUNT(DISTINCT title) AS titles \
                      FROM entries WHERE picture_group IS NOT NULL GROUP BY picture_group) g \
                      JOIN entries e ON e.date_id = g.picture_group";

impl ApodReader {
    pub async fn pictures(
        &self,
        filters: &PictureFilters,
        order: PictureOrder,
        direction: Order,
        offset: usize,
        limit: usize,
    ) -> ApodResult<Listing<Picture>> {
        let mut where_clause = String::from("WHERE 1 = 1");
        let mut params: Vec<Param> = Vec::new();
        push_filters(&mut where_clause, &mut params, filters);

        let total: i64 = sqlx::query_scalar_with(
            AssertSqlSafe(format!("SELECT COUNT(*) FROM {GROUPS} {where_clause}")),
            arguments(&params),
        )
        .fetch_one(self.db().reader())
        .await?;

        let sql = format!(
            "SELECT {PICTURE_COLUMNS} FROM {GROUPS} {where_clause}
             ORDER BY {ordering} LIMIT ? OFFSET ?",
            ordering = ordering(order, direction),
        );
        params.push(Param::Int(limit as i64));
        params.push(Param::Int(offset as i64));

        let rows = sqlx::query_with(AssertSqlSafe(sql), arguments(&params))
            .fetch_all(self.db().reader())
            .await?;

        Ok(Listing {
            items: rows
                .iter()
                .map(|row| self.picture(row))
                .collect::<ApodResult<_>>()?,
            total,
        })
    }

    pub async fn picture_appearances(
        &self,
        date: ApodDate,
    ) -> ApodResult<Option<PictureAppearances>> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT {PICTURE_COLUMNS} FROM {GROUPS}
             WHERE g.picture_group = (SELECT picture_group FROM entries WHERE date_id = ?1)"
        )))
        .bind(date.days())
        .fetch_optional(self.db().reader())
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        let picture = self.picture(&row)?;

        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {SUMMARY_COLUMNS}, explanation_text, credits, \
                    COALESCE(NULLIF(media_hd_url, ''), media_url, '')
             FROM entries WHERE picture_group = ?1 ORDER BY date_id"
        )))
        .bind(picture.id.days())
        .fetch_all(self.db().reader())
        .await?;

        let mut items: Vec<Appearance> = Vec::with_capacity(rows.len());
        let mut previous: Option<(String, String, String, String, ApodDate)> = None;

        for row in &rows {
            let entry = self.summary(row)?;
            let explanation: String = row.try_get(10)?;
            let credit = credit_block(from_json(row.try_get(11)?));
            let file: String = row.try_get(12)?;

            let (changed, since) = match &previous {
                None => (Changed::default(), None),
                Some((title, prev_explanation, prev_credit, prev_file, prev_date)) => (
                    Changed {
                        title: *title != entry.title,
                        explanation: *prev_explanation != explanation,
                        credit: *prev_credit != credit,
                        file: *prev_file != file,
                    },
                    Some(entry.date.days() as i64 - prev_date.days() as i64),
                ),
            };

            previous = Some((entry.title.clone(), explanation, credit, file, entry.date));
            items.push(Appearance {
                entry,
                changed,
                since_previous_days: since,
            });
        }

        Ok(Some(PictureAppearances { picture, items }))
    }

    fn picture(&self, row: &SqliteRow) -> ApodResult<Picture> {
        let first = ApodDate::from_days(row.try_get::<i64, _>(2)? as i32);
        let last = ApodDate::from_days(row.try_get::<i64, _>(3)? as i32);

        Ok(Picture {
            id: ApodDate::from_days(row.try_get::<i64, _>(0)? as i32),
            appearances: row.try_get(1)?,
            first,
            last,
            titles: row.try_get(4)?,
            title: row.try_get(5)?,
            media: self.media(
                &row.try_get::<String, _>(6)?,
                row.try_get(7)?,
                row.try_get(8)?,
                super::read::read_thumb(row, 9)?,
            ),
            span_days: last.days() as i64 - first.days() as i64,
        })
    }
}

fn push_filters(where_clause: &mut String, params: &mut Vec<Param>, filters: &PictureFilters) {
    if let Some(query) = filters
        .query
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
    {
        where_clause.push_str(
            " AND EXISTS (SELECT 1 FROM entries r \
               WHERE r.picture_group = g.picture_group AND r.title LIKE ? ESCAPE '\\')",
        );
        params.push(Param::Text(format!(
            "%{}%",
            super::catalogue::escape_like(query)
        )));
    }

    if let Some(min) = filters.min_appearances {
        where_clause.push_str(" AND g.shown >= ?");
        params.push(Param::Int(min));
    }

    match filters.retitled {
        Some(true) => where_clause.push_str(" AND g.titles > 1"),
        Some(false) => where_clause.push_str(" AND g.titles = 1"),
        None => {}
    }
}

fn ordering(order: PictureOrder, direction: Order) -> String {
    let sort = match direction {
        Order::Asc => "ASC",
        Order::Desc => "DESC",
    };

    let column = match order {
        PictureOrder::Appearances => "g.shown",
        PictureOrder::First => "g.first_id",
        PictureOrder::Last => "g.last_id",
        PictureOrder::Span => "g.last_id - g.first_id",
        PictureOrder::Title => "e.title COLLATE NOCASE",
    };

    format!("{column} {sort}, g.picture_group ASC")
}

fn credit_block(credits: Vec<Credit>) -> String {
    credits
        .into_iter()
        .map(|credit| format!("{}: {}", credit.role, credit.text))
        .collect::<Vec<_>>()
        .join("\n")
}
