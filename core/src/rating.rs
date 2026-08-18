pub mod baseline;
pub mod fit;
pub mod model;
pub mod pool;
#[cfg(feature = "rating-data")]
pub mod store;

pub use baseline::{Dataset, Manifest, Row};
pub use fit::{Anchor, Fit, Grouping, Outcome, Prior, Score, Vote, fit, tiers};
pub use model::{Category, Progress, Stage};
pub use pool::{Candidate, Draw, Pairing, Pool};
#[cfg(feature = "rating-data")]
pub use store::{Cast, Ranked, Standing, Tally, VoteStore, Voter};

pub const MODEL: &str = "bt-map-1";
pub const Z: f64 = 1.96;

/// The width of the weak `Normal(0, sigma^2)` prior on every score.
pub const DEFAULT_SIGMA: f64 = 2.0;

/// The prior on the left-hand advantage.
pub const SIDE_BIAS_SIGMA: f64 = 1.0;

/// The Fisher information one evenly matched comparison carries, `p(1-p)` at `p = 0.5`.
pub const COMPARISON_INFORMATION: f64 = 0.25;

/// Below this a picture is showing the prior rather than an opinion, so it is left off the board
/// rather than listed at the average.
pub const MIN_COMPARISONS: u32 = 5;

/// The ceiling on a baseline row's effective sample size. Without it a baseline built from
/// 90,000 votes permanently outvotes everything collected afterwards and the committed file
/// becomes a fossil that cannot be corrected.
pub const BASELINE_MAX_ESS: f64 = 50.0;
