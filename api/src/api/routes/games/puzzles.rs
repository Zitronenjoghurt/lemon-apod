use super::token::{self, Kind};
use super::{Picture, Setup};
use crate::api::error::{ApiError, ApiResult};
use crate::state::ServerState;
use apod_core::apod::games;
use apod_core::{ApodDate, Cloze};
use serde::Serialize;
use std::collections::HashSet;

const MIN_GAP: i32 = 180;
const CHOICES: usize = 6;

#[derive(Debug, Serialize)]
struct Range {
    first: ApodDate,
    last: ApodDate,
}

#[derive(Debug, Serialize)]
pub struct Puzzle<T> {
    game: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    day: Option<ApodDate>,
    #[serde(flatten)]
    range: Range,
    rounds: Vec<T>,
}

#[derive(Debug, Serialize)]
pub struct Pair {
    a: Picture,
    b: Picture,
}

#[derive(Debug, Serialize)]
pub struct Match {
    round: String,
    explanation: String,
    choices: Vec<Picture>,
}

#[derive(Debug, Serialize)]
pub struct Words {
    picture: String,
    title_words: usize,
    #[serde(flatten)]
    cloze: Cloze,
}

pub async fn date(state: &ServerState, mut setup: Setup) -> ApiResult<Puzzle<Picture>> {
    let pool = state.store.picture_pool(setup.before()).await?;
    let range = range(&pool)?;

    let dates = setup.deal.take(&pool, setup.rounds);
    let rounds = state
        .store
        .summaries(&dates)
        .await?
        .iter()
        .map(Picture::of)
        .collect();

    Ok(Puzzle {
        game: "date",
        day: setup.day,
        range,
        rounds,
    })
}

pub async fn order(state: &ServerState, mut setup: Setup) -> ApiResult<Puzzle<Pair>> {
    let pool = state.store.picture_pool(setup.before()).await?;
    let range = range(&pool)?;

    let mut used: HashSet<ApodDate> = HashSet::new();
    let mut rounds = Vec::with_capacity(setup.rounds);

    for _ in 0..setup.rounds {
        let fresh: Vec<ApodDate> = pool
            .iter()
            .copied()
            .filter(|date| !used.contains(date))
            .collect();

        let Some(&first) = setup.deal.take(&fresh, 1).first() else {
            break;
        };
        let far: Vec<ApodDate> = fresh
            .iter()
            .copied()
            .filter(|date| (date.days() - first.days()).abs() >= MIN_GAP)
            .collect();
        let Some(&second) = setup
            .deal
            .take(if far.is_empty() { &fresh } else { &far }, 1)
            .first()
        else {
            break;
        };
        if first == second {
            break;
        }

        used.insert(first);
        used.insert(second);

        let summaries = state.store.summaries(&[first, second]).await?;
        if summaries.len() == 2 {
            rounds.push(Pair {
                a: Picture::of(&summaries[0]),
                b: Picture::of(&summaries[1]),
            });
        }
    }

    Ok(Puzzle {
        game: "order",
        day: setup.day,
        range,
        rounds,
    })
}

pub async fn pick(state: &ServerState, mut setup: Setup) -> ApiResult<Puzzle<Match>> {
    let pool = state.store.picture_pool(setup.before()).await?;
    let range = range(&pool)?;

    let drawn = setup.deal.take(&pool, setup.rounds * CHOICES);
    let mut rounds = Vec::with_capacity(setup.rounds);

    for group in drawn.chunks(CHOICES) {
        let (Some(&answer), true) = (group.first(), group.len() > 1) else {
            break;
        };
        let Some(entry) = state.store.entry(answer).await? else {
            continue;
        };

        let mut choices: Vec<Picture> = state
            .store
            .summaries(group)
            .await?
            .iter()
            .map(Picture::of)
            .collect();
        setup.deal.shuffle(&mut choices);

        rounds.push(Match {
            round: token::encode(Kind::Round, answer),
            explanation: entry.explanation_text,
            choices,
        });
    }

    Ok(Puzzle {
        game: "match",
        day: setup.day,
        range,
        rounds,
    })
}

pub async fn words(state: &ServerState, mut setup: Setup) -> ApiResult<Puzzle<Words>> {
    let pool = state.store.text_pool(setup.before()).await?;
    let range = range(&pool)?;

    let salt = setup.deal.seed();

    let Some(&date) = setup.deal.take(&pool, 1).first() else {
        return Err(ApiError::NotFound);
    };
    let entry = state.store.entry(date).await?.ok_or(ApiError::NotFound)?;
    let given = state.store.given_words().await?;

    let cloze = games::cloze(&entry.title, &entry.explanation_text, &given, salt);
    let title_words = apod_core::text::word_counts(&entry.title)
        .into_keys()
        .filter(|word| !given.contains(word))
        .count();

    Ok(Puzzle {
        game: "words",
        day: setup.day,
        range,
        rounds: vec![Words {
            picture: token::encode(Kind::Picture, date),
            title_words,
            cloze,
        }],
    })
}

fn range(pool: &[ApodDate]) -> ApiResult<Range> {
    match (pool.first(), pool.last()) {
        (Some(&first), Some(&last)) => Ok(Range { first, last }),
        _ => Err(ApiError::NotFound),
    }
}
