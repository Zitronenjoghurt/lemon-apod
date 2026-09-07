use crate::api::error::ApiResult;
use crate::api::response;
use crate::bot::BotNumbers;
use crate::config::{Contact, Discord, Notify};
use crate::schedule::Schedule;
use crate::state::ServerState;
use apod_core::ApodSummary;
use axum::Router;
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct Status {
    latest: Option<ApodSummary>,
    entries: i64,
    publish: Schedule,
    rating: Rating,
    contact: Contact,
    notify: Notify,
    discord: DiscordBlock,
}

#[derive(Debug, Serialize)]
struct DiscordBlock {
    #[serde(flatten)]
    config: Discord,
    numbers: Option<BotNumbers>,
}

#[derive(Debug, Default, Serialize)]
struct Rating {
    enabled: bool,
    ready: bool,
    pool: u64,
    votes: u64,
    spent: usize,
}

async fn rating(state: &ServerState) -> ApiResult<Rating> {
    let Some(rating) = state.rating.as_deref() else {
        return Ok(Rating::default());
    };

    let mut votes = 0;
    for category in apod_core::rating::Category::ALL {
        votes += rating.store.tally(category).await?.votes;
    }

    Ok(Rating {
        enabled: true,
        ready: rating.ready().await,
        pool: rating.pool_size().await,
        spent: rating.spent_ballots(),
        votes,
    })
}

async fn get_status(State(state): State<ServerState>) -> ApiResult<Response> {
    let status = Status {
        latest: state.store.latest().await?.map(|entry| entry.to_summary()),
        entries: state.store.count().await?,
        publish: Schedule::now(&state.config.publish),
        rating: rating(&state).await?,
        contact: state.config.contact.clone(),
        notify: state.config.notify.clone(),
        discord: DiscordBlock {
            config: state.config.discord.clone(),
            numbers: state.bot.numbers().await,
        },
    };

    Ok(response::cached(state.config.cache_status_secs, status))
}

pub fn router() -> Router<ServerState> {
    Router::new().route("/", get(get_status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_discord_block_keeps_the_invite_urls_where_the_web_app_reads_them() {
        let block = DiscordBlock {
            config: Discord {
                invite_url: Some("https://discord.com/invite".to_owned()),
                user_install_url: Some("https://discord.com/install".to_owned()),
            },
            numbers: Some(BotNumbers {
                announcing: Some(2),
                subscribers: None,
                favorites: None,
                favorite_entries: None,
            }),
        };

        let json = serde_json::to_value(&block).unwrap();

        assert_eq!(json["invite_url"], "https://discord.com/invite");
        assert_eq!(json["user_install_url"], "https://discord.com/install");
        assert_eq!(json["numbers"]["announcing"], 2);
        assert!(json["numbers"]["subscribers"].is_null());
    }

    #[test]
    fn a_deployment_without_a_bot_still_offers_the_invite_urls() {
        let block = DiscordBlock {
            config: Discord {
                invite_url: Some("https://discord.com/invite".to_owned()),
                user_install_url: Some("https://discord.com/install".to_owned()),
            },
            numbers: None,
        };

        let json = serde_json::to_value(&block).unwrap();

        assert_eq!(json["invite_url"], "https://discord.com/invite");
        assert!(json["numbers"].is_null());
    }
}
