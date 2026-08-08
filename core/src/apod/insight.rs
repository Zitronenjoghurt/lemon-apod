use super::catalogue::{contains, days, escape_like};
use super::model::{
    Coverage, EntryLength, Listing, MonthCount, Order, ResourceSummary, TextSummary, Timeline,
    Word, WordEntry, WordFilters, WordOrder, WordUse, YearCount, YearStats,
};
use super::read::{ApodReader, ApodResult, Param, arguments};
use sqlx::{AssertSqlSafe, Row};

const YEAR: &str = "CAST(substr(entries.date, 1, 4) AS INTEGER)";
const MONTH: &str = "CAST(substr(entries.date, 6, 2) AS INTEGER)";

impl ApodReader {
    pub async fn text_summary(&self) -> ApodResult<TextSummary> {
        let (measured, total_words, avg_words, min_words, max_words): (i64, i64, f64, i64, i64) =
            sqlx::query_as(
                "SELECT COUNT(*), COALESCE(SUM(word_count), 0), COALESCE(AVG(word_count), 0),
                        COALESCE(MIN(word_count), 0), COALESCE(MAX(word_count), 0)
                 FROM entry_stats",
            )
            .fetch_one(self.db().reader())
            .await?;

        if measured == 0 {
            return Ok(TextSummary::default());
        }

        let (avg_unique, avg_chars, avg_sentences, sentences, avg_links): (
            f64,
            f64,
            f64,
            i64,
            f64,
        ) = sqlx::query_as(
            "SELECT COALESCE(AVG(unique_words), 0), COALESCE(AVG(char_count), 0),
                    COALESCE(AVG(sentences), 0), COALESCE(SUM(sentences), 0),
                    COALESCE(AVG(link_count), 0)
             FROM entry_stats",
        )
        .fetch_one(self.db().reader())
        .await?;

        let median_words: i64 = sqlx::query_scalar(
            "SELECT word_count FROM entry_stats ORDER BY word_count
             LIMIT 1 OFFSET (SELECT COUNT(*) / 2 FROM entry_stats)",
        )
        .fetch_one(self.db().reader())
        .await?;

        let (distinct_words, used_once): (i64, i64) =
            sqlx::query_as("SELECT COUNT(*), COALESCE(SUM(total = 1), 0) FROM words")
                .fetch_one(self.db().reader())
                .await?;

        Ok(TextSummary {
            measured,
            total_words,
            distinct_words,
            avg_words,
            median_words,
            min_words,
            max_words,
            avg_unique_words: avg_unique,
            avg_chars,
            avg_sentences,
            avg_words_per_sentence: ratio(total_words, sentences),
            avg_links,
            used_once,
            shortest: self.extreme(Order::Asc).await?,
            longest: self.extreme(Order::Desc).await?,
        })
    }

    pub async fn resource_summary(&self) -> ApodResult<ResourceSummary> {
        let (resources, hosts, references, referenced_once): (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), COUNT(DISTINCT host), COALESCE(SUM(refs), 0),
                        COALESCE(SUM(entries = 1), 0)
                 FROM resources",
        )
        .fetch_one(self.db().reader())
        .await?;

