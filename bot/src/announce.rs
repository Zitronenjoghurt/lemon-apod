use crate::card;
use crate::state::BotState;
use crate::store::{Explanation, Guild};
use apod_core::ApodEntry;
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use poise::serenity_prelude as serenity;
use serenity::{ChannelId, CreateAttachment, CreateEmbed, CreateMessage, HttpError, UserId};
use std::collections::HashMap;

const CONTENT_MAX: usize = 2000;
const UNKNOWN_CHANNEL: isize = 10003;

const CANNOT_DM: [isize; 2] = [50007, 50278];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pass {
    pub sent: usize,
    pub failed: usize,
}

pub async fn run(ctx: serenity::Context, state: BotState) {
    let poll = state.config.announce.poll;
    let settle = state.config.announce.settle;
    tracing::info!(?poll, ?settle, "watching the archive for a new entry");

    loop {
        match pass(&ctx, &state, Utc::now()).await {
            Ok(pass) if pass.sent > 0 || pass.failed > 0 => {
                tracing::info!(sent = pass.sent, failed = pass.failed, "announced");
            }
            Ok(_) => {}
            Err(error) => tracing::warn!("announcement pass failed: {error:#}"),
        }

        tokio::time::sleep(poll).await;
    }
}

pub async fn pass(
    ctx: &serenity::Context,
    state: &BotState,
    now: DateTime<Utc>,
) -> anyhow::Result<Pass> {
    let Some(entry) = state.apod.latest().await? else {
        return Ok(Pass::default());
    };

    if !is_fresh(&entry, now, state.config.announce.max_age) {
        return Ok(Pass::default());
    }

    if let Some(waiting_for) = entry.settling(state.config.announce.settle, now) {
        tracing::info!(
            date = %entry.date,
            %waiting_for,
            "the archive has not finished this entry, holding the announcement"
        );
        return Ok(Pass::default());
    }

    let owed = state.store.owed(entry.date.days()).await?;
    let subscribers = state.store.owed_subscribers(entry.date.days()).await?;
    if owed.is_empty() && subscribers.is_empty() {
        return Ok(Pass::default());
    }

    let attachment = card::thumbnail(&state.config, &entry).await;
    let mut embeds: HashMap<Explanation, CreateEmbed> = HashMap::new();

    let mut pass = Pass::default();
    for guild in owed {
        let embed = embeds
            .entry(guild.explanation)
            .or_insert_with(|| {
                card::embed(
                    &state.config,
                    &entry,
                    guild.explanation,
                    attachment.as_ref(),
                )
            })
            .clone();

        match post(ctx, &guild, embed, attachment.clone()).await {
            Ok(()) => {
                state
                    .store
                    .mark(guild.guild_id, entry.date.days(), now)
                    .await?;
                pass.sent += 1;
                tracing::info!(
                    guild = guild.guild_id,
                    date = %entry.date,
                    "posted the new entry"
                );
            }
            Err(error) => {
                pass.failed += 1;
                if channel_is_gone(&error) {
                    tracing::warn!(
                        guild = guild.guild_id,
                        "the channel is gone, forgetting it rather than retrying every minute"
                    );
                    state.store.forget_channel(guild.guild_id, now).await?;
                } else {
                    tracing::warn!(guild = guild.guild_id, "could not post: {error}");
                }
            }
        }
    }

    for user in subscribers {
        let embed = embeds
            .entry(user.explanation)
            .or_insert_with(|| {
                card::embed(&state.config, &entry, user.explanation, attachment.as_ref())
            })
            .clone();

        match dm(ctx, user.user_id, embed, attachment.clone()).await {
            Ok(()) => {
                state
                    .store
                    .mark_subscriber(user.user_id, entry.date.days(), now)
                    .await?;
                pass.sent += 1;
                tracing::info!(user = user.user_id, date = %entry.date, "sent the new entry");
            }
            Err(error) => {
                pass.failed += 1;
                if cannot_dm(&error) {
                    tracing::warn!(
                        user = user.user_id,
                        "Discord will not deliver a DM, unsubscribing them"
                    );
                    state.store.unsubscribe(user.user_id, now).await?;
                } else {
                    tracing::warn!(user = user.user_id, "could not send: {error}");
                }
            }
        }
    }

    Ok(pass)
}

pub async fn dm(
    ctx: &serenity::Context,
    user_id: u64,
    embed: CreateEmbed,
    attachment: Option<CreateAttachment>,
) -> serenity::Result<()> {
    let mut message = CreateMessage::new().embed(embed);
    if let Some(attachment) = attachment {
        message = message.add_file(attachment);
    }

    UserId::new(user_id).direct_message(ctx, message).await?;
    Ok(())
}

pub fn cannot_dm(error: &serenity::Error) -> bool {
    matches!(
        error,
        serenity::Error::Http(HttpError::UnsuccessfulRequest(response))
            if CANNOT_DM.contains(&response.error.code)
    )
}

