use crate::Context;
use crate::error::{BotError, BotResult};
use crate::store::Explanation;
use crate::{announce, card};
use apod_core::ApodDate;
use chrono::Utc;
use poise::CreateReply;
use poise::serenity_prelude::{Colour, CreateEmbed};

const GREY: Colour = Colour::new(0xA0_97_A1);
const GREEN: Colour = Colour::new(0xAC_B5_65);

/// Get each new Astronomy Picture of the Day as a direct message.
#[poise::command(slash_command)]
pub async fn dm(
    ctx: Context<'_>,
    #[description = "Activate daily announcements"] subscribe: Option<bool>,
    #[description = "How much of the explanation to include"] explanation: Option<Explanation>,
) -> BotResult<()> {
    ctx.defer_ephemeral().await?;

    let state = ctx.data();
    let user_id = ctx.author().id.get();
    let mut user = state.store.subscriber(user_id).await?;

    let changing = subscribe.is_some() || explanation.is_some();
    let turning_on = subscribe == Some(true) && !user.enabled;

    if let Some(subscribe) = subscribe {
        user.enabled = subscribe;
    }
    if let Some(explanation) = explanation {
        user.explanation = explanation;
    }

    if turning_on {
        let entry = state
            .apod
            .latest()
            .await?
            .ok_or(BotError::NoEntry(ApodDate::START))?;

        let attachment = card::thumbnail(&state.config, &entry).await;
        let embed = card::embed(&state.config, &entry, user.explanation, attachment.as_ref());

        if let Err(error) = announce::dm(ctx.serenity_context(), user_id, embed, attachment).await {
            if announce::cannot_dm(&error) {
                ctx.send(
                    CreateReply::default()
                        .embed(crate::error::notice(
                            "Discord will not let me DM you",
                            "Your privacy settings, or the fact that we share no server, stop me from sending you messages outside of command replies. Join a server the bot is in, and allow direct messages from server members, then try again.",
                        ))
                        .ephemeral(true),
                )
                .await?;
                return Ok(());
            }
            return Err(error.into());
        }

        state.store.save_subscriber(&user, Utc::now()).await?;
        state
            .store
            .mark_subscriber(user_id, entry.date.days(), Utc::now())
            .await?;

        ctx.send(
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .colour(GREEN)
                        .title("Subscribed")
                        .description(format!(
                            "{} was successfully sent to your DMs and the next ones will be sent daily as long as we share a server and you allow direct messages from server members.",
                            entry.date
                        )),
                )
                .ephemeral(true),
        )
        .await?;

        return Ok(());
    }

    if changing {
        state.store.save_subscriber(&user, Utc::now()).await?;
    }

    ctx.send(
        CreateReply::default()
            .embed(summary(user.enabled, user.explanation, changing))
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

fn summary(enabled: bool, explanation: Explanation, changed: bool) -> CreateEmbed {
    CreateEmbed::new()
        .colour(match changed {
            true => GREEN,
            false => GREY,
        })
        .title(match (changed, enabled) {
            (true, true) => "Saved",
            (true, false) => "Unsubscribed",
            (false, _) => "Your direct messages",
        })
        .field(
            "Direct messages",
            match enabled {
                true => "`on`",
                false => "`off`",
            },
            false,
        )
        .field("Explanation", format!("`{explanation}`"), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(embed: CreateEmbed) -> String {
        serde_json::to_string(&embed).unwrap()
    }

    #[test]
    fn turning_it_off_says_so_rather_than_reporting_it_as_saved() {
        assert!(rendered(summary(false, Explanation::Full, true)).contains("Unsubscribed"));
        assert!(rendered(summary(true, Explanation::Full, true)).contains("Saved"));
    }

    #[test]
    fn asking_without_changing_anything_just_reports() {
        let json = rendered(summary(true, Explanation::Teaser, false));
        assert!(json.contains("Your direct messages"), "{json}");
        assert!(json.contains("teaser"), "{json}");
    }
}
