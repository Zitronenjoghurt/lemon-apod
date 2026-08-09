use crate::db::{Db, DbConfig, DbResult};
use chrono::{DateTime, TimeZone, Utc};
use sqlx::Row;
use sqlx::migrate::Migrator;
use std::path::Path;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations-notify");

#[derive(Debug, Clone)]
pub struct NotifyStore {
    db: Db,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    pub topic: String,
    pub key: String,
    pub sent_at: DateTime<Utc>,
}

impl NotifyStore {
    pub async fn open(path: impl AsRef<Path>) -> DbResult<Self> {
        let db = Db::open(DbConfig::read_write(path.as_ref())).await?;
        db.migrate(&MIGRATIONS).await?;
        Ok(Self { db })
    }

    pub async fn is_sent(&self, topic: &str, key: &str) -> DbResult<bool> {
        let found: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM sent WHERE topic = ?1 AND key = ?2")
                .bind(topic)
                .bind(key)
                .fetch_optional(self.db.reader())
                .await?;

        Ok(found.is_some())
    }

    pub async fn mark(&self, topic: &str, key: &str, at: DateTime<Utc>) -> DbResult<bool> {
        let done = sqlx::query(
            "INSERT INTO sent (topic, key, sent_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(topic, key) DO NOTHING",
        )
        .bind(topic)
        .bind(key)
        .bind(at.timestamp())
        .execute(self.db.writer()?)
        .await?;

        Ok(done.rows_affected() == 1)
    }

    pub async fn count(&self) -> DbResult<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM sent")
            .fetch_one(self.db.reader())
            .await?)
    }

    pub async fn recent(&self, limit: i64) -> DbResult<Vec<Sent>> {
        let rows = sqlx::query(
            "SELECT topic, key, sent_at FROM sent ORDER BY sent_at DESC, topic, key LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(self.db.reader())
        .await?;

        Ok(rows
            .iter()
            .map(|row| Sent {
                topic: row.get("topic"),
                key: row.get("key"),
                sent_at: Utc
                    .timestamp_opt(row.get::<i64, _>("sent_at"), 0)
                    .single()
                    .unwrap_or_default(),
            })
            .collect())
    }

    pub async fn close(&self) {
        self.db.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn store() -> NotifyStore {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);

        let dir = std::env::temp_dir().join(format!(
            "apod-notify-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        NotifyStore::open(dir.join("notify.db")).await.unwrap()
    }

    #[tokio::test]
    async fn the_first_mark_claims_the_key_and_later_ones_do_not() {
        let store = store().await;

        assert!(
            store
                .mark("apod", "apod:2026-03-05", Utc::now())
                .await
                .unwrap()
        );
        assert!(
            !store
                .mark("apod", "apod:2026-03-05", Utc::now())
                .await
                .unwrap()
        );
        assert_eq!(store.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn the_same_key_under_a_different_topic_is_a_different_thing() {
        let store = store().await;

        assert!(store.mark("apod", "2026-03-05", Utc::now()).await.unwrap());
        assert!(store.mark("sky", "2026-03-05", Utc::now()).await.unwrap());
        assert!(store.is_sent("apod", "2026-03-05").await.unwrap());
        assert!(store.is_sent("sky", "2026-03-05").await.unwrap());
        assert!(!store.is_sent("aurora", "2026-03-05").await.unwrap());
    }

    #[tokio::test]
    async fn an_unsent_key_is_not_reported_as_sent() {
        let store = store().await;
        assert!(!store.is_sent("apod", "never").await.unwrap());
    }

    #[tokio::test]
    async fn the_recent_list_is_newest_first() {
        let store = store().await;
        let now = Utc::now();

        store
            .mark("apod", "old", now - chrono::Duration::hours(2))
            .await
            .unwrap();
        store.mark("apod", "new", now).await.unwrap();

        let recent = store.recent(10).await.unwrap();
        assert_eq!(recent[0].key, "new");
        assert_eq!(recent[1].key, "old");
    }
}
