use crate::Context;
use crate::card::NAME;
use crate::error::{BotError, BotResult};
use crate::state::BotState;
use apod_core::{Filters, SearchResults};
use poise::CreateReply;
use poise::serenity_prelude as serenity;
use serenity::{
    ButtonStyle, ComponentInteractionCollector, CreateActionRow, CreateButton, CreateEmbed,
    CreateEmbedAuthor, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};

const DESCRIPTION_MAX: usize = 4096;
const SNIPPET_TOKENS: usize = 24;
const NASA_BLUE: serenity::Colour = serenity::Colour::new(0x0B_3D_91);

/// Full-text search across all Astronomy Picture of the Day.
#[poise::command(slash_command)]
pub async fn search(
    ctx: Context<'_>,
    #[description = "Words to look for in the title, explanation, credits or tags"] query: String,
) -> BotResult<()> {
    ctx.defer().await?;

    let state = ctx.data();
    let per = state.config.search_page;
    let query = query.trim().to_owned();

    let mut offset = 0;
    let mut results = run(state, &query, offset, per).await?;
    if results.items.is_empty() {
        return Err(BotError::NothingFound);
    }

    let back = format!("{}:back", ctx.id());
    let forward = format!("{}:forward", ctx.id());

    let handle = ctx
        .send(
            CreateReply::default()
                .embed(page(state, &query, &results, offset, per))
                .components(buttons(&back, &forward, offset, per, results.total)),
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
            true => offset + per,
            false => offset.saturating_sub(per),
        };
        results = run(state, &query, offset, per).await?;

        press
            .create_response(
                ctx.serenity_context(),
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .embed(page(state, &query, &results, offset, per))
                        .components(buttons(&back, &forward, offset, per, results.total)),
                ),
            )
            .await?;
    }

    handle
        .edit(
            ctx,
            CreateReply::default()
                .embed(page(state, &query, &results, offset, per))
                .components(Vec::new()),
        )
        .await?;

    Ok(())
}

async fn run(state: &BotState, query: &str, offset: usize, per: usize) -> BotResult<SearchResults> {
    Ok(state
        .apod
        .search(
            query,
            &Filters::default(),
            false,
            offset,
            per,
            SNIPPET_TOKENS,
        )
        .await?)
}

fn page(
    state: &BotState,
    query: &str,
    results: &SearchResults,
    offset: usize,
    per: usize,
) -> CreateEmbed {
    let body = results
        .items
        .iter()
        .map(|hit| {
            format!(
                "**[{title}]({url})** · {date}\n{snippet}",
                title = link_text(&hit.entry.title),
                url = state.config.entry_url(hit.entry.date),
                date = hit.entry.date.format("%-d %B %Y"),
                snippet = hit.snippet.trim(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    CreateEmbed::new()
        .colour(NASA_BLUE)
        .author(CreateEmbedAuthor::new(NAME))
        .title(format!("Search: {}", link_text(query)))
        .description(body.chars().take(DESCRIPTION_MAX).collect::<String>())
        .footer(CreateEmbedFooter::new(footer(results.total, offset, per)))
}

fn footer(total: i64, offset: usize, per: usize) -> String {
    let total = total.max(0) as usize;
    let pages = total.div_ceil(per).max(1);
    let page = offset / per + 1;

    format!(
        "Page {page} of {pages} · {total} {}",
        match total {
            1 => "entry",
            _ => "entries",
        }
    )
}

fn buttons(
    back: &str,
    forward: &str,
    offset: usize,
    per: usize,
    total: i64,
) -> Vec<CreateActionRow> {
    vec![CreateActionRow::Buttons(vec![
        CreateButton::new(back)
            .label("Back")
            .style(ButtonStyle::Secondary)
            .disabled(offset == 0),
        CreateButton::new(forward)
            .label("More")
            .style(ButtonStyle::Secondary)
            .disabled(offset + per >= total.max(0) as usize),
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
        assert_eq!(footer(12, 0, 5), "Page 1 of 3 · 12 entries");
        assert_eq!(footer(12, 5, 5), "Page 2 of 3 · 12 entries");
        assert_eq!(footer(12, 10, 5), "Page 3 of 3 · 12 entries");
    }

    #[test]
    fn one_hit_is_an_entry_and_no_hits_still_reads_as_a_sentence() {
        assert_eq!(footer(1, 0, 5), "Page 1 of 1 · 1 entry");
        assert_eq!(footer(0, 0, 5), "Page 1 of 1 · 0 entries");
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

        assert_eq!(disabled(&buttons("b", "f", 0, 5, 12)), (true, false));
        assert_eq!(disabled(&buttons("b", "f", 5, 5, 12)), (false, false));
        assert_eq!(disabled(&buttons("b", "f", 10, 5, 12)), (false, true));
        assert_eq!(
            disabled(&buttons("b", "f", 0, 5, 3)),
            (true, true),
            "a single page of results needs neither"
        );
    }

    #[test]
    fn a_title_with_brackets_cannot_break_out_of_its_own_link() {
        assert_eq!(
            link_text("M31 [the Andromeda Galaxy]"),
            "M31 \\[the Andromeda Galaxy\\]"
        );
    }
}
