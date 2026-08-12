use super::model::{Cloze, ClozePiece, PictureSummary};
use super::read::{to_dates, ApodReader, ApodResult};
use crate::date::ApodDate;
use crate::entry::ApodSummary;
use crate::text;
use sqlx::AssertSqlSafe;
use std::collections::HashSet;

const PICTURE_KINDS: &str = "'image_jpg', 'image_png', 'image_gif'";

pub const WORDS_MIN: i64 = 90;
pub const WORDS_MAX: i64 = 200;
pub const GIVEN_SHARE: f64 = 0.4;

impl ApodReader {
    pub async fn picture_pool(&self, before: Option<ApodDate>) -> ApodResult<Vec<ApodDate>> {
        let days: Vec<i64> = sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT MIN(date_id) FROM entries
             WHERE thumb_path IS NOT NULL AND media_kind IN ({PICTURE_KINDS})
               AND (?1 IS NULL OR date_id < ?1)
             GROUP BY COALESCE(picture_group, date_id)
             ORDER BY 1"
        )))
        .bind(before.map(ApodDate::days))
        .fetch_all(self.db().reader())
        .await?;

        Ok(to_dates(days))
    }

    pub async fn text_pool(&self, before: Option<ApodDate>) -> ApodResult<Vec<ApodDate>> {
        let days: Vec<i64> = sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT entries.date_id FROM entries
             JOIN entry_stats ON entry_stats.date_id = entries.date_id
             WHERE entry_stats.word_count BETWEEN ?1 AND ?2
               AND entries.thumb_path IS NOT NULL
               AND entries.media_kind IN ({PICTURE_KINDS})
               AND (?3 IS NULL OR entries.date_id < ?3)
             ORDER BY entries.date_id"
        )))
        .bind(WORDS_MIN)
        .bind(WORDS_MAX)
        .bind(before.map(ApodDate::days))
        .fetch_all(self.db().reader())
        .await?;

        Ok(to_dates(days))
    }

    pub async fn summaries(&self, dates: &[ApodDate]) -> ApodResult<Vec<ApodSummary>> {
        let mut out = Vec::with_capacity(dates.len());

        for &date in dates {
            let row = sqlx::query(AssertSqlSafe(format!(
                "SELECT {} FROM entries WHERE date_id = ?1",
                super::SUMMARY_COLUMNS
            )))
            .bind(date.days())
            .fetch_optional(self.db().reader())
            .await?;

            if let Some(row) = row {
                out.push(self.summary(&row)?);
            }
        }

        Ok(out)
    }

    pub async fn picture_dates(&self, date: ApodDate) -> ApodResult<Vec<ApodDate>> {
        let days: Vec<i64> = sqlx::query_scalar(
            "SELECT date_id FROM entries
             WHERE picture_group IS NOT NULL
               AND picture_group = (SELECT picture_group FROM entries WHERE date_id = ?1)
             ORDER BY date_id",
        )
        .bind(date.days())
        .fetch_all(self.db().reader())
        .await?;

        Ok(if days.is_empty() {
            vec![date]
        } else {
            to_dates(days)
        })
    }

    pub async fn picture_summary(&self) -> ApodResult<PictureSummary> {
        let (pictures, entries): (i64, i64) = sqlx::query_as(
            "SELECT COUNT(DISTINCT picture_group), COUNT(*) FROM entries
             WHERE picture_group IS NOT NULL",
        )
        .fetch_one(self.db().reader())
        .await?;

        let most: Option<(i64, i64)> = sqlx::query_as(
            "SELECT picture_group, COUNT(*) FROM entries WHERE picture_group IS NOT NULL
             GROUP BY picture_group ORDER BY COUNT(*) DESC, picture_group LIMIT 1",
        )
        .fetch_optional(self.db().reader())
        .await?;

        Ok(PictureSummary {
            hashed: sqlx::query_scalar("SELECT COUNT(phash) FROM entries")
                .fetch_one(self.db().reader())
                .await?,
            pictures,
            entries,
            most_shown: most.map(|(days, _)| ApodDate::from_days(days as i32)),
            most_shown_times: most.map_or(0, |(_, count)| count),
        })
    }

    pub async fn word_reach(&self, word: &str) -> ApodResult<Option<i64>> {
        Ok(
            sqlx::query_scalar("SELECT entries FROM words WHERE word = ?1")
                .bind(word)
                .fetch_optional(self.db().reader())
                .await?,
        )
    }

    pub async fn given_words(&self) -> ApodResult<HashSet<String>> {
        let entries: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entry_stats")
            .fetch_one(self.db().reader())
            .await?;

        let words: Vec<String> = sqlx::query_scalar(
            "SELECT word FROM words WHERE entries > ?1 ORDER BY entries DESC LIMIT 200",
        )
        .bind((entries as f64 * GIVEN_SHARE) as i64)
        .fetch_all(self.db().reader())
        .await?;

        Ok(words.into_iter().collect())
    }
}

