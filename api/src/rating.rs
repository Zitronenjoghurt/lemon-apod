pub mod ballot;
pub mod cohort;
pub mod nonce;

use crate::config::Rating as Settings;
use anyhow::{Context, Result};
use apod_core::rating::store::{Cast, Standing, VoteStore, VoterId};
use apod_core::rating::{
    self, Candidate, Category, Grouping, Outcome, Pairing, Pool, Prior, Progress,
};
use apod_core::{ApodDate, ApodReader};
use ballot::{Ballot, BallotError};
use chrono::{DateTime, Utc};
use nonce::Nonces;
use rand::RngExt;
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct Who {
    pub voter: Option<VoterId>,
    pub cohort: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Denied {
    #[error("the archive is not ready to be rated yet; try again shortly")]
    Unavailable,
    #[error("that is a lot of votes in an hour. Come back a bit later")]
    OverBudget,
}

#[derive(Debug, Clone)]
pub struct Issued {
    pub ballot: Ballot,
    pub token: String,
}

#[derive(Default)]
struct Live {
    grouping: Arc<Grouping>,
    pools: HashMap<Category, Pool>,
    pool: u64,
}

pub struct Rating {
    pub store: VoteStore,
    pub settings: Settings,
    secret: Vec<u8>,
    nonces: Nonces,
    live: RwLock<Live>,
}

impl Rating {
    pub async fn open(path: &Path, settings: Settings) -> Result<Self> {
        let store = VoteStore::open(path)
            .await
            .with_context(|| format!("opening {}", path.display()))?;

        Ok(Self {
            secret: settings.secret(),
            nonces: Nonces::new(settings.ballot_life),
            live: RwLock::new(Live::default()),
            store,
            settings,
        })
    }

    pub fn cohort(&self, address: Option<IpAddr>, user_agent: Option<&str>) -> Option<Vec<u8>> {
        address.map(|address| cohort::cohort(&self.secret, address, user_agent))
    }

    pub async fn check(
        &self,
        who: &Who,
        category: Category,
        now: DateTime<Utc>,
    ) -> Result<Standing, Denied> {
        let since = now - self.settings.budget_window;

        let standing = match who.voter {
            None => Standing::default(),
            Some(voter) => self
                .store
                .standing(
                    voter,
                    category,
                    self.settings.recent,
                    self.settings.per_picture,
                    since,
                    now - self.settings.probe_after,
                )
                .await
                .map_err(|error| {
                    tracing::error!("reading a voter's standing: {error}");
                    Denied::Unavailable
                })?,
        };

        if standing.votes >= self.settings.votes_per_window {
            return Err(Denied::OverBudget);
        }

        if let Some(cohort) = &who.cohort {
            let votes = self
                .store
                .cohort_votes(cohort, since)
                .await
                .unwrap_or_default();
            if votes >= self.settings.cohort_votes_per_window {
                return Err(Denied::OverBudget);
            }
        }

        Ok(standing)
    }

    pub async fn draw(
        &self,
        category: Category,
        who: &Who,
        standing: &Standing,
        also_avoid: &[ApodDate],
        now: DateTime<Utc>,
    ) -> Result<Issued, Denied> {
        let seed: u64 = rand::rng().random();
        let nonce: u64 = rand::rng().random();

        let live = self.live.read().await;
        let pool = live.pools.get(&category).ok_or(Denied::Unavailable)?;

        let avoid: Vec<ApodDate> = standing
            .avoid
            .iter()
            .chain(also_avoid)
            .map(|&entry| live.grouping.group(entry))
            .collect();

        let repeat = standing.probe.and_then(|(left, right)| {
            let pair = (live.grouping.group(left), live.grouping.group(right));
            pool.repeat(pair.0, pair.1)
        });

        let drawn = match Pairing::pick(seed, repeat.is_some()) {
            Pairing::Probe => repeat,
            other => pool.draw(seed, other, &avoid),
        }
        .or_else(|| pool.draw(seed, Pairing::Informative, &avoid))
        .ok_or(Denied::Unavailable)?;

        let ballot = Ballot::new(
            category,
            drawn.left,
            drawn.right,
            drawn.pairing,
            who.voter,
            now,
            nonce,
        );

        Ok(Issued {
            token: ballot.sign(&self.secret),
            ballot,
        })
    }

    pub fn verify(
        &self,
        token: &str,
        voter: Option<VoterId>,
        now: DateTime<Utc>,
    ) -> Result<Ballot, BallotError> {
        let ballot = Ballot::open(&self.secret, token)?;
        ballot.belongs_to(voter)?;

        if !ballot.fresh(now, self.settings.ballot_life_delta()) {
            return Err(BallotError::Stale);
        }

        Ok(ballot)
    }

    pub fn spend(&self, ballot: &Ballot) -> Result<(), BallotError> {
        match self.nonces.claim(ballot.nonce) {
            true => Ok(()),
            false => Err(BallotError::Spent),
        }
    }

    pub async fn record(
        &self,
        ballot: &Ballot,
        voter: VoterId,
        cohort: Option<Vec<u8>>,
        outcome: Outcome,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let response = ballot.response(now);

        if response < self.settings.min_response_delta() {
            tracing::debug!(
                ?voter,
                ms = response.num_milliseconds(),
                "a vote came back faster than a judgment takes"
            );
        }
        if !ballot.fresh(now, self.settings.ballot_fresh_delta()) {
            tracing::debug!(
                ?voter,
                secs = response.num_seconds(),
                "a vote came back on a ballot that had been sitting a while"
            );
        }

        let cast = Cast {
            voter,
            cohort,
            category: ballot.category,
            left: ballot.left,
            right: ballot.right,
            outcome,
            issued_at: ballot.issued_at,
            voted_at: now,
            probe: ballot.probe,
        };

        self.store
            .record(&cast, now - self.settings.cohort_life)
            .await
            .context("recording a vote")?;
        Ok(())
    }

    pub async fn progress(&self, votes: u64) -> Progress {
        Progress::of(self.live.read().await.pool, votes)
    }

    pub async fn pool_size(&self) -> u64 {
        self.live.read().await.pool
    }

    pub async fn ready(&self) -> bool {
        !self.live.read().await.pools.is_empty()
    }

    pub async fn refit(&self, index: &ApodReader, before: Option<ApodDate>) -> Result<()> {
        let grouping = Arc::new(Grouping::new(
            index
                .picture_groups()
                .await
                .context("reading the picture groups")?,
        ));
        let eligible = index
            .picture_pool(before)
            .await
            .context("reading the eligible pictures")?;

        let mut pools = HashMap::new();
        for category in Category::ALL {
            let anchors = self.store.anchors(category).await?;
            let log = self.store.log(category).await?;
            let fit = rating::fit(&log, &grouping, &Prior::weak().anchored(anchors));

            self.store.save(category, &fit).await?;

            let candidates: Vec<Candidate> = eligible
                .iter()
                .map(|&picture| match fit.score(picture) {
                    Some(score) => Candidate {
                        picture,
                        score: score.score,
                        stderr: score.stderr,
                        comparisons: score.comparisons,
                    },
                    None => Candidate::unseen(picture),
                })
                .collect();

            let votes = self.store.tally(category).await?.votes;
            let stage = Progress::of(eligible.len() as u64, votes).stage;
            let pool = match stage.contenders() {
                None => Pool::new(candidates),
                Some(keep) => Pool::focused(candidates, keep as usize),
            };

            tracing::debug!(
                category = category.as_str(),
                votes = fit.votes,
                dropped = fit.self_matches,
                pictures = fit.scores.len(),
                iterations = fit.iterations,
                bias = fit.side_bias,
                stage = stage.as_str(),
                contenders = pool.contenders(),
                "fitted"
            );

            pools.insert(category, pool);
        }

        let mut live = self.live.write().await;
        live.grouping = grouping;
        live.pool = eligible.len() as u64;
        live.pools = pools;

        Ok(())
    }

    pub async fn sweep(&self, now: DateTime<Utc>) -> Result<()> {
        let cohorts = self
            .store
            .expire_cohorts(now - self.settings.cohort_life)
            .await?;
        let voters = self
            .store
            .forget_stale(now - self.settings.cookie_life)
            .await?;

        if cohorts > 0 || voters > 0 {
            tracing::info!(cohorts, voters, "swept the voter table");
        }
        Ok(())
    }

    pub fn spent_ballots(&self) -> usize {
        self.nonces.spent()
    }
}

pub fn weighted_category(beautiful_share: u32) -> Category {
    match rand::rng().random_range(0..100) < beautiful_share {
        true => Category::Beautiful,
        false => Category::Fascinating,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mix_leans_on_the_board_that_has_to_reach_significance_first() {
        let mut beautiful = 0;
        for _ in 0..10_000 {
            if weighted_category(65) == Category::Beautiful {
                beautiful += 1;
            }
        }

        assert!(
            (6_200..6_800).contains(&beautiful),
            "{beautiful} of 10,000 went to the first board"
        );
    }

    #[test]
    fn an_even_split_is_one_setting_away() {
        let mut beautiful = 0;
        for _ in 0..10_000 {
            if weighted_category(50) == Category::Beautiful {
                beautiful += 1;
            }
        }

        assert!((4_700..5_300).contains(&beautiful), "{beautiful} of 10,000");
    }

    #[test]
    fn a_share_of_a_hundred_never_asks_the_second_question() {
        for _ in 0..1_000 {
            assert_eq!(weighted_category(100), Category::Beautiful);
        }
    }
}
