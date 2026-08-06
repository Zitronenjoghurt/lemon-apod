mod api;
mod config;
mod meta;
mod shutdown;
mod state;
mod store;
mod web;

use anyhow::{Context, Result};
use axum::Router;
use axum::routing::get;
use config::Config;
use state::ServerState;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    let config = Config::from_env()?;
    let address = SocketAddr::from((config.bind, config.port));
    let state = ServerState::new(config)?;

    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .with_context(|| format!("binding {address}"))?;
    tracing::info!("listening on {address}");

    axum::serve(
        listener,
        router(&state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown::signal())
    .await
    .context("serving")?;

    Ok(())
}

fn router(state: &ServerState) -> Router {
    let governor = GovernorConfigBuilder::default()
        .per_second(state.config.rate_limit_per_second)
        .burst_size(state.config.rate_limit_burst)
        .key_extractor(SmartIpKeyExtractor)
        .use_headers()
        .finish()
        .expect("rate limit configuration is valid");

    let static_files = ServeDir::new(&state.config.static_dir)
        .append_index_html_on_directories(false)
        .fallback(get(web::spa).with_state(state.clone()));

    Router::new()
        .nest(
            "/api",
            api::build().layer(GovernorLayer::new(Arc::new(governor))),
        )
        .merge(web::build(state))
        .fallback_service(static_files)
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone())
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
}
