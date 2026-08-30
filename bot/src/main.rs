use crate::error::BotError;
use crate::state::BotState;
use anyhow::{Context as _, Result};
use poise::serenity_prelude::{ClientBuilder, GatewayIntents, UserId};
use tracing::info;
use tracing_subscriber::EnvFilter;

mod announce;
mod card;
mod commands;
mod config;
mod error;
mod preview;
mod shutdown;
mod state;
mod store;

pub type Context<'a> = poise::Context<'a, BotState, BotError>;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = config::Config::from_env()?;

    if preview::requested() {
        return preview::run(config).await;
    }

    let token = std::env::var("DISCORD_TOKEN")
        .context("DISCORD_TOKEN is not set, and the bot has nothing to log in with")?;
    let announcing = config.announce.enabled;
    let state = BotState::initialize(config).await?;
    let closing = state.clone();

    let framework = poise::Framework::<BotState, BotError>::builder()
        .options(poise::FrameworkOptions {
            commands: commands::all(),
            owners: owners(&state),
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: None,
                mention_as_prefix: true,
                ..Default::default()
            },
            on_error: |error| Box::pin(error::handler(error)),
            ..Default::default()
        })
        .setup(move |ctx, ready, framework| {
            Box::pin(async move {
                info!(
                    bot = %ready.user.name,
                    guilds = ready.guilds.len(),
                    owners = ?framework.options().owners,
                    "connected"
                );

                if announcing {
                    tokio::spawn(announce::run(ctx.clone(), state.clone()));
                } else {
                    info!("announcements are off, so nothing will be posted");
                }

                Ok(state)
            })
        })
        .build();

    let intents =
        GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES | GatewayIntents::DIRECT_MESSAGES;

    let mut client = ClientBuilder::new(&token, intents)
        .framework(framework)
        .await?;

    let gateway = client.shard_manager.clone();
    tokio::spawn(async move {
        shutdown::signal().await;
        info!("shutting down, closing the gateway");
        gateway.shutdown_all().await;
    });

    client.start().await?;

    closing.close().await;
    info!("stopped");
    Ok(())
}

fn owners(state: &BotState) -> std::collections::HashSet<UserId> {
    state
        .config
        .owner_ids
        .iter()
        .map(|id| UserId::new(*id))
        .collect()
}
