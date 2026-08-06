use crate::config::Config;
use crate::meta::Shell;
use crate::store::Store;
use anyhow::Result;
use std::sync::Arc;

#[derive(Clone)]
pub struct ServerState {
    pub config: Arc<Config>,
    pub store: Store,
    pub shell: Arc<Shell>,
}

impl ServerState {
    pub fn new(config: Config) -> Result<Self> {
        Ok(Self {
            store: Store::open(&config.index_db)?,
            shell: Arc::new(Shell::load(&config)?),
            config: Arc::new(config),
        })
    }
}
