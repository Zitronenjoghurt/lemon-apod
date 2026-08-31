pub mod ballot;
pub mod cohort;
pub mod nonce;

use crate::config::Rating as Settings;
use anyhow::{Context, Result};
use apod_core::rating::store::{Cast, Standing, VoteStore, VoterId, Whose};
use apod_core::rating::{
    self, Candidate, Category, Grouping, Outcome, Pairing, Pool, Prior, Progress,
};
use apod_core::{ApodDate, ApodReader};
use ballot::{Ballot, BallotError};
use chrono::{DateTime, TimeDelta, Utc};
use nonce::Nonces;
use rand::RngExt;
use serde::Serialize;
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct Who {
    pub voter: Option<VoterId>,
    pub cohort: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Denied {
    #[error("The archive is not ready to be rated yet; try again shortly")]
    Unavailable,
    #[error("vote budget spent")]
    OverBudget(Budget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Budget {
    pub scope: Scope,
    pub allowed: u64,
    pub window_secs: u64,
    pub retry_after: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Voter,
    Network,
}

impl Budget {
    fn spent(
        scope: Scope,
        allowed: u64,
        window: Duration,
        frees_at: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Self {
        let retry_after = frees_at
            .map(|frees_at| (frees_at - now).num_seconds().max(0) as u64)
            .unwrap_or_else(|| window.as_secs())
            .max(1);

        Self {
            scope,
            allowed,
            window_secs: window.as_secs(),
            retry_after,
        }
    }
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

        let window = self.settings.budget_window;
        let allowed = self.settings.votes_per_window;

        if let Some(voter) = who.voter
            && standing.votes >= allowed
        {
            let frees_at = self
                .frees_at(Whose::Voter(voter), since, standing.votes - allowed, window)
                .await;
            return Err(Denied::OverBudget(Budget::spent(
                Scope::Voter,
                allowed,
                window,
                frees_at,
                now,
            )));
        }

        if let Some(cohort) = &who.cohort {
            let allowed = self.settings.cohort_votes_per_window;
            let votes = self
                .store
                .cohort_votes(cohort, since)
                .await
                .unwrap_or_default();

            if votes >= allowed {
                let frees_at = self
                    .frees_at(Whose::Cohort(cohort), since, votes - allowed, window)
                    .await;
                return Err(Denied::OverBudget(Budget::spent(
                    Scope::Network,
                    allowed,
                    window,
                    frees_at,
                    now,
                )));
            }
        }

        Ok(standing)
    }

    async fn frees_at(
        &self,
        whose: Whose<'_>,
        since: DateTime<Utc>,
        over_by: u64,
        window: Duration,
    ) -> Option<DateTime<Utc>> {
        let ages_out = self
            .store
            .ages_out_at(whose, since, over_by)
            .await
            .unwrap_or_default()?;

        Some(ages_out + TimeDelta::from_std(window).ok()?)
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

    const HOUR: Duration = Duration::from_secs(3_600);

    #[test]
    fn a_spent_budget_counts_the_wait_from_when_the_window_frees_up() {
        let now = Utc::now();
        let frees_at = now + TimeDelta::minutes(42);
        let budget = Budget::spent(Scope::Voter, 300, HOUR, Some(frees_at), now);

        assert_eq!(budget.retry_after, 42 * 60);
        assert_eq!(budget.allowed, 300);
        assert_eq!(budget.window_secs, 3_600);
        assert_eq!(budget.scope, Scope::Voter);
    }

    #[test]
    fn a_budget_with_nothing_in_the_log_waits_the_whole_window_rather_than_guessing() {
        let now = Utc::now();
        let blind = Budget::spent(Scope::Network, 1_000, HOUR, None, now);

        assert_eq!(
            blind.retry_after, 3_600,
            "with no vote to age out, the only answer that cannot send them back early is the \
             whole window"
        );
        assert_eq!(blind.scope, Scope::Network);
    }

    #[test]
    fn a_wait_that_has_already_passed_still_asks_for_a_moment() {
        let now = Utc::now();
        let gone = Budget::spent(
            Scope::Voter,
            300,
            HOUR,
            Some(now - TimeDelta::hours(2)),
            now,
        );

        assert_eq!(
            gone.retry_after, 1,
            "zero would read as a green light and put the client straight back into a refusal"
        );
    }

    #[test]
    fn the_refusal_carries_facts_and_not_a_sentence() {
        let now = Utc::now();
        let budget = Budget::spent(Scope::Voter, 300, HOUR, Some(now), now);
        let said = serde_json::to_string(&budget).unwrap();

        assert!(said.contains("\"scope\":\"voter\""), "{said}");
        assert!(
            !said.contains("You "),
            "the words the reader sees belong to the client in front of them, not to the \
             archive: {said}"
        );
    }

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