#[derive(Debug, Clone)]
pub struct Deal(u64);

impl Deal {
    pub fn daily(game: &str, day: ApodDate) -> Self {
        let mut seed = hash64(game.as_bytes());
        seed = mix(seed ^ hash64(&day.days().to_be_bytes()));
        Self(mix(seed ^ 0x5eed_a90d_1995_0616))
    }

    pub fn from_seed(seed: u64) -> Self {
        Self(mix(seed))
    }

    pub fn loose() -> Self {
        static ROUNDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos() as u64);
        let count = ROUNDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        Self::from_seed(now ^ mix(count))
    }

    pub fn seed(&self) -> u64 {
        self.0
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix(self.0)
    }

    pub fn below(&mut self, limit: usize) -> Option<usize> {
        (limit > 0).then(|| (self.next() % limit as u64) as usize)
    }

    pub fn take<T: Copy>(&mut self, pool: &[T], count: usize) -> Vec<T> {
        let count = count.min(pool.len());
        let mut drawn = HashSet::with_capacity(count);
        let mut out = Vec::with_capacity(count);

        while out.len() < count {
            let Some(index) = self.below(pool.len()) else {
                break;
            };
            if drawn.insert(index) {
                out.push(pool[index]);
            }
        }

        out
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for index in (1..items.len()).rev() {
            let other = (self.next() % (index as u64 + 1)) as usize;
            items.swap(index, other);
        }
    }
}

