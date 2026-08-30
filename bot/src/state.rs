use crate::config::Config;
use crate::store::BotStore;
use anyhow::{Context, Result};
use apod_core::ApodReader;
use apod_core::apod::Snippet;
use apod_core::db::DbConfig;
use std::sync::Arc;

#[derive(Clone)]
pub struct BotState {
    pub config: Arc<Config>,
    pub apod: ApodReader,
    pub store: BotStore,
}

impl BotState {
    pub async fn initialize(config: Config) -> Result<Self> {
        let apod = ApodReader::open(DbConfig::read_only(&config.index_db))
            .await
            .with_context(|| format!("opening {}", config.index_db.display()))?
            .with_snippet(Snippet::Delimited {
                open: "**".to_owned(),
                close: "**".to_owned(),
            });

        let store = BotStore::open(&config.bot_db)
            .await
            .with_context(|| format!("opening {}", config.bot_db.display()))?;

        tracing::info!(
            archive = %config.index_db.display(),
            own = %config.bot_db.display(),
            announcing = store.announcing().await.unwrap_or(0),
            "state ready"
        );

        Ok(Self {
            config: Arc::new(config),
            apod,
            store,
        })
    }

    pub async fn close(&self) {
        self.store.close().await;
        self.apod.db().close().await;
    }
}
