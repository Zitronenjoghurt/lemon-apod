use crate::Context;
use crate::card::NAME;
use crate::colour::{GREY, NASA_BLUE};
use crate::error::BotResult;
use crate::state::BotState;
use apod_core::ApodDate;
use poise::CreateReply;
use poise::serenity_prelude as serenity;
use serenity::{
    ButtonStyle, ComponentInteractionCollector, CreateActionRow, CreateButton, CreateEmbed,
    CreateEmbedAuthor, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};

const DESCRIPTION_MAX: usize = 4096;
const PER_PAGE: usize = 10;

/// The entries you have favorited.
#[poise::command(slash_command)]
pub async fn favorites(
    ctx: Context<'_>,
    #[description = "Clear all your favorites."] clear: Option<bool>,
) -> BotResult<()> {
    ctx.defer_ephemeral().await?;

    let state = ctx.data();
    let user_id = ctx.author().id.get();

    if clear == Some(true) {
        let gone = state.store.forget_favorites(user_id).await?;
        ctx.send(
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .colour(GREY)
                        .title("Favorites cleared")
                        .description(match gone {
                            0 => "There were none to clear.".to_owned(),
                            gone => format!("{} cleared.", entries(gone as i64)),
                        }),
                )
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let mut offset = 0;
    let mut page = read(state, user_id, offset).await?;

    if page.total == 0 {
        ctx.send(
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .colour(GREY)
                        .author(CreateEmbedAuthor::new(NAME))
                        .title("No favorites yet")
                        .description(
                            "Press the favorite button under any entry to favorite it. \
                             `/apod today` is a good place to start.",
                        ),
                )
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let back = format!("{}:back", ctx.id());
    let forward = format!("{}:forward", ctx.id());

    let handle = ctx
        .send(
            CreateReply::default()
                .embed(embed(state, &page, offset))
                .components(buttons(&back, &forward, offset, page.total))
                .ephemeral(true),
        )
        .await?;

    loop {
        let (this_back, this_forward) = (back.clone(), forward.clone());
        let pressed = ComponentInteractionCollector::new(ctx.serenity_context())
            .author_id(ctx.author().id)
            .channel_id(ctx.channel_id())
            .timeout(state.config.page_life)
            .filter(move |press| {
                press.data.custom_id == this_back || press.data.custom_id == this_forward
            })
            .await;

        let Some(press) = pressed else { break };

        offset = match press.data.custom_id == forward {
            true => offset + PER_PAGE,
            false => offset.saturating_sub(PER_PAGE),
        };
        page = read(state, user_id, offset).await?;

        press
            .create_response(
                ctx.serenity_context(),
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .embed(embed(state, &page, offset))
                        .components(buttons(&back, &forward, offset, page.total)),
                ),
            )
            .await?;
    }

    handle
        .edit(
            ctx,
            CreateReply::default()
                .embed(embed(state, &page, offset))
                .components(Vec::new()),
        )
        .await?;

    Ok(())
}

struct Page {
    items: Vec<(ApodDate, String)>,
    total: i64,
}

async fn read(state: &BotState, user_id: u64, offset: usize) -> BotResult<Page> {
    let dates = state.store.favorites(user_id, offset, PER_PAGE).await?;
    let mut items = Vec::with_capacity(dates.len());

    for date in dates {
        let title = match state.apod.entry(date).await? {
            Some(entry) => entry.title,
            None => "not in the archive".to_owned(),
        };
        items.push((date, title));
    }

    Ok(Page {
        items,
        total: state.store.favorites_total(user_id).await?,
    })
}

fn embed(state: &BotState, page: &Page, offset: usize) -> CreateEmbed {
    let body = page
        .items
        .iter()
        .map(|(date, title)| {
            format!(
                "**[{title}]({url})** · {date}",
                title = link_text(title),
                url = state.config.entry_url(*date),
                date = date.format("%-d %B %Y"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    CreateEmbed::new()
        .colour(NASA_BLUE)
        .author(CreateEmbedAuthor::new(NAME))
        .title("Your favorites")
        .description(body.chars().take(DESCRIPTION_MAX).collect::<String>())
        .footer(CreateEmbedFooter::new(footer(page.total, offset)))
}

fn footer(total: i64, offset: usize) -> String {
    let total = total.max(0) as usize;
    let pages = total.div_ceil(PER_PAGE).max(1);
    let page = offset / PER_PAGE + 1;

    format!("Page {page} of {pages} · {}", entries(total as i64))
}

fn entries(total: i64) -> String {
    match total {
        1 => "1 entry".to_owned(),
        total => format!("{total} entries"),
    }
}

fn buttons(back: &str, forward: &str, offset: usize, total: i64) -> Vec<CreateActionRow> {
    vec![CreateActionRow::Buttons(vec![
        CreateButton::new(back)
            .label("Back")
            .style(ButtonStyle::Secondary)
            .disabled(offset == 0),
        CreateButton::new(forward)
            .label("More")
            .style(ButtonStyle::Secondary)
            .disabled(offset + PER_PAGE >= total.max(0) as usize),
    ])]
}

fn link_text(text: &str) -> String {
    text.replace('[', "\\[").replace(']', "\\]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_footer_counts_pages_from_one_and_rounds_the_last_partial_page_up() {
        assert_eq!(footer(25, 0), "Page 1 of 3 · 25 entries");
        assert_eq!(footer(25, 10), "Page 2 of 3 · 25 entries");
        assert_eq!(footer(25, 20), "Page 3 of 3 · 25 entries");
    }

    #[test]
    fn one_favorite_reads_as_a_sentence() {
        assert_eq!(footer(1, 0), "Page 1 of 1 · 1 entry");
        assert_eq!(entries(0), "0 entries");
    }

    #[test]
    fn back_is_dead_on_the_first_page_and_more_is_dead_on_the_last() {
        let disabled = |rows: &[CreateActionRow]| {
            let json = serde_json::to_value(rows).unwrap();
            let row = &json[0]["components"];
            (
                row[0]["disabled"].as_bool().unwrap_or(false),
                row[1]["disabled"].as_bool().unwrap_or(false),
            )
        };

        assert_eq!(disabled(&buttons("b", "f", 0, 25)), (true, false));
        assert_eq!(disabled(&buttons("b", "f", 10, 25)), (false, false));
        assert_eq!(disabled(&buttons("b", "f", 20, 25)), (false, true));
        assert_eq!(
            disabled(&buttons("b", "f", 0, 4)),
            (true, true),
            "a single page needs neither"
        );
    }

    #[test]
    fn a_title_with_brackets_cannot_break_out_of_its_own_link() {
        assert_eq!(link_text("M31 [Andromeda]"), "M31 \\[Andromeda\\]");
    }
}
