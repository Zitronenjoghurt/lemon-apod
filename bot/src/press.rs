use crate::card;
use crate::colour::{GREEN, GREY};
use crate::state::BotState;
use anyhow::{Context as _, Result};
use apod_core::ApodDate;
use chrono::Utc;
use poise::serenity_prelude as serenity;
use serenity::{
    ComponentInteraction, Context, CreateInteractionResponse, CreateInteractionResponseFollowup,
};

pub async fn favorite(ctx: &Context, state: &BotState, press: &ComponentInteraction) -> Result<()> {
    let Some(date) = card::favorited_date(&press.data.custom_id) else {
        return Ok(());
    };

    press
        .create_response(ctx, CreateInteractionResponse::Acknowledge)
        .await
        .context("acknowledging the press")?;

    let user_id = press.user.id.get();
    let saved = state
        .store
        .toggle_favorite(user_id, date, Utc::now())
        .await?;

    let total = state.store.favorite_count(date).await?;

    press
        .create_followup(
            ctx,
            CreateInteractionResponseFollowup::new()
                .embed(said(date, saved, total))
                .ephemeral(true),
        )
        .await
        .context("confirming the press")?;

    Ok(())
}

fn said(date: ApodDate, saved: bool, total: i64) -> serenity::CreateEmbed {
    let title = match saved {
        true => "Saved to your favorites",
        false => "Removed from your favorites",
    };

    serenity::CreateEmbed::new()
        .colour(match saved {
            true => GREEN,
            false => GREY,
        })
        .title(title)
        .description(format!(
            "{} · {}\nSee them all with `/apod favorites`.",
            date.format("%-d %B %Y"),
            people(total)
        ))
}

fn people(total: i64) -> String {
    match total {
        ..=0 => "nobody has this one saved".to_owned(),
        1 => "1 person has this one saved".to_owned(),
        total => format!("{total} people have this one saved"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(embed: serenity::CreateEmbed) -> String {
        serde_json::to_string(&embed).unwrap()
    }

    #[test]
    fn a_press_says_which_way_it_went_rather_than_just_saved() {
        let date = ApodDate::from_ymd(2026, 9, 1).unwrap();

        let on = rendered(said(date, true, 3));
        assert!(on.contains("Saved to your favorites"), "{on}");
        assert!(on.contains("1 September 2026"), "{on}");

        let off = rendered(said(date, false, 2));
        assert!(off.contains("Removed from your favorites"), "{off}");
    }

    #[test]
    fn the_tally_reads_as_a_sentence_at_every_size() {
        assert_eq!(people(0), "nobody has this one saved");
        assert_eq!(people(1), "1 person has this one saved");
        assert_eq!(people(2), "2 people have this one saved");
    }
}
