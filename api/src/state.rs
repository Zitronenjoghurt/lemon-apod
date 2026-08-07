use crate::config::Config;
use crate::meta::Shell;
use anyhow::Result;
use apod_core::db::DbConfig;
use apod_core::{ApodReader, Snippet};
use std::sync::Arc;

const DB_READERS: u32 = 8;

#[derive(Clone)]
pub struct ServerState {
    pub config: Arc<Config>,
    pub store: ApodReader,
    pub shell: Arc<Shell>,
}

impl ServerState {
    pub async fn new(config: Config) -> Result<Self> {
        let store =
            ApodReader::open(DbConfig::read_only(&config.index_db).with_readers(DB_READERS))
                .await?
                .with_thumb_base("/thumbs/")
                .with_snippet(Snippet::Html);

        Ok(Self {
            shell: Arc::new(Shell::load(&config)?),
            store,
            config: Arc::new(config),
        })
    }
}
