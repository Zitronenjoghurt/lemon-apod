use apod_core::ApodDate;
use apod_core::db::{Db, DbConfig, DbResult};
use chrono::{DateTime, Utc};
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{AssertSqlSafe, Row};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

const COLUMNS: &str = "guild_id, channel_id, message, explanation, enabled, last_date_id";
const USER_COLUMNS: &str = "user_id, explanation, enabled, last_date_id";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, poise::ChoiceParameter)]
pub enum Explanation {
    #[default]
    #[name = "Full explanation"]
    Full,
    #[name = "Short teaser"]
    Teaser,
    #[name = "No explanation"]
    None,
}

impl fmt::Display for Explanation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Full => "full",
            Self::Teaser => "teaser",
            Self::None => "none",
        })
    }
}

impl FromStr for Explanation {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "full" => Self::Full,
            "teaser" => Self::Teaser,
            "none" => Self::None,
            _ => return Err(()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guild {
    pub guild_id: u64,
    pub channel_id: Option<u64>,
    pub message: Option<String>,
    pub explanation: Explanation,
    pub enabled: bool,
    pub last_date_id: Option<i32>,
}

impl Guild {
    pub fn new(guild_id: u64) -> Self {
        Self {
            guild_id,
            channel_id: None,
            message: None,
            explanation: Explanation::default(),
            enabled: false,
            last_date_id: None,
        }
    }

    pub fn announces(&self) -> bool {
        self.enabled && self.channel_id.is_some()
    }

    fn read(row: &SqliteRow) -> Self {
        Self {
            guild_id: row.get::<i64, _>("guild_id") as u64,
            channel_id: row.get::<Option<i64>, _>("channel_id").map(|id| id as u64),
            message: row.get("message"),
            explanation: row
                .get::<String, _>("explanation")
                .parse()
                .unwrap_or_default(),
            enabled: row.get::<i64, _>("enabled") == 1,
            last_date_id: row
                .get::<Option<i64>, _>("last_date_id")
                .map(|id| id as i32),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscriber {
    pub user_id: u64,
    pub explanation: Explanation,
    pub enabled: bool,
    pub last_date_id: Option<i32>,
}

impl Subscriber {
    pub fn new(user_id: u64) -> Self {
        Self {
            user_id,
            explanation: Explanation::default(),
            enabled: false,
            last_date_id: None,
        }
    }

    fn read(row: &SqliteRow) -> Self {
        Self {
            user_id: row.get::<i64, _>("user_id") as u64,
            explanation: row
                .get::<String, _>("explanation")
                .parse()
                .unwrap_or_default(),
            enabled: row.get::<i64, _>("enabled") == 1,
            last_date_id: row
                .get::<Option<i64>, _>("last_date_id")
                .map(|id| id as i32),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BotStore {
    db: Db,
}

impl BotStore {
    pub async fn open(path: impl AsRef<Path>) -> DbResult<Self> {
        let db = Db::open(DbConfig::read_write(path.as_ref())).await?;
        db.migrate(&MIGRATIONS).await?;
        Ok(Self { db })
    }

    pub async fn guild(&self, guild_id: u64) -> DbResult<Guild> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM guilds WHERE guild_id = ?1"
        )))
        .bind(guild_id as i64)
        .fetch_optional(self.db.reader())
        .await?;

        Ok(row.map_or_else(|| Guild::new(guild_id), |row| Guild::read(&row)))
    }

    pub async fn save(&self, guild: &Guild, at: DateTime<Utc>) -> DbResult<()> {
        sqlx::query(
            "INSERT INTO guilds (guild_id, channel_id, message, explanation, enabled, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(guild_id) DO UPDATE SET
               channel_id = excluded.channel_id,
               message = excluded.message,
               explanation = excluded.explanation,
               enabled = excluded.enabled,
               updated_at = excluded.updated_at",
        )
        .bind(guild.guild_id as i64)
        .bind(guild.channel_id.map(|id| id as i64))
        .bind(guild.message.as_deref())
        .bind(guild.explanation.to_string())
        .bind(i64::from(guild.enabled))
        .bind(at.timestamp())
        .execute(self.db.writer()?)
        .await?;

        Ok(())
    }

    pub async fn owed(&self, date_id: i32) -> DbResult<Vec<Guild>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM guilds
             WHERE enabled = 1 AND channel_id IS NOT NULL
               AND (last_date_id IS NULL OR last_date_id < ?1)
             ORDER BY guild_id"
        )))
        .bind(date_id as i64)
        .fetch_all(self.db.reader())
        .await?;

        Ok(rows.iter().map(Guild::read).collect())
    }

