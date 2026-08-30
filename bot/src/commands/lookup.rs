use crate::Context;
use crate::commands::show;
use crate::error::{BotError, BotResult};
use apod_core::{ApodDate, KindFilter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, poise::ChoiceParameter)]
pub enum Kind {
    #[name = "Anything"]
    Any,
    #[name = "A picture"]
    Picture,
    #[name = "A video"]
    Video,
}

impl Kind {
    fn filter(self) -> Option<KindFilter> {
        match self {
            Self::Any => None,
            Self::Picture => KindFilter::new(KindFilter::IMAGE),
            Self::Video => KindFilter::new(KindFilter::VIDEO),
        }
    }
}

/// Today's Astronomy Picture of the Day.
#[poise::command(slash_command)]
pub async fn today(ctx: Context<'_>) -> BotResult<()> {
    ctx.defer().await?;

    let entry = ctx
        .data()
        .apod
        .latest()
        .await?
        .ok_or(BotError::NoEntry(ApodDate::START))?;

    show(ctx, &entry).await
}

/// The Astronomy Picture of the Day for a specific date.
#[poise::command(slash_command)]
pub async fn date(
    ctx: Context<'_>,
    #[description = "The day to look up, as YYYY-MM-DD"] date: String,
) -> BotResult<()> {
    ctx.defer().await?;

    let parsed: ApodDate = date
        .trim()
        .parse()
        .map_err(|_| BotError::NotADate(date.trim().to_owned()))?;

    let entry = ctx
        .data()
        .apod
        .entry(parsed)
        .await?
        .ok_or(BotError::NoEntry(parsed))?;

    show(ctx, &entry).await
}

/// A random Astronomy Picture of the Day.
#[poise::command(slash_command)]
pub async fn random(
    ctx: Context<'_>,
    #[description = "Narrow it to pictures or to videos"] kind: Option<Kind>,
) -> BotResult<()> {
    ctx.defer().await?;

    let filter = kind.unwrap_or(Kind::Any).filter();
    let state = ctx.data();

    let date = state
        .apod
        .random(filter.as_ref())
        .await?
        .ok_or(BotError::NothingFound)?;

    let entry = state
        .apod
        .entry(date)
        .await?
        .ok_or(BotError::NoEntry(date))?;

    show(ctx, &entry).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use apod_core::MediaKind;

    #[test]
    fn anything_is_no_filter_at_all_rather_than_a_filter_listing_every_kind() {
        assert!(Kind::Any.filter().is_none());
    }

    #[test]
    fn a_picture_and_a_video_are_different_sets_and_neither_is_empty() {
        let picture = Kind::Picture.filter().expect("images are a set");
        let video = Kind::Video.filter().expect("videos are a set");

        assert_ne!(picture, video);
        assert!(KindFilter::IMAGE.contains(&MediaKind::ImageJpg));
        assert!(KindFilter::VIDEO.contains(&MediaKind::YouTube));
    }
}
