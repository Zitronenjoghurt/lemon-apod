use crate::Context;
use crate::error::{BotError, BotResult};
use crate::store::{Explanation, Guild};
use crate::{announce, card};
use apod_core::ApodDate;
use chrono::Utc;
use poise::CreateReply;
use poise::serenity_prelude as serenity;
use serenity::{Colour, CreateEmbed};

const GREY: Colour = Colour::new(0xA0_97_A1);
const GREEN: Colour = Colour::new(0xAC_B5_65);

/// Daily announcement settings.
#[poise::command(
    slash_command,
    guild_only,
    required_permissions = "MANAGE_GUILD",
    default_member_permissions = "MANAGE_GUILD"
)]
pub async fn settings(
    ctx: Context<'_>,
    #[description = "Enable daily announcements"] announce: Option<bool>,
    #[description = "Announcement channel"]
    #[channel_types("Text", "News")]
    channel: Option<serenity::Channel>,
    #[description = "A message included with each announcement"] message: Option<String>,
    #[description = "How much of the explanation to include"] explanation: Option<Explanation>,
) -> BotResult<()> {
    ctx.defer_ephemeral().await?;

    let state = ctx.data();
    let guild_id = ctx.guild_id().ok_or(BotError::GuildOnly)?.get();
    let mut guild = state.store.guild(guild_id).await?;

    let changing =
        announce.is_some() || channel.is_some() || message.is_some() || explanation.is_some();

    if let Some(announce) = announce {
        guild.enabled = announce;
    }
    if let Some(channel) = &channel {
        guild.channel_id = Some(channel.id().get());
    }
    if let Some(message) = &message {
        guild.message = wanted(message);
    }
    if let Some(explanation) = explanation {
        guild.explanation = explanation;
    }

    if changing {
        state.store.save(&guild, Utc::now()).await?;
        tracing::info!(guild = guild_id, "settings changed by {}", ctx.author().id);
    }

    ctx.send(
        CreateReply::default()
            .embed(summary(&guild, changing))
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// Force-send today's entry, to this server's channel or to your DMs if you run it there.
#[poise::command(
    slash_command,
    required_permissions = "MANAGE_GUILD",
    default_member_permissions = "MANAGE_GUILD"
)]
pub async fn announce(ctx: Context<'_>) -> BotResult<()> {
    ctx.defer_ephemeral().await?;

    let state = ctx.data();
    let entry = state
        .apod
        .latest()
        .await?
        .ok_or(BotError::NoEntry(ApodDate::START))?;

    let Some(guild_id) = ctx.guild_id().map(|id| id.get()) else {
        return to_dm(ctx, &entry).await;
    };

    let guild = state.store.guild(guild_id).await?;
    let channel_id = guild.channel_id.ok_or(BotError::NoChannel)?;

    let attachment = card::thumbnail(&state.config, &entry).await;
    let embed = card::embed(
        &state.config,
        &entry,
        guild.explanation,
        attachment.as_ref(),
    );

    announce::post(ctx.serenity_context(), &guild, embed, attachment).await?;

    let first = state
        .store
        .mark(guild_id, entry.date.days(), Utc::now())
        .await?;

    tracing::info!(
        guild = guild_id,
        date = %entry.date,
        repeat = !first,
        "announced by hand by {}",
        ctx.author().id
    );

    ctx.send(
        CreateReply::default()
            .embed(
                CreateEmbed::new()
                    .colour(GREEN)
                    .title("Posted")
                    .description(format!("{} is in <#{channel_id}> now.", entry.date)),
            )
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

async fn to_dm(ctx: Context<'_>, entry: &apod_core::ApodEntry) -> BotResult<()> {
    let state = ctx.data();
    let user_id = ctx.author().id.get();
    let user = state.store.subscriber(user_id).await?;

    let attachment = card::thumbnail(&state.config, entry).await;
    let embed = card::embed(&state.config, entry, user.explanation, attachment.as_ref());

    if let Err(error) = announce::dm(ctx.serenity_context(), user_id, embed, attachment).await {
        if announce::cannot_dm(&error) {
            ctx.send(
                CreateReply::default()
                    .embed(crate::error::notice(
                        "Discord will not let me DM you",
                        "Your privacy settings, or the fact that we share no server, stop me \
                         sending it. Join a server the bot is in, or allow direct messages from \
                         server members, then try again.",
                    ))
                    .ephemeral(true),
            )
            .await?;
            return Ok(());
        }
        return Err(error.into());
    }

    let first = state
        .store
        .mark_subscriber(user_id, entry.date.days(), Utc::now())
        .await?;

    tracing::info!(user = user_id, date = %entry.date, repeat = !first, "sent by hand");

    ctx.send(
        CreateReply::default()
            .embed(
                CreateEmbed::new()
                    .colour(GREEN)
                    .title("Sent")
                    .description(format!(
                        "{} is in your DMs now.{}",
                        entry.date,
                        match user.enabled {
                            true => "",
                            false =>
                                " You are not subscribed, so nothing else will follow unless \
                                      you run `/apod dm subscribe:true`.",
                        }
                    )),
            )
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

fn wanted(message: &str) -> Option<String> {
    let trimmed = message.trim();
    (!trimmed.is_empty() && trimmed != "-").then(|| trimmed.to_owned())
}

fn summary(guild: &Guild, changed: bool) -> CreateEmbed {
    let embed = CreateEmbed::new()
        .colour(match changed {
            true => GREEN,
            false => GREY,
        })
        .title(match changed {
            true => "Settings saved",
            false => "Settings",
        })
        .field("Announcements", state_of(guild), false)
        .field(
            "Channel",
            guild
                .channel_id
                .map_or_else(|| "`not set`".to_owned(), |id| format!("<#{id}>")),
            false,
        )
        .field(
            "Message",
            guild
                .message
                .clone()
                .unwrap_or_else(|| "`embed-only`".to_owned()),
            false,
        )
        .field("Explanation", format!("`{}`", guild.explanation), false);

    match note(guild) {
        Some(note) => embed.description(note),
        None => embed,
    }
}

fn state_of(guild: &Guild) -> &'static str {
    match (guild.enabled, guild.channel_id.is_some()) {
        (true, true) => "`on`",
        (true, false) => "`on, but no channel set`",
        (false, _) => "`off`",
    }
}

fn note(guild: &Guild) -> Option<&'static str> {
    match (guild.enabled, guild.channel_id.is_some()) {
        (true, false) => Some("Set a channel to activate announcements."),
        (true, true) => {
            Some("Each new Astronomy Picture of the Day will be posted once it is available.")
        }
        (false, _) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guild(enabled: bool, channel: Option<u64>) -> Guild {
        Guild {
            guild_id: 1,
            channel_id: channel,
            message: None,
            explanation: Explanation::Full,
            enabled,
            last_date_id: None,
        }
    }

    #[test]
    fn a_dash_clears_the_line_and_real_text_is_kept_as_written() {
        assert_eq!(wanted("-"), None);
        assert_eq!(wanted("  "), None);
        assert_eq!(
            wanted(" <@&42> look up "),
            Some("<@&42> look up".to_owned())
        );
    }

    #[test]
    fn turning_it_on_without_a_channel_says_so_rather_than_going_quiet() {
        assert_eq!(state_of(&guild(true, None)), "`on, but no channel set`");
        assert!(note(&guild(true, None)).unwrap().contains("Set a channel"));
    }

    #[test]
    fn a_finished_setup_says_announcements_are_coming() {
        assert_eq!(state_of(&guild(true, Some(9))), "`on`");
        assert!(
            note(&guild(true, Some(9)))
                .unwrap()
                .contains("will be posted")
        );
    }

    #[test]
    fn a_server_that_wants_none_of_this_is_not_nagged() {
        assert_eq!(state_of(&guild(false, Some(9))), "`off`");
        assert_eq!(note(&guild(false, Some(9))), None);
    }
}
