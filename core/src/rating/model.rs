use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Beautiful,
    Fascinating,
}

impl Category {
    pub const ALL: [Self; 2] = [Self::Beautiful, Self::Fascinating];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Beautiful => "beautiful",
            Self::Fascinating => "fascinating",
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("'{0}' is not a category anybody is voting on")]
pub struct CategoryParseError(String);

impl FromStr for Category {
    type Err = CategoryParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw {
            "beautiful" => Ok(Self::Beautiful),
            "fascinating" => Ok(Self::Fascinating),
            other => Err(CategoryParseError(other.to_owned())),
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub const SCREEN_DEPTH: u64 = 10;
pub const CONTEND_POOL: u64 = 500;
pub const CONTEND_DEPTH: u64 = 100;
pub const SETTLE_POOL: u64 = 50;
pub const SETTLE_DEPTH: u64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    /// Sorting the whole archive into rough bands.
    Screen,
    /// Separating the few hundred that could belong in the best hundred.
    Contend,
    /// Trying to separate the top few, which may not separate at all.
    Settle,
    /// Every stage target met.
    Settled,
}

impl Stage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Screen => "screen",
            Self::Contend => "contend",
            Self::Settle => "settle",
            Self::Settled => "settled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub stage: Stage,
    pub votes: u64,
    pub done: u64,
    pub target: u64,
    pub total: u64,
}

impl Progress {
    pub fn of(pool: u64, votes: u64) -> Self {
        let screen = pool * SCREEN_DEPTH / 2;
        let contend = pool.min(CONTEND_POOL) * CONTEND_DEPTH / 2;
        let settle = pool.min(SETTLE_POOL) * SETTLE_DEPTH / 2;
        let total = screen + contend + settle;

        let (stage, done, target) = if votes < screen {
            (Stage::Screen, votes, screen)
        } else if votes < screen + contend {
            (Stage::Contend, votes - screen, contend)
        } else if votes < total {
            (Stage::Settle, votes - screen - contend, settle)
        } else {
            (Stage::Settled, settle, settle)
        };

        Self {
            stage,
            votes,
            done,
            target,
            total,
        }
    }

    pub fn fraction(&self) -> f64 {
        if self.target == 0 {
            return 1.0;
        }
        (self.done as f64 / self.target as f64).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POOL: u64 = 9_582;

    #[test]
    fn a_category_survives_a_round_trip_through_its_name() {
        for category in Category::ALL {
            assert_eq!(category.as_str().parse::<Category>().unwrap(), category);
        }
        assert!("impressive".parse::<Category>().is_err());
        assert!("".parse::<Category>().is_err());
    }

    #[test]
    fn the_whole_funnel_costs_about_ninety_thousand_votes() {
        let progress = Progress::of(POOL, 0);
        assert_eq!(progress.total, 47_910 + 25_000 + 15_000);
        assert_eq!(progress.stage, Stage::Screen);
        assert_eq!(progress.target, 47_910, "the whole archive, ten deep");
    }

    #[test]
    fn each_stage_reports_its_own_share_rather_than_the_running_total() {
        let screening = Progress::of(POOL, 12_400);
        assert_eq!(screening.stage, Stage::Screen);
        assert_eq!((screening.done, screening.target), (12_400, 47_910));

        let contending = Progress::of(POOL, 50_000);
        assert_eq!(contending.stage, Stage::Contend);
        assert_eq!(
            (contending.done, contending.target),
            (2_090, 25_000),
            "a stage bar that started at 50,000 of 25,000 would read as nonsense"
        );

        let settling = Progress::of(POOL, 80_000);
        assert_eq!(settling.stage, Stage::Settle);
        assert_eq!((settling.done, settling.target), (7_090, 15_000));
    }

    #[test]
    fn a_finished_funnel_reads_full_and_stays_there() {
        let done = Progress::of(POOL, 500_000);
        assert_eq!(done.stage, Stage::Settled);
        assert_eq!(done.done, done.target);
        assert_eq!(done.fraction(), 1.0);
        assert_eq!(done.votes, 500_000, "the count keeps going up");
    }

    #[test]
    fn a_small_pool_does_not_ask_for_more_depth_than_it_has_pictures_for() {
        let progress = Progress::of(20, 0);
        assert_eq!(progress.target, 100, "twenty pictures, ten deep");
        assert_eq!(progress.total, 100 + 20 * 50 + 20 * 300);
    }

    #[test]
    fn an_empty_pool_is_finished_rather_than_dividing_by_nothing() {
        let progress = Progress::of(0, 0);
        assert_eq!(progress.total, 0);
        assert_eq!(progress.stage, Stage::Settled);
        assert_eq!(progress.fraction(), 1.0);
    }
}
