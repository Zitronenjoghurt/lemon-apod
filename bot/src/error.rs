use crate::Context;
use crate::state::BotState;
use apod_core::ApodDate;
use poise::serenity_prelude::{Colour, CreateEmbed};
use poise::{CreateReply, FrameworkError};
use std::time::Duration;
use tracing::{debug, error, warn};

pub type BotResult<T> = Result<T, BotError>;

const RED: Colour = Colour::new(0xB8_5C_54);

#[derive(Debug, thiserror::Error)]
pub enum BotError {
    #[error("{0}")]
    Apod(#[from] apod_core::ApodError),
    #[error("{0}")]
    Db(#[from] apod_core::DbError),
    #[error("This command only works inside a server.")]
    GuildOnly,
    #[error("No channel is set. Run `/apod settings channel:#somewhere` first.")]
    NoChannel,
    #[error("The archive has no entry for {0}.")]
    NoEntry(ApodDate),
    #[error("'{0}' is not a date. Write it as YYYY-MM-DD, any day from 1995-06-16 onwards.")]
    NotADate(String),
    #[error("No matches found.")]
    NothingFound,
    #[error("{0}")]
    Serenity(#[from] poise::serenity_prelude::Error),
}

impl BotError {
    pub fn is_user_error(&self) -> bool {
        match self {
            Self::GuildOnly
            | Self::NoChannel
            | Self::NoEntry(_)
            | Self::NotADate(_)
            | Self::NothingFound => true,
            Self::Apod(_) | Self::Db(_) | Self::Serenity(_) => false,
        }
    }
}

pub fn notice(title: &str, body: impl Into<String>) -> CreateEmbed {
    CreateEmbed::new()
        .colour(RED)
        .title(title)
        .description(body)
}

async fn tell(ctx: &Context<'_>, title: &str, body: impl Into<String>) {
    let reply = CreateReply::default()
        .embed(notice(title, body))
        .ephemeral(true);

    if let Err(error) = ctx.send(reply).await {
        warn!("could not answer '{}': {error}", ctx.command().name);
    }
}

pub async fn handler(error: FrameworkError<'_, BotState, BotError>) {
    match error {
        // The command's own body failed.
        FrameworkError::Command { error, ctx, .. } => {
            if error.is_user_error() {
                tell(&ctx, "That did not work", error.to_string()).await;
                return;
            }

            error!(
                "[#{}] '{}' run by '{}' failed: {error}",
                ctx.id(),
                ctx.command().name,
                ctx.author().id
            );
            tell(&ctx, "Something broke", unexpected(ctx.id())).await;
        }
        FrameworkError::CommandPanic { payload, ctx, .. } => {
            error!(
                payload = payload,
                "CRITICAL [#{}] a panic in '{}' run by '{}'",
                ctx.id(),
                ctx.command().name,
                ctx.author().id
            );
            tell(&ctx, "Something broke", unexpected(ctx.id())).await;
        }

        FrameworkError::NotAnOwner { ctx, .. } => {
            debug!(
                "'{}' turned away from '{}'",
                ctx.author().id,
                ctx.command().name
            );
        }
        FrameworkError::MissingUserPermissions { ctx, .. } => {
            tell(
                &ctx,
                "Not yours to change",
                "Announcement settings are for people who can manage this server.",
            )
            .await;
        }
        FrameworkError::CommandCheckFailed { error, ctx, .. } => {
            let body = error.map_or_else(
                || "You cannot run that here.".to_owned(),
                |error| error.to_string(),
            );
            tell(&ctx, "That did not work", body).await;
        }
        FrameworkError::CooldownHit {
            remaining_cooldown,
            ctx,
            ..
        } => {
            tell(
                &ctx,
                "Not so fast",
                format!("Try that again in {}.", seconds(remaining_cooldown)),
            )
            .await;
        }
        FrameworkError::GuildOnly { ctx, .. } => {
            tell(&ctx, "Servers only", BotError::GuildOnly.to_string()).await;
        }
        FrameworkError::DmOnly { ctx, .. } => {
            tell(
                &ctx,
                "Not here",
                "That one only works in a direct message with me.",
            )
            .await;
        }
        FrameworkError::NsfwOnly { ctx, .. } => {
            tell(
                &ctx,
                "Not here",
                "That one only works in an age-gated channel.",
            )
            .await;
        }
        FrameworkError::ArgumentParse { input, ctx, .. } => {
            let body = match input {
                Some(input) => format!("I could not read `{input}`."),
                None => "I could not read that.".to_owned(),
            };
            tell(&ctx, "That did not work", body).await;
        }
        FrameworkError::SubcommandRequired { ctx } => {
            tell(
                &ctx,
                "Pick one",
                format!(
                    "`{}` needs one of: {}.",
                    ctx.command().name,
                    ctx.command()
                        .subcommands
                        .iter()
                        .map(|command| format!("`{}`", command.name))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
            .await;
        }

        FrameworkError::MissingBotPermissions {
            missing_permissions,
            ctx,
            ..
        } => {
            warn!(
                guild = ?ctx.guild_id(),
                "missing {missing_permissions} for '{}'",
                ctx.command().name
            );
            tell(
                &ctx,
                "I am missing a permission",
                format!("I need `{missing_permissions}` in this server to do that."),
            )
            .await;
        }

        FrameworkError::UnknownInteraction { interaction, .. } => {
            debug!(
                "Discord offered '{}', which this build does not have. Run `@bot sync`.",
                interaction.data.name
            );
        }
        FrameworkError::UnknownCommand { .. } | FrameworkError::NonCommandMessage { .. } => {}

        FrameworkError::CommandStructureMismatch { description, .. } => {
            warn!(
                "what Discord has registered no longer matches this build ({description}). Run `@bot sync` to push the current commands."
            );
        }
        FrameworkError::Setup { error, .. } => error!("the bot could not start: {error}"),
        FrameworkError::EventHandler { error, event, .. } => {
            error!("handling '{}': {error}", event.snake_case_name())
        }
        other => error!("unhandled framework error: {other}"),
    }
}

fn seconds(remaining: Duration) -> String {
    let seconds = remaining.as_secs().max(1);
    match seconds {
        1 => "1 second".to_owned(),
        seconds => format!("{seconds} seconds"),
    }
}

fn unexpected(id: u64) -> String {
    format!("Something went wrong on my side. If you report it, quote `#{id}`.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wait_is_phrased_as_a_sentence_and_never_rounds_down_to_nothing() {
        assert_eq!(seconds(Duration::from_millis(200)), "1 second");
        assert_eq!(seconds(Duration::from_secs(1)), "1 second");
        assert_eq!(seconds(Duration::from_secs(42)), "42 seconds");
    }

    #[test]
    fn what_the_person_can_fix_is_told_to_them_and_the_rest_is_logged() {
        assert!(BotError::NoChannel.is_user_error());
        assert!(BotError::NotADate("yesterday".into()).is_user_error());
        assert!(BotError::NothingFound.is_user_error());
        assert!(!BotError::Db(apod_core::DbError::ReadOnly { path: "x".into() }).is_user_error());
    }
}
