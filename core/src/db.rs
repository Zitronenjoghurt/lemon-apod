use sqlx::migrate::Migrator;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_READERS: u32 = 4;
pub const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("creating {}", .path.display())]
    Directory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("opening {}", .path.display())]
    Open {
        path: PathBuf,
        #[source]
        source: sqlx::Error,
    },
    #[error("{} is open read-only", .path.display())]
    ReadOnly { path: PathBuf },
    #[error("migrating {}", .path.display())]
    Migrate {
        path: PathBuf,
        #[source]
        source: sqlx::migrate::MigrateError,
    },
    #[error(transparent)]
    Query(#[from] sqlx::Error),
}

pub type DbResult<T> = Result<T, DbError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone)]
pub struct DbConfig {
    pub path: PathBuf,
    pub access: Access,
    pub readers: u32,
    pub busy_timeout: Duration,
}

impl DbConfig {
    pub fn read_only(path: impl Into<PathBuf>) -> Self {
        Self::new(path, Access::ReadOnly)
    }

    pub fn read_write(path: impl Into<PathBuf>) -> Self {
        Self::new(path, Access::ReadWrite)
    }

    fn new(path: impl Into<PathBuf>, access: Access) -> Self {
        Self {
            path: path.into(),
            access,
            readers: DEFAULT_READERS,
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
        }
    }

    pub fn with_readers(mut self, readers: u32) -> Self {
        self.readers = readers.max(1);
        self
    }

    pub fn with_busy_timeout(mut self, busy_timeout: Duration) -> Self {
        self.busy_timeout = busy_timeout;
        self
    }
}

#[derive(Debug, Clone)]
pub struct Db {
    readers: SqlitePool,
    writer: Option<SqlitePool>,
    path: PathBuf,
}

impl Db {
    pub async fn open(config: DbConfig) -> DbResult<Self> {
        if config.access == Access::ReadWrite {
            create_parent(&config.path)?;
        }

        let writer = match config.access {
            Access::ReadOnly => None,
            Access::ReadWrite => Some(pool(&config, 1, write_options(&config)).await?),
        };

        let readers = pool(&config, config.readers, read_options(&config)).await?;

        Ok(Self {
            readers,
            writer,
            path: config.path,
        })
    }

    pub async fn migrate(&self, migrator: &Migrator) -> DbResult<()> {
        let writer = self.writer()?;
        migrator
            .run(writer)
            .await
            .map_err(|source| DbError::Migrate {
                path: self.path.clone(),
                source,
            })
    }

    pub async fn applied_version(&self) -> DbResult<Option<i64>> {
        let migrated: Option<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
        )
        .fetch_optional(&self.readers)
        .await?;

        if migrated.is_none() {
            return Ok(None);
        }

        Ok(
            sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1")
                .fetch_one(&self.readers)
                .await?,
        )
    }

    pub fn reader(&self) -> &SqlitePool {
        &self.readers
    }

    pub fn writer(&self) -> DbResult<&SqlitePool> {
        self.writer.as_ref().ok_or_else(|| DbError::ReadOnly {
            path: self.path.clone(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_writable(&self) -> bool {
        self.writer.is_some()
    }

    pub async fn close(&self) {
        if let Some(writer) = &self.writer {
            writer.close().await;
        }
        self.readers.close().await;
    }
}

fn write_options(config: &DbConfig) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(&config.path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(config.busy_timeout)
}

fn read_options(config: &DbConfig) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(&config.path)
        .read_only(true)
        .busy_timeout(config.busy_timeout)
}

async fn pool(
    config: &DbConfig,
    max_connections: u32,
    options: SqliteConnectOptions,
) -> DbResult<SqlitePool> {
    SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await
        .map_err(|source| DbError::Open {
            path: config.path.clone(),
            source,
        })
}

fn create_parent(path: &Path) -> DbResult<()> {
    let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return Ok(());
    };

    std::fs::create_dir_all(parent).map_err(|source| DbError::Directory {
        path: parent.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_path() -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "apod-db-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("nested").join("test.db")
    }

    async fn seeded(path: &Path) -> Db {
        let db = Db::open(DbConfig::read_write(path)).await.unwrap();
        sqlx::raw_sql("CREATE TABLE t (x INTEGER); INSERT INTO t VALUES (1), (2);")
            .execute(db.writer().unwrap())
            .await
            .unwrap();
        db
    }

    #[tokio::test]
    async fn creates_missing_parent_directories() {
        let path = temp_path();
        assert!(!path.parent().unwrap().exists());

        let db = Db::open(DbConfig::read_write(&path)).await.unwrap();
        assert!(path.exists());
        db.close().await;
    }

    #[tokio::test]
    async fn opens_in_wal_with_the_configured_pragmas() {
        let db = Db::open(DbConfig::read_write(temp_path())).await.unwrap();

        let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(db.writer().unwrap())
            .await
            .unwrap();
        assert_eq!(journal, "wal");

        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(db.writer().unwrap())
            .await
            .unwrap();
        assert_eq!(foreign_keys, 1, "cascading deletes have to be enforced");

        db.close().await;
    }

    #[tokio::test]
    async fn a_read_only_handle_can_read_but_not_write_or_migrate() {
        let path = temp_path();
        seeded(&path).await.close().await;

        let db = Db::open(DbConfig::read_only(&path)).await.unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM t")
            .fetch_one(db.reader())
            .await
            .unwrap();
        assert_eq!(count, 2);

        assert!(db.writer().is_err(), "there is no writer to hand out");
        assert!(!db.is_writable());

        let write = sqlx::query("INSERT INTO t VALUES (3)")
            .execute(db.reader())
            .await;
        assert!(write.is_err(), "the reader pool must reject writes");

        db.close().await;
    }

    #[tokio::test]
    async fn a_read_only_handle_opens_a_wal_database_with_no_sidecars() {
        let path = temp_path();
        let db = seeded(&path).await;
        sqlx::raw_sql("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(db.writer().unwrap())
            .await
            .unwrap();
        db.close().await;

        for suffix in ["-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }

        let db = Db::open(DbConfig::read_only(&path)).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM t")
            .fetch_one(db.reader())
            .await
            .unwrap();
        assert_eq!(count, 2);
        db.close().await;
    }

    #[tokio::test]
    async fn reports_no_version_before_any_migration_runs() {
        let db = Db::open(DbConfig::read_write(temp_path())).await.unwrap();
        assert_eq!(db.applied_version().await.unwrap(), None);
        db.close().await;
    }

    #[tokio::test]
    async fn opening_a_missing_file_read_only_is_an_error_not_an_empty_database() {
        let error = Db::open(DbConfig::read_only(temp_path()))
            .await
            .unwrap_err();
        assert!(
            matches!(error, DbError::Open { .. }),
            "expected an open failure, got {error:?}"
        );
    }
}