    pub async fn mark(&self, guild_id: u64, date_id: i32, at: DateTime<Utc>) -> DbResult<bool> {
        let done = sqlx::query(
            "UPDATE guilds SET last_date_id = ?2, updated_at = ?3
             WHERE guild_id = ?1 AND (last_date_id IS NULL OR last_date_id < ?2)",
        )
        .bind(guild_id as i64)
        .bind(date_id as i64)
        .bind(at.timestamp())
        .execute(self.db.writer()?)
        .await?;

        Ok(done.rows_affected() == 1)
    }

    pub async fn forget_channel(&self, guild_id: u64, at: DateTime<Utc>) -> DbResult<()> {
        sqlx::query(
            "UPDATE guilds SET channel_id = NULL, enabled = 0, updated_at = ?2
             WHERE guild_id = ?1",
        )
        .bind(guild_id as i64)
        .bind(at.timestamp())
        .execute(self.db.writer()?)
        .await?;

        Ok(())
    }

    pub async fn subscriber(&self, user_id: u64) -> DbResult<Subscriber> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT {USER_COLUMNS} FROM users WHERE user_id = ?1"
        )))
        .bind(user_id as i64)
        .fetch_optional(self.db.reader())
        .await?;

        Ok(row.map_or_else(|| Subscriber::new(user_id), |row| Subscriber::read(&row)))
    }

    pub async fn save_subscriber(&self, user: &Subscriber, at: DateTime<Utc>) -> DbResult<()> {
        sqlx::query(
            "INSERT INTO users (user_id, explanation, enabled, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(user_id) DO UPDATE SET
               explanation = excluded.explanation,
               enabled = excluded.enabled,
               updated_at = excluded.updated_at",
        )
        .bind(user.user_id as i64)
        .bind(user.explanation.to_string())
        .bind(i64::from(user.enabled))
        .bind(at.timestamp())
        .execute(self.db.writer()?)
        .await?;

        Ok(())
    }

    pub async fn owed_subscribers(&self, date_id: i32) -> DbResult<Vec<Subscriber>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {USER_COLUMNS} FROM users
             WHERE enabled = 1 AND (last_date_id IS NULL OR last_date_id < ?1)
             ORDER BY user_id"
        )))
        .bind(date_id as i64)
        .fetch_all(self.db.reader())
        .await?;

        Ok(rows.iter().map(Subscriber::read).collect())
    }

    pub async fn mark_subscriber(
        &self,
        user_id: u64,
        date_id: i32,
        at: DateTime<Utc>,
    ) -> DbResult<bool> {
        let done = sqlx::query(
            "UPDATE users SET last_date_id = ?2, updated_at = ?3
             WHERE user_id = ?1 AND (last_date_id IS NULL OR last_date_id < ?2)",
        )
        .bind(user_id as i64)
        .bind(date_id as i64)
        .bind(at.timestamp())
        .execute(self.db.writer()?)
        .await?;

        Ok(done.rows_affected() == 1)
    }

    pub async fn unsubscribe(&self, user_id: u64, at: DateTime<Utc>) -> DbResult<()> {
        sqlx::query("UPDATE users SET enabled = 0, updated_at = ?2 WHERE user_id = ?1")
            .bind(user_id as i64)
            .bind(at.timestamp())
            .execute(self.db.writer()?)
            .await?;

        Ok(())
    }

    pub async fn subscribed(&self) -> DbResult<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE enabled = 1")
                .fetch_one(self.db.reader())
                .await?,
        )
    }

    pub async fn toggle_favorite(
        &self,
        user_id: u64,
        date: ApodDate,
        at: DateTime<Utc>,
    ) -> DbResult<bool> {
        let removed = sqlx::query("DELETE FROM favorites WHERE user_id = ?1 AND date_id = ?2")
            .bind(user_id as i64)
            .bind(date.days())
            .execute(self.db.writer()?)
            .await?;

        if removed.rows_affected() > 0 {
            return Ok(false);
        }

        sqlx::query("INSERT INTO favorites (user_id, date_id, created_at) VALUES (?1, ?2, ?3)")
            .bind(user_id as i64)
            .bind(date.days())
            .bind(at.timestamp())
            .execute(self.db.writer()?)
            .await?;

        Ok(true)
    }

    pub async fn favorite_count(&self, date: ApodDate) -> DbResult<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM favorites WHERE date_id = ?1")
                .bind(date.days())
                .fetch_one(self.db.reader())
                .await?,
        )
    }

    pub async fn favorites(
        &self,
        user_id: u64,
        offset: usize,
        limit: usize,
    ) -> DbResult<Vec<ApodDate>> {
        let rows: Vec<i64> = sqlx::query_scalar(
            "SELECT date_id FROM favorites WHERE user_id = ?1
             ORDER BY date_id DESC LIMIT ?2 OFFSET ?3",
        )
        .bind(user_id as i64)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(self.db.reader())
        .await?;

        Ok(rows
            .into_iter()
            .map(|days| ApodDate::from_days(days as i32))
            .collect())
    }

    pub async fn favorites_total(&self, user_id: u64) -> DbResult<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM favorites WHERE user_id = ?1")
                .bind(user_id as i64)
                .fetch_one(self.db.reader())
                .await?,
        )
    }

    pub async fn forget_favorites(&self, user_id: u64) -> DbResult<u64> {
        let gone = sqlx::query("DELETE FROM favorites WHERE user_id = ?1")
            .bind(user_id as i64)
            .execute(self.db.writer()?)
            .await?;

        Ok(gone.rows_affected())
    }

    pub async fn announcing(&self) -> DbResult<i64> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM guilds WHERE enabled = 1 AND channel_id IS NOT NULL",
        )
        .fetch_one(self.db.reader())
        .await?)
    }

    pub async fn close(&self) {
        self.db.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    async fn store() -> BotStore {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "apod-bot-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        BotStore::open(dir.join("bot.db")).await.unwrap()
    }

    fn configured(guild_id: u64) -> Guild {
        Guild {
            guild_id,
            channel_id: Some(999),
            message: Some("<@&42> new picture".to_owned()),
            explanation: Explanation::Teaser,
            enabled: true,
            last_date_id: None,
        }
    }

    #[tokio::test]
    async fn a_guild_that_has_never_used_the_bot_reads_as_the_defaults_and_writes_nothing() {
        let store = store().await;

        let guild = store.guild(7).await.unwrap();
        assert_eq!(guild, Guild::new(7));
        assert!(!guild.announces());
        assert_eq!(store.announcing().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn settings_survive_a_round_trip_including_the_message_and_the_explanation_choice() {
        let store = store().await;
        store.save(&configured(7), Utc::now()).await.unwrap();

        assert_eq!(store.guild(7).await.unwrap(), configured(7));
    }

    #[tokio::test]
    async fn only_a_guild_that_is_on_and_has_a_channel_is_owed_an_announcement() {
        let store = store().await;
        let now = Utc::now();

        store.save(&configured(1), now).await.unwrap();

        let mut off = configured(2);
        off.enabled = false;
        store.save(&off, now).await.unwrap();

        let mut homeless = configured(3);
        homeless.channel_id = None;
        store.save(&homeless, now).await.unwrap();

        let owed = store.owed(11_000).await.unwrap();
        assert_eq!(owed.len(), 1, "{owed:?}");
        assert_eq!(owed[0].guild_id, 1);
        assert_eq!(store.announcing().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn a_guild_at_todays_watermark_is_not_owed_it_again_but_tomorrow_still_is() {
        let store = store().await;
        let now = Utc::now();
        store.save(&configured(1), now).await.unwrap();

        assert!(store.mark(1, 11_000, now).await.unwrap());
        assert!(store.owed(11_000).await.unwrap().is_empty());
        assert_eq!(store.owed(11_001).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn the_watermark_only_ever_moves_forward() {
        let store = store().await;
        let now = Utc::now();
        store.save(&configured(1), now).await.unwrap();

        assert!(store.mark(1, 11_000, now).await.unwrap());
        assert!(
            !store.mark(1, 11_000, now).await.unwrap(),
            "the same day again is not a move"
        );
        assert!(
            !store.mark(1, 10_000, now).await.unwrap(),
            "an older entry must not walk it back and replay everything since"
        );
        assert_eq!(store.guild(1).await.unwrap().last_date_id, Some(11_000));
    }

    #[tokio::test]
    async fn changing_the_settings_does_not_disturb_where_the_guild_has_got_to() {
        let store = store().await;
        let now = Utc::now();
        store.save(&configured(1), now).await.unwrap();
        store.mark(1, 11_000, now).await.unwrap();

        let mut changed = store.guild(1).await.unwrap();
        changed.explanation = Explanation::None;
        store.save(&changed, now).await.unwrap();

        let guild = store.guild(1).await.unwrap();
        assert_eq!(guild.explanation, Explanation::None);
        assert_eq!(
            guild.last_date_id,
            Some(11_000),
            "editing the settings must not replay today"
        );
    }

    #[tokio::test]
    async fn a_channel_that_is_gone_is_forgotten_without_losing_the_rest_of_the_setup() {
        let store = store().await;
        let now = Utc::now();
        store.save(&configured(1), now).await.unwrap();
        store.mark(1, 11_000, now).await.unwrap();

        store.forget_channel(1, now).await.unwrap();

        let guild = store.guild(1).await.unwrap();
        assert_eq!(guild.channel_id, None);
        assert!(!guild.enabled);
        assert_eq!(
            guild.message.as_deref(),
            Some("<@&42> new picture"),
            "the mention is what took effort to write, so it stays"
        );
        assert_eq!(guild.explanation, Explanation::Teaser);
        assert_eq!(
            guild.last_date_id,
            Some(11_000),
            "pointing it at a new channel must not replay the day it already posted"
        );
    }

    #[tokio::test]
    async fn a_person_who_never_asked_for_anything_reads_as_the_defaults() {
        let store = store().await;

        assert_eq!(store.subscriber(5).await.unwrap(), Subscriber::new(5));
        assert_eq!(store.subscribed().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn a_subscription_round_trips_and_only_the_enabled_are_owed() {
        let store = store().await;
        let now = Utc::now();

        let wanted = Subscriber {
            user_id: 5,
            explanation: Explanation::Teaser,
            enabled: true,
            last_date_id: None,
        };
        store.save_subscriber(&wanted, now).await.unwrap();
        store
            .save_subscriber(&Subscriber::new(6), now)
            .await
            .unwrap();

        assert_eq!(store.subscriber(5).await.unwrap(), wanted);
        assert_eq!(store.subscribed().await.unwrap(), 1);

        let owed = store.owed_subscribers(11_000).await.unwrap();
        assert_eq!(owed.len(), 1, "{owed:?}");
        assert_eq!(owed[0].user_id, 5);
    }

    #[tokio::test]
    async fn a_persons_watermark_moves_forward_only_and_stops_them_being_owed_again() {
        let store = store().await;
        let now = Utc::now();
        store
            .save_subscriber(
                &Subscriber {
                    user_id: 5,
                    explanation: Explanation::Full,
                    enabled: true,
                    last_date_id: None,
                },
                now,
            )
            .await
            .unwrap();

        assert!(store.mark_subscriber(5, 11_000, now).await.unwrap());
        assert!(!store.mark_subscriber(5, 11_000, now).await.unwrap());
        assert!(!store.mark_subscriber(5, 10_000, now).await.unwrap());
        assert!(store.owed_subscribers(11_000).await.unwrap().is_empty());
        assert_eq!(store.owed_subscribers(11_001).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn somebody_discord_will_not_let_us_reach_is_dropped_without_losing_their_choice() {
        let store = store().await;
        let now = Utc::now();
        store
            .save_subscriber(
                &Subscriber {
                    user_id: 5,
                    explanation: Explanation::Teaser,
                    enabled: true,
                    last_date_id: None,
                },
                now,
            )
            .await
            .unwrap();
        store.mark_subscriber(5, 11_000, now).await.unwrap();

        store.unsubscribe(5, now).await.unwrap();

        let user = store.subscriber(5).await.unwrap();
        assert!(!user.enabled);
        assert_eq!(user.explanation, Explanation::Teaser);
        assert_eq!(user.last_date_id, Some(11_000));
        assert!(store.owed_subscribers(11_000).await.unwrap().is_empty());
    }

    fn day(year: i32, month: u32, day: u32) -> ApodDate {
        ApodDate::from_ymd(year, month, day).unwrap()
    }

    #[tokio::test]
    async fn a_press_saves_and_a_second_press_takes_it_back() {
        let store = store().await;
        let now = Utc::now();
        let date = day(2026, 9, 1);

        assert_eq!(store.favorite_count(date).await.unwrap(), 0);

        assert!(store.toggle_favorite(5, date, now).await.unwrap());
        assert_eq!(store.favorite_count(date).await.unwrap(), 1);

        assert!(!store.toggle_favorite(5, date, now).await.unwrap());
        assert_eq!(store.favorite_count(date).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn the_count_on_a_card_is_everybody_while_the_list_is_only_yours() {
        let store = store().await;
        let now = Utc::now();
        let date = day(2026, 9, 1);

        for user in [5, 6, 7] {
            store.toggle_favorite(user, date, now).await.unwrap();
        }
        store
            .toggle_favorite(5, day(2026, 8, 30), now)
            .await
            .unwrap();

        assert_eq!(
            store.favorite_count(date).await.unwrap(),
            3,
            "the card shows how many people saved it, not whether you did"
        );
        assert_eq!(store.favorites_total(5).await.unwrap(), 2);
        assert_eq!(store.favorites_total(6).await.unwrap(), 1);
        assert_eq!(store.favorites_total(8).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn a_list_reads_newest_first_and_pages_without_repeating_or_skipping() {
        let store = store().await;
        let now = Utc::now();
        let saved_out_of_date_order = [
            day(2026, 9, 1),
            day(2026, 8, 30),
            day(1995, 6, 16),
            day(2001, 1, 1),
        ];

        for date in saved_out_of_date_order {
            store.toggle_favorite(5, date, now).await.unwrap();
        }

        assert_eq!(
            store.favorites(5, 0, 2).await.unwrap(),
            vec![day(2026, 9, 1), day(2026, 8, 30)]
        );
        assert_eq!(
            store.favorites(5, 2, 2).await.unwrap(),
            vec![day(2001, 1, 1), day(1995, 6, 16)]
        );
        assert!(store.favorites(5, 4, 2).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn clearing_favorites_actually_removes_them_and_leaves_other_people_alone() {
        let store = store().await;
        let now = Utc::now();
        let date = day(2026, 9, 1);

        store.toggle_favorite(5, date, now).await.unwrap();
        store
            .toggle_favorite(5, day(2026, 8, 30), now)
            .await
            .unwrap();
        store.toggle_favorite(6, date, now).await.unwrap();

        assert_eq!(store.forget_favorites(5).await.unwrap(), 2);
        assert_eq!(store.favorites_total(5).await.unwrap(), 0);
        assert_eq!(
            store.favorite_count(date).await.unwrap(),
            1,
            "somebody else asking for nothing must not take yours with it"
        );
        assert_eq!(
            store.forget_favorites(5).await.unwrap(),
            0,
            "asking twice is not an error"
        );
    }

    #[tokio::test]
    async fn favorites_and_subscriptions_do_not_touch_each_other() {
        let store = store().await;
        let now = Utc::now();

        store
            .toggle_favorite(5, day(2026, 9, 1), now)
            .await
            .unwrap();
        assert_eq!(
            store.subscriber(5).await.unwrap(),
            Subscriber::new(5),
            "pressing a button must not sign anybody up for a daily DM"
        );

        store.unsubscribe(5, now).await.unwrap();
        assert_eq!(
            store.favorites_total(5).await.unwrap(),
            1,
            "and turning the DMs off must not throw away what they saved"
        );
    }

    #[test]
    fn every_explanation_choice_survives_the_trip_through_storage() {
        for choice in [Explanation::Full, Explanation::Teaser, Explanation::None] {
            assert_eq!(choice.to_string().parse(), Ok(choice));
        }
    }
}