        Ok(ResourceSummary {
            resources,
            hosts,
            references,
            referenced_once,
        })
    }

    pub async fn timeline(&self) -> ApodResult<Timeline> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {YEAR} AS year, COUNT(*), COUNT(s.date_id),
                    COALESCE(SUM(s.word_count), 0), COALESCE(AVG(s.word_count), 0),
                    COALESCE(MIN(s.word_count), 0), COALESCE(MAX(s.word_count), 0),
                    COALESCE(AVG(s.sentences), 0), COALESCE(SUM(s.sentences), 0),
                    COALESCE(AVG(s.link_count), 0),
                    SUM(entries.has_copyright),
                    SUM(entries.media_kind IN ('image_jpg', 'image_png', 'image_gif')),
                    SUM(entries.media_kind IN ('video_mp4', 'youtube', 'vimeo'))
             FROM entries LEFT JOIN entry_stats s ON s.date_id = entries.date_id
             GROUP BY year ORDER BY year"
        )))
        .fetch_all(self.db().reader())
        .await?;

        let distinct = self
            .words_per_year("COUNT(DISTINCT entry_words.word)")
            .await?;
        let fresh = self.new_words_per_year().await?;

        let years = rows
            .iter()
            .map(|row| {
                let year: i32 = row.try_get(0)?;
                let total_words: i64 = row.try_get(3)?;
                let sentences: i64 = row.try_get(8)?;

                Ok(YearStats {
                    year,
                    entries: row.try_get(1)?,
                    measured: row.try_get(2)?,
                    total_words,
                    distinct_words: lookup(&distinct, year),
                    new_words: lookup(&fresh, year),
                    avg_words: row.try_get(4)?,
                    min_words: row.try_get(5)?,
                    max_words: row.try_get(6)?,
                    avg_sentences: row.try_get(7)?,
                    avg_words_per_sentence: ratio(total_words, sentences),
                    avg_links: row.try_get(9)?,
                    copyright: row.try_get(10)?,
                    images: row.try_get(11)?,
                    videos: row.try_get(12)?,
                })
            })
            .collect::<ApodResult<Vec<_>>>()?;

        Ok(Timeline { years })
    }

    pub async fn coverage(&self) -> ApodResult<Coverage> {
        let rows = sqlx::query_as::<_, (i32, i32, i64)>(AssertSqlSafe(format!(
            "SELECT {YEAR} AS year, {MONTH} AS month, COUNT(*)
             FROM entries GROUP BY year, month ORDER BY year, month"
        )))
        .fetch_all(self.db().reader())
        .await?;

        Ok(Coverage {
            months: rows
                .into_iter()
                .map(|(year, month, entries)| MonthCount {
                    year,
                    month: month.unsigned_abs(),
                    entries,
                })
                .collect(),
        })
    }

    pub async fn words(
        &self,
        filters: &WordFilters,
        order: WordOrder,
        direction: Order,
        offset: usize,
        limit: usize,
    ) -> ApodResult<Listing<Word>> {
        let mut where_clause = String::from("WHERE 1 = 1");
        let mut params: Vec<Param> = Vec::new();

        if let Some(query) = filters
            .query
            .as_deref()
            .map(str::trim)
            .filter(|q| !q.is_empty())
        {
            where_clause.push_str(" AND word LIKE ? ESCAPE '\\'");
            params.push(Param::Text(pattern(query)));
        }
        if let Some(min) = filters.min_total {
            where_clause.push_str(" AND total >= ?");
            params.push(Param::Int(min));
        }
        if let Some(max) = filters.max_total {
            where_clause.push_str(" AND total <= ?");
            params.push(Param::Int(max));
        }

        let total: i64 = sqlx::query_scalar_with(
            AssertSqlSafe(format!("SELECT COUNT(*) FROM words {where_clause}")),
            arguments(&params),
        )
        .fetch_one(self.db().reader())
        .await?;

        let sql = format!(
            "SELECT word, total, entries FROM words {where_clause}
             ORDER BY {ordering} LIMIT ? OFFSET ?",
            ordering = match (order, direction) {
                (WordOrder::Total, Order::Desc) => "total DESC, word ASC",
                (WordOrder::Total, Order::Asc) => "total ASC, word ASC",
                (WordOrder::Entries, Order::Desc) => "entries DESC, word ASC",
                (WordOrder::Entries, Order::Asc) => "entries ASC, word ASC",
                (WordOrder::Alphabetical, Order::Desc) => "word DESC",
                (WordOrder::Alphabetical, Order::Asc) => "word ASC",
            },
        );
        params.push(Param::Int(limit as i64));
        params.push(Param::Int(offset as i64));

        let items =
            sqlx::query_as_with::<_, (String, i64, i64), _>(AssertSqlSafe(sql), arguments(&params))
                .fetch_all(self.db().reader())
                .await?
                .into_iter()
                .map(|(word, total, entries)| Word {
                    word,
                    total,
                    entries,
                })
                .collect();

        Ok(Listing { items, total })
    }

    pub async fn word(&self, word: &str, top_entries: usize) -> ApodResult<Option<WordUse>> {
        let word = word.trim().to_lowercase();

        let Some((total, entries)) =
            sqlx::query_as::<_, (i64, i64)>("SELECT total, entries FROM words WHERE word = ?1")
                .bind(&word)
                .fetch_optional(self.db().reader())
                .await?
        else {
            return Ok(None);
        };

        let (first, last): (Option<i64>, Option<i64>) =
            sqlx::query_as("SELECT MIN(date_id), MAX(date_id) FROM entry_words WHERE word = ?1")
                .bind(&word)
                .fetch_one(self.db().reader())
                .await?;

        let by_year = sqlx::query_as::<_, (i32, i64, i64)>(AssertSqlSafe(format!(
            "SELECT {YEAR} AS year, SUM(entry_words.n), COUNT(*)
             FROM entry_words JOIN entries ON entries.date_id = entry_words.date_id
             WHERE entry_words.word = ?1 GROUP BY year ORDER BY year"
        )))
        .bind(&word)
        .fetch_all(self.db().reader())
        .await?
        .into_iter()
        .map(|(year, total, entries)| YearCount {
            year,
            total,
            entries,
        })
        .collect();

        let top = sqlx::query_as::<_, (i64, String, i64)>(
            "SELECT entries.date_id, entries.title, entry_words.n
             FROM entry_words JOIN entries ON entries.date_id = entry_words.date_id
             WHERE entry_words.word = ?1
             ORDER BY entry_words.n DESC, entries.date_id DESC LIMIT ?2",
        )
        .bind(&word)
        .bind(top_entries as i64)
        .fetch_all(self.db().reader())
        .await?
        .into_iter()
        .map(|(date_id, title, count)| WordEntry {
            date: days(date_id),
            title,
            count,
        })
        .collect();

        Ok(Some(WordUse {
            word: Word {
                word,
                total,
                entries,
            },
            first: first.map(days),
            last: last.map(days),
            by_year,
            top_entries: top,
        }))
    }

    async fn extreme(&self, direction: Order) -> ApodResult<Option<EntryLength>> {
        let sql = format!(
            "SELECT s.date_id, entries.title, s.word_count
             FROM entry_stats s JOIN entries ON entries.date_id = s.date_id
             ORDER BY s.word_count {}, s.date_id ASC LIMIT 1",
            match direction {
                Order::Asc => "ASC",
                Order::Desc => "DESC",
            }
        );

        Ok(sqlx::query_as::<_, (i64, String, i64)>(AssertSqlSafe(sql))
            .fetch_optional(self.db().reader())
            .await?
            .map(|(date_id, title, word_count)| EntryLength {
                date: days(date_id),
                title,
                word_count,
            }))
    }

    async fn words_per_year(&self, measure: &str) -> ApodResult<Vec<(i32, i64)>> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {YEAR} AS year, {measure}
             FROM entry_words JOIN entries ON entries.date_id = entry_words.date_id
             GROUP BY year ORDER BY year"
        )))
        .fetch_all(self.db().reader())
        .await?)
    }

    async fn new_words_per_year(&self) -> ApodResult<Vec<(i32, i64)>> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {YEAR} AS year, COUNT(*)
             FROM (SELECT MIN(date_id) AS date_id FROM entry_words GROUP BY word) first
             JOIN entries ON entries.date_id = first.date_id
             GROUP BY year ORDER BY year"
        )))
        .fetch_all(self.db().reader())
        .await?)
    }
}

fn pattern(query: &str) -> String {
    match query.strip_suffix('*') {
        Some(prefix) => format!("{}%", escape_like(prefix)),
        None => contains(query),
    }
}

fn lookup(counts: &[(i32, i64)], year: i32) -> i64 {
    counts
        .iter()
        .find(|(at, _)| *at == year)
        .map_or(0, |(_, count)| *count)
}

fn ratio(numerator: i64, denominator: i64) -> f64 {
    if denominator == 0 {
        return 0.0;
    }
    numerator as f64 / denominator as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trailing_star_asks_for_a_prefix() {
        assert_eq!(pattern("neb*"), "neb%");
        assert_eq!(pattern("neb"), "%neb%");
        assert_eq!(pattern("50%*"), "50\\%%");
    }

    #[test]
    fn a_ratio_of_nothing_is_zero_rather_than_a_division_by_zero() {
        assert_eq!(ratio(0, 0), 0.0);
        assert_eq!(ratio(10, 4), 2.5);
    }
}