fn mix(value: u64) -> u64 {
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

pub fn hash64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub fn word_hash(salt: u64, word: &str) -> String {
    let mut bytes = salt.to_be_bytes().to_vec();
    bytes.extend_from_slice(word.as_bytes());
    format!("{:x}", hash64(&bytes))
}

pub fn cloze(title: &str, text: &str, given: &HashSet<String>, salt: u64) -> Cloze {
    let mut hidden = 0;
    let mut distinct = HashSet::new();

    let mut redact = |source: &str| -> Vec<ClozePiece> {
        text::tokens(source)
            .into_iter()
            .map(|token| match token {
                text::Token::Gap(gap) => ClozePiece::Shown { s: gap.to_owned() },
                text::Token::Word(word) => match text::normalise(word) {
                    None => ClozePiece::Shown { s: word.to_owned() },
                    Some(normalised) if given.contains(&normalised) => {
                        ClozePiece::Shown { s: word.to_owned() }
                    }
                    Some(normalised) => {
                        hidden += 1;
                        distinct.insert(normalised.clone());
                        ClozePiece::Hidden {
                            h: word_hash(salt, &normalised),
                            n: normalised.chars().count(),
                        }
                    }
                },
            })
            .collect()
    };

    let title_pieces = redact(title);
    let text_pieces = redact(text);

    Cloze {
        salt: format!("{salt:x}"),
        title: title_pieces,
        text: text_pieces,
        hidden,
        distinct: distinct.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn given(words: &[&str]) -> HashSet<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn a_day_deals_the_same_puzzle_every_time_and_a_different_one_per_game() {
        let day: ApodDate = "2026-08-08".parse().unwrap();
        let pool: Vec<i32> = (0..500).collect();

        let first = Deal::daily("date", day).take(&pool, 5);
        let again = Deal::daily("date", day).take(&pool, 5);
        assert_eq!(first, again, "the same day has to deal the same rounds");

        let other_game = Deal::daily("words", day).take(&pool, 5);
        let other_day = Deal::daily("date", day.next()).take(&pool, 5);
        assert_ne!(first, other_game);
        assert_ne!(first, other_day);
    }

    #[test]
    fn a_deal_never_hands_out_the_same_entry_twice() {
        let pool: Vec<i32> = (0..10).collect();
        let drawn = Deal::from_seed(7).take(&pool, 6);

        assert_eq!(drawn.len(), 6);
        assert_eq!(
            drawn.iter().collect::<HashSet<_>>().len(),
            6,
            "one entry in two rounds is one round wasted"
        );
    }

    #[test]
    fn asking_for_more_than_there_is_gives_what_there_is() {
        let pool = [1, 2, 3];
        assert_eq!(Deal::from_seed(1).take(&pool, 10).len(), 3);
        assert!(Deal::from_seed(1).take::<i32>(&[], 5).is_empty());
        assert_eq!(Deal::from_seed(1).below(0), None);
    }

    #[test]
    fn a_shuffle_keeps_every_item_and_is_reproducible() {
        let mut one = [1, 2, 3, 4, 5, 6, 7, 8];
        let mut two = one;
        Deal::from_seed(42).shuffle(&mut one);
        Deal::from_seed(42).shuffle(&mut two);

        assert_eq!(one, two);
        assert_ne!(one, [1, 2, 3, 4, 5, 6, 7, 8], "it did shuffle");
        assert_eq!(one.iter().sum::<i32>(), 36);
    }

    #[test]
    fn the_word_hash_is_a_fixed_value_the_frontend_can_reproduce() {
        assert_eq!(hash64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(hash64(b"nebula"), 0x9c5e_5908_3fbc_0ee6);
        assert_eq!(word_hash(0, "nebula"), "2da88849e7af2686");
        assert_eq!(word_hash(1, "nebula"), "9e82832964457621");
    }

    #[test]
    fn redaction_holds_back_the_words_and_keeps_everything_else() {
        let puzzle = cloze(
            "The Crab Nebula",
            "The crab is 6,500 light-years away. The nebula glows!",
            &given(&["the", "is", "away"]),
            0,
        );

        let shown: String = puzzle
            .text
            .iter()
            .map(|piece| match piece {
                ClozePiece::Shown { s } => s.clone(),
                ClozePiece::Hidden { n, .. } => "_".repeat(*n),
            })
            .collect();
        assert_eq!(
            shown, "The ____ is 6,500 ___________ away. The ______ _____!",
            "a bare number is not a word anybody could guess, so it stays"
        );

        assert_eq!(puzzle.hidden, 6, "four in the text and two in the title");
        assert_eq!(
            puzzle.distinct, 4,
            "the crab in the title is the crab in the text"
        );
    }

    #[test]
    fn the_same_word_hashes_the_same_however_it_was_written() {
        let puzzle = cloze(
            "Nebula",
            "A NEBULA and a nebula.",
            &given(&["a", "and"]),
            99,
        );
        let hashes: Vec<&String> = puzzle
            .text
            .iter()
            .chain(&puzzle.title)
            .filter_map(|piece| match piece {
                ClozePiece::Hidden { h, .. } => Some(h),
                ClozePiece::Shown { .. } => None,
            })
            .collect();

        assert_eq!(hashes.len(), 3);
        assert!(
            hashes.windows(2).all(|pair| pair[0] == pair[1]),
            "one guess has to light up all three"
        );
    }

    #[test]
    fn a_salt_changes_every_hash() {
        let words = given(&[]);
        let one = cloze("Nebula", "A nebula.", &words, 1);
        let two = cloze("Nebula", "A nebula.", &words, 2);
        assert_ne!(one.title, two.title);
    }
}
