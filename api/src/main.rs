mod api;
mod config;
mod meta;
mod probe;
mod rating;
mod schedule;
mod shutdown;
mod state;
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
    if probe::requested() {
        return probe::run().await;
    }

    init_logging();

    let config = Config::from_env()?;
    let address = SocketAddr::from((config.bind, config.port));
    let state = ServerState::new(config).await?;

    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .with_context(|| format!("binding {address}"))?;
    tracing::info!("listening on {address}");

    if let Some(rating) = state.rating.clone() {
        tokio::spawn(refit(state.clone(), rating));
    }

    axum::serve(
        listener,
        router(&state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown::signal())
    .await
    .context("serving")?;

    Ok(())
}
async fn refit(state: ServerState, rating: Arc<rating::Rating>) {
    let every = rating.settings.fit_every;

    loop {
        let before = schedule::Schedule::now(&state.config.publish)
            .today
            .parse()
            .ok();

        if let Err(error) = rating.refit(&state.store, before).await {
            tracing::error!("refitting the ratings: {error:#}");
        }
        if let Err(error) = rating.sweep(chrono::Utc::now()).await {
            tracing::error!("sweeping the voter table: {error:#}");
        }

        tokio::time::sleep(every).await;
    }
}

fn router(state: &ServerState) -> Router {
    let reading = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(state.config.rate_limit_per_second)
            .burst_size(state.config.rate_limit_burst)
            .key_extractor(SmartIpKeyExtractor)
            .use_headers()
            .finish()
            .expect("the reading rate limit is valid"),
    );
    let voting = Arc::new(
        GovernorConfigBuilder::default()
            .period(state.config.rating.vote_limit_period)
            .burst_size(state.config.rating.vote_limit_burst)
            .key_extractor(SmartIpKeyExtractor)
            .use_headers()
            .finish()
            .expect("the voting rate limit is valid"),
    );

    let static_files = ServeDir::new(&state.config.static_dir)
        .append_index_html_on_directories(false)
        .fallback(get(web::spa).with_state(state.clone()));

    let api = Router::new()
        .merge(api::read_routes().route_layer(GovernorLayer::new(reading.clone())))
        .merge(api::vote_routes().route_layer(GovernorLayer::new(voting)))
        .fallback(api::unknown_route);

    let router = Router::new()
        .nest("/api", api)
        .merge(web::metered().route_layer(GovernorLayer::new(reading)))
        .merge(web::unmetered(state))
        .fallback_service(static_files)
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    web::with_security_headers(router)
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
}