pub async fn post(
    ctx: &serenity::Context,
    guild: &Guild,
    embed: CreateEmbed,
    attachment: Option<CreateAttachment>,
) -> serenity::Result<()> {
    let Some(channel_id) = guild.channel_id else {
        return Ok(());
    };

    let mut message = CreateMessage::new().embed(embed);

    if let Some(content) = content(guild) {
        message = message.content(content);
    }
    if let Some(attachment) = attachment {
        message = message.add_file(attachment);
    }

    ChannelId::new(channel_id)
        .send_message(ctx, message)
        .await?;
    Ok(())
}

fn content(guild: &Guild) -> Option<String> {
    let message = guild.message.as_deref()?.trim();
    if message.is_empty() {
        return None;
    }

    Some(message.chars().take(CONTENT_MAX).collect())
}

fn is_fresh(entry: &ApodEntry, now: DateTime<Utc>, max_age: std::time::Duration) -> bool {
    let published = entry.date.naive().and_time(NaiveTime::MIN).and_utc();
    let Ok(max_age) = TimeDelta::from_std(max_age) else {
        return false;
    };

    now >= published - TimeDelta::days(1) && now - published <= max_age
}

fn channel_is_gone(error: &serenity::Error) -> bool {
    matches!(
        error,
        serenity::Error::Http(HttpError::UnsuccessfulRequest(response))
            if response.error.code == UNKNOWN_CHANNEL
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Subscriber;
    use apod_core::entry::Provenance;
    use apod_core::{Media, MediaKind};
    use chrono::TimeZone;
    use std::time::Duration;

    const DAY: Duration = Duration::from_secs(24 * 3600);

    fn entry(date: &str) -> ApodEntry {
        ApodEntry {
            date: date.parse().unwrap(),
            title: "A Nebula".into(),
            title_raw: None,
            explanation_html: "Cloud.".into(),
            explanation_text: "Cloud.".into(),
            credits: Vec::new(),
            has_copyright: false,
            license_url: None,
            tomorrow_teaser: None,
            keywords: Vec::new(),
            media: Media::new(MediaKind::ImageJpg, None, None),
            extra_media: Vec::new(),
            legacy_media_url: None,
            first_stored_at: None,
            alt: None,
            authors: Vec::new(),
            provenance: Provenance::Both,
            source_url: String::new(),
            picture: None,
        }
    }

    fn at(year: i32, month: u32, day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, 0, 0).unwrap()
    }

    #[test]
    fn the_two_refusals_discord_uses_for_a_closed_dm_are_both_recognised() {
        assert_eq!(CANNOT_DM, [50_007, 50_278]);
        assert!(!CANNOT_DM.contains(&UNKNOWN_CHANNEL));
    }

    #[test]
    fn a_subscriber_is_owed_an_entry_on_the_same_freshness_window_a_channel_is() {
        let today = entry("2026-08-30");
        let user = Subscriber {
            user_id: 5,
            explanation: Explanation::Teaser,
            enabled: true,
            last_date_id: None,
        };

        assert_eq!(user.explanation, Explanation::Teaser);
        assert!(is_fresh(&today, at(2026, 8, 30, 6), DAY + DAY / 2));
        assert!(!is_fresh(&today, at(2026, 9, 3, 6), DAY + DAY / 2));
    }

    fn guild(message: Option<&str>) -> Guild {
        Guild {
            guild_id: 1,
            channel_id: Some(2),
            message: message.map(str::to_owned),
            explanation: Explanation::Full,
            enabled: true,
            last_date_id: None,
        }
    }

    #[test]
    fn todays_entry_is_fresh_and_last_weeks_is_not() {
        let today = entry("2026-08-30");

        assert!(is_fresh(&today, at(2026, 8, 30, 6), DAY + DAY / 2));
        assert!(is_fresh(&today, at(2026, 8, 31, 6), DAY + DAY / 2));
        assert!(
            !is_fresh(&today, at(2026, 9, 3, 6), DAY + DAY / 2),
            "a guild that enables the bot on Thursday is not owed Monday"
        );
    }

    #[test]
    fn an_entry_dated_ahead_of_the_clock_is_not_announced_early() {
        let ahead = entry("2026-09-05");
        assert!(
            !is_fresh(&ahead, at(2026, 8, 30, 6), DAY + DAY / 2),
            "a date the archive got ahead of itself on is not today's picture"
        );
        assert!(
            is_fresh(&ahead, at(2026, 9, 4, 20), DAY + DAY / 2),
            "but the last hours of the day before are still it, wherever the reader is"
        );
    }

    #[test]
    fn a_guild_that_set_no_message_gets_no_line_above_the_embed() {
        assert_eq!(content(&guild(None)), None);
        assert_eq!(content(&guild(Some("   "))), None);
        assert_eq!(
            content(&guild(Some("<@&42> new picture"))).as_deref(),
            Some("<@&42> new picture")
        );
    }

    #[test]
    fn a_message_longer_than_discord_accepts_is_cut_rather_than_refused() {
        let long = "x".repeat(3_000);
        let cut = content(&guild(Some(&long))).unwrap();
        assert_eq!(cut.chars().count(), CONTENT_MAX);
    }
}
