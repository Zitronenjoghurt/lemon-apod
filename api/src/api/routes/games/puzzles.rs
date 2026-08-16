use super::token::{self, Kind};
use super::{Picture, Setup};
use crate::api::error::{ApiError, ApiResult};
use crate::state::ServerState;
use apod_core::apod::games;
use apod_core::{ApodDate, Cloze, GameEntry};
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
    #[serde(flatten)]
    picture: Picture,
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

    let chain = chain(&mut setup, &pool);
    let rounds = state
        .store
        .summaries(&chain)
        .await?
        .windows(2)
        .map(|pair| Pair {
            a: Picture::of(&pair[0]),
            b: Picture::of(&pair[1]),
        })
        .collect();

    Ok(Puzzle {
        game: "order",
        day: setup.day,
        range,
        rounds,
    })
}

fn chain(setup: &mut Setup, pool: &[ApodDate]) -> Vec<ApodDate> {
    let start = setup
        .from
        .filter(|date| pool.contains(date))
        .or_else(|| setup.deal.take(pool, 1).first().copied());

    let Some(start) = start else {
        return Vec::new();
    };

    let mut used: HashSet<ApodDate> = HashSet::from([start]);
    let mut chain = vec![start];

    while chain.len() <= setup.rounds {
        let last = *chain.last().expect("the chain starts with one picture");

        let mut fresh: Vec<ApodDate> = pool
            .iter()
            .copied()
            .filter(|date| !used.contains(date) && (date.days() - last.days()).abs() >= MIN_GAP)
            .collect();

        if fresh.is_empty() {
            fresh = pool
                .iter()
                .copied()
                .filter(|date| !used.contains(date))
                .collect();
        }

        let Some(&next) = setup.deal.take(&fresh, 1).first() else {
            break;
        };

        used.insert(next);
        chain.push(next);
    }

    chain
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
            picture: Picture::of(&GameEntry {
                summary: entry.to_summary(),
                credits: entry.credits.clone(),
            }),
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
