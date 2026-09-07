use apod_core::db::{Db, DbConfig};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

const ANNOUNCING: &str = "SELECT COUNT(*) FROM guilds WHERE enabled = 1 AND channel_id IS NOT NULL";
const SUBSCRIBERS: &str = "SELECT COUNT(*) FROM users WHERE enabled = 1";
const FAVORITES: &str = "SELECT COUNT(*) FROM favorites";
const FAVORITE_ENTRIES: &str = "SELECT COUNT(DISTINCT date_id) FROM favorites";

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct BotNumbers {
    pub announcing: Option<i64>,
    pub subscribers: Option<i64>,
    pub favorites: Option<i64>,
    pub favorite_entries: Option<i64>,
}

impl BotNumbers {
    fn nothing_to_show(&self) -> bool {
        self.announcing.is_none()
            && self.subscribers.is_none()
            && self.favorites.is_none()
            && self.favorite_entries.is_none()
    }
}

#[derive(Clone)]
pub struct Bot {
    path: Arc<PathBuf>,
    db: Arc<RwLock<Option<Db>>>,
}

impl Bot {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path: Arc::new(path),
            db: Arc::new(RwLock::new(None)),
        }
    }

    async fn db(&self) -> Option<Db> {
        if let Some(db) = self.db.read().await.as_ref() {
            return Some(db.clone());
        }

        if !tokio::fs::try_exists(&*self.path).await.unwrap_or(false) {
            return None;
        }

        let mut held = self.db.write().await;
        if let Some(db) = held.as_ref() {
            return Some(db.clone());
        }

        match Db::open(DbConfig::read_only(&*self.path)).await {
            Ok(db) => {
                tracing::info!(path = %self.path.display(), "opened the bot database");
                *held = Some(db.clone());
                Some(db)
            }
            Err(error) => {
                tracing::debug!(path = %self.path.display(), "no bot database yet: {error}");
                None
            }
        }
    }

    pub async fn numbers(&self) -> Option<BotNumbers> {
        let db = self.db().await?;

        let numbers = BotNumbers {
            announcing: shown(count(&db, ANNOUNCING).await),
            subscribers: shown(count(&db, SUBSCRIBERS).await),
            favorites: shown(count(&db, FAVORITES).await),
            favorite_entries: shown(count(&db, FAVORITE_ENTRIES).await),
        };

        match numbers.nothing_to_show() {
            true => None,
            false => Some(numbers),
        }
    }
}

async fn count(db: &Db, sql: &'static str) -> Option<i64> {
    sqlx::query_scalar(sql)
        .fetch_one(db.reader())
        .await
        .inspect_err(|error| tracing::warn!("counting from the bot database: {error}"))
        .ok()
}

fn shown(counted: Option<i64>) -> Option<i64> {
    counted.filter(|counted| *counted > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU32, Ordering};

    const GUILDS: &str = "CREATE TABLE guilds (
                            guild_id     INTEGER PRIMARY KEY,
                            channel_id   INTEGER,
                            message      TEXT,
                            explanation  TEXT    NOT NULL DEFAULT 'full',
                            enabled      INTEGER NOT NULL DEFAULT 0,
                            last_date_id INTEGER,
                            updated_at   INTEGER NOT NULL)";
    const USERS: &str = "CREATE TABLE users (
                           user_id      INTEGER PRIMARY KEY,
                           explanation  TEXT    NOT NULL DEFAULT 'full',
                           enabled      INTEGER NOT NULL DEFAULT 0,
                           last_date_id INTEGER,
                           updated_at   INTEGER NOT NULL)";
    const FAVORITES_TABLE: &str = "CREATE TABLE favorites (
                                     user_id    INTEGER NOT NULL,
                                     date_id    INTEGER NOT NULL,
                                     created_at INTEGER NOT NULL,
                                     PRIMARY KEY (user_id, date_id))";

    fn scratch() -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "apod-api-bot-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("bot.db")
    }

    async fn written(statements: &[&'static str]) -> Bot {
        let path = scratch();
        let db = Db::open(DbConfig::read_write(&path)).await.unwrap();
        for statement in statements {
            sqlx::query(*statement)
                .execute(db.writer().unwrap())
                .await
                .unwrap();
        }
        db.close().await;

        Bot::new(path)
    }

    #[tokio::test]
    async fn a_deployment_without_a_bot_publishes_nothing_and_survives_it() {
        let bot = Bot::new(scratch());

        assert!(bot.numbers().await.is_none());
        assert!(bot.numbers().await.is_none(), "and again, on the hot path");
    }

    #[tokio::test]
    async fn a_bot_that_never_ran_the_favorites_migration_still_reports_its_servers() {
        let bot = written(&[
            GUILDS,
            USERS,
            "INSERT INTO guilds (guild_id, channel_id, enabled, updated_at) VALUES (1, 10, 1, 0)",
            "INSERT INTO users (user_id, enabled, updated_at) VALUES (1, 1, 0)",
        ])
        .await;

        let numbers = bot.numbers().await.unwrap();

        assert_eq!(numbers.announcing, Some(1));
        assert_eq!(numbers.subscribers, Some(1));
        assert_eq!(
            numbers.favorites, None,
            "a table the bot has not migrated yet takes only its own figure down"
        );
        assert_eq!(numbers.favorite_entries, None);
    }

    #[tokio::test]
    async fn a_guild_that_switched_off_or_forgot_its_channel_is_not_announcing() {
        let bot = written(&[
            GUILDS,
            USERS,
            FAVORITES_TABLE,
            "INSERT INTO guilds (guild_id, channel_id, enabled, updated_at) VALUES (1, 10, 1, 0)",
            "INSERT INTO guilds (guild_id, channel_id, enabled, updated_at) VALUES (2, 20, 0, 0)",
            "INSERT INTO guilds (guild_id, channel_id, enabled, updated_at) VALUES (3, NULL, 1, 0)",
            "INSERT INTO users (user_id, enabled, updated_at) VALUES (1, 1, 0)",
            "INSERT INTO users (user_id, enabled, updated_at) VALUES (2, 0, 0)",
            "INSERT INTO favorites (user_id, date_id, created_at) VALUES (1, 100, 0)",
            "INSERT INTO favorites (user_id, date_id, created_at) VALUES (2, 100, 0)",
            "INSERT INTO favorites (user_id, date_id, created_at) VALUES (1, 200, 0)",
        ])
        .await;

        let numbers = bot.numbers().await.unwrap();

        assert_eq!(
            numbers.announcing,
            Some(1),
            "forgetting a channel and switching off both keep the row"
        );
        assert_eq!(
            numbers.subscribers,
            Some(1),
            "unsubscribing keeps the row too"
        );
        assert_eq!(numbers.favorites, Some(3));
        assert_eq!(numbers.favorite_entries, Some(2));
    }

    #[tokio::test]
    async fn a_bot_nobody_has_used_yet_publishes_nothing() {
        let bot = written(&[GUILDS, USERS, FAVORITES_TABLE]).await;

        assert!(
            bot.numbers().await.is_none(),
            "a page recruiting servers must not say it has none"
        );
    }

    #[tokio::test]
    async fn one_real_figure_is_enough_to_publish_the_block() {
        let bot = written(&[
            GUILDS,
            USERS,
            FAVORITES_TABLE,
            "INSERT INTO users (user_id, enabled, updated_at) VALUES (1, 1, 0)",
        ])
        .await;

        let numbers = bot.numbers().await.unwrap();

        assert_eq!(numbers.subscribers, Some(1));
        assert_eq!(numbers.announcing, None);
        assert_eq!(numbers.favorites, None);
    }
}
