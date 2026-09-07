use crate::Context;
use crate::card::NAME;
use crate::colour::NASA_BLUE;
use crate::error::{BotError, BotResult};
use apod_core::ApodDate;
use chrono::Datelike;
use poise::CreateReply;
use poise::serenity_prelude::{CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter};

const DESCRIPTION_MAX: usize = 4096;

/// Every Astronomy Picture of the Day that ran on a specified calendar day.
#[poise::command(slash_command, rename = "on-this-day")]
pub async fn on_this_day(
    ctx: Context<'_>,
    #[description = "The calendar day, as MM-DD. Today if you leave it out"] day: Option<String>,
) -> BotResult<()> {
    ctx.defer().await?;

    let (month, day) = match day.as_deref().map(str::trim).filter(|day| !day.is_empty()) {
        Some(raw) => month_day(raw).ok_or_else(|| BotError::NotADay(raw.to_owned()))?,
        None => day_of(ApodDate::today_utc()),
    };

    let state = ctx.data();
    let entries = state.apod.on_this_day(month, day).await?;
    if entries.is_empty() {
        return Err(BotError::NothingFound);
    }

    let body: String = entries
        .iter()
        .map(|entry| {
            format!(
                "**{year}** · [{title}]({url})",
                year = entry.date.format("%Y"),
                title = link_text(&entry.title),
                url = state.config.entry_url(entry.date),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let embed = CreateEmbed::new()
        .colour(NASA_BLUE)
        .author(CreateEmbedAuthor::new(NAME))
        .title(heading(month, day))
        .description(body.chars().take(DESCRIPTION_MAX).collect::<String>())
        .footer(CreateEmbedFooter::new(footer(entries.len())));

    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}

fn month_day(raw: &str) -> Option<(u32, u32)> {
    if let Ok(date) = raw.parse::<ApodDate>() {
        return Some(day_of(date));
    }

    raw.split_once('-')
        .and_then(|(month, day)| Some((month.parse::<u32>().ok()?, day.parse::<u32>().ok()?)))
        .filter(|(month, day)| (1..=12).contains(month) && (1..=31).contains(day))
}

fn day_of(date: ApodDate) -> (u32, u32) {
    let date = date.naive();
    (date.month(), date.day())
}

fn heading(month: u32, day: u32) -> String {
    let name = match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        _ => "December",
    };

    format!("On {day} {name}")
}

fn footer(shown: usize) -> String {
    format!(
        "{shown} {}",
        match shown {
            1 => "entry",
            _ => "entries",
        }
    )
}

fn link_text(text: &str) -> String {
    text.replace('[', "\\[").replace(']', "\\]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_calendar_day_is_read_the_way_the_api_reads_one() {
        assert_eq!(month_day("06-16"), Some((6, 16)));
        assert_eq!(month_day("6-16"), Some((6, 16)));
        assert_eq!(
            month_day("1995-06-16"),
            Some((6, 16)),
            "a full date names a calendar day too, and somebody will type one"
        );
    }

    #[test]
    fn a_day_that_is_not_a_day_is_turned_away_rather_than_clamped() {
        assert_eq!(month_day("13-01"), None);
        assert_eq!(month_day("00-01"), None);
        assert_eq!(month_day("06-32"), None);
        assert_eq!(month_day("06-00"), None);
        assert_eq!(month_day("june"), None);
        assert_eq!(month_day(""), None);
    }

    #[test]
    fn the_heading_names_the_day_rather_than_repeating_the_numbers() {
        assert_eq!(heading(6, 16), "On 16 June");
        assert_eq!(heading(12, 1), "On 1 December");
    }

    #[test]
    fn one_entry_reads_as_a_sentence() {
        assert_eq!(footer(1), "1 entry");
        assert_eq!(footer(31), "31 entries");
    }
}
