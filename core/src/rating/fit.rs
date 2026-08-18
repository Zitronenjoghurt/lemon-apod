use super::{COMPARISON_INFORMATION, DEFAULT_SIGMA, SIDE_BIAS_SIGMA, Z};
use crate::date::ApodDate;
use std::collections::HashMap;
use std::str::FromStr;

const DAMPING: f64 = 0.5;
const MAX_ITERATIONS: u32 = 500;
const TOLERANCE: f64 = 1e-11;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Left,
    Right,
    Tie,
}

impl Outcome {
    const fn to_left(self) -> f64 {
        match self {
            Self::Left => 1.0,
            Self::Right => 0.0,
            Self::Tie => 0.5,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Tie => "tie",
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("'{0}' is not one of left, right or tie")]
pub struct OutcomeParseError(String);

impl FromStr for Outcome {
    type Err = OutcomeParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw {
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            "tie" => Ok(Self::Tie),
            other => Err(OutcomeParseError(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Vote {
    pub left: ApodDate,
    pub right: ApodDate,
    pub outcome: Outcome,
    pub weight: f64,
}

impl Vote {
    pub fn new(left: ApodDate, right: ApodDate, outcome: Outcome) -> Self {
        Self {
            left,
            right,
            outcome,
            weight: 1.0,
        }
    }

    pub fn weighted(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Anchor {
    pub picture: ApodDate,
    pub score: f64,
    /// How many evenly matched comparisons the score is worth. Capped on import, so a baseline
    /// is a strong opinion rather than an unappealable one.
    pub ess: f64,
}

/// Which entries show the same picture, as the current grouping pass sees it. An entry nobody
/// grouped is its own picture.
#[derive(Debug, Clone, Default)]
pub struct Grouping {
    of: HashMap<ApodDate, ApodDate>,
}

impl Grouping {
    pub fn new(pairs: impl IntoIterator<Item = (ApodDate, ApodDate)>) -> Self {
        Self {
            of: pairs.into_iter().collect(),
        }
    }

    pub fn group(&self, entry: ApodDate) -> ApodDate {
        self.of.get(&entry).copied().unwrap_or(entry)
    }

    pub fn is_empty(&self) -> bool {
        self.of.is_empty()
    }
}

/// A `Normal(mean, sigma^2)` prior on every score.
#[derive(Debug, Clone)]
pub struct Prior {
    precision: f64,
    anchors: HashMap<ApodDate, (f64, f64)>,
}

impl Prior {
    pub fn weak() -> Self {
        Self::with_sigma(DEFAULT_SIGMA)
    }

    pub fn with_sigma(sigma: f64) -> Self {
        Self {
            precision: if sigma > 0.0 { sigma.powi(-2) } else { 0.0 },
            anchors: HashMap::new(),
        }
    }

    pub fn none() -> Self {
        Self {
            precision: 0.0,
            anchors: HashMap::new(),
        }
    }

    pub fn anchored(mut self, anchors: impl IntoIterator<Item = Anchor>) -> Self {
        for anchor in anchors {
            let precision = (anchor.ess * COMPARISON_INFORMATION).max(self.precision);
            self.anchors
                .insert(anchor.picture, (anchor.score, precision));
        }
        self
    }

    fn mean(&self, picture: ApodDate) -> f64 {
        self.anchors.get(&picture).map_or(0.0, |&(mean, _)| mean)
    }

    fn precision_for(&self, picture: ApodDate) -> f64 {
        self.anchors
            .get(&picture)
            .map_or(self.precision, |&(_, precision)| precision)
    }

    fn pictures(&self) -> impl Iterator<Item = ApodDate> + '_ {
        self.anchors.keys().copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score {
    pub picture: ApodDate,
    pub score: f64,
    pub stderr: f64,
    pub comparisons: u32,
}

impl Score {
    pub fn lower(&self) -> f64 {
        self.score - Z * self.stderr
    }

    pub fn upper(&self) -> f64 {
        self.score + Z * self.stderr
    }
}

#[derive(Debug, Clone)]
pub struct Fit {
    /// Every picture a vote or an anchor mentions, in date order. A picture nobody has voted on
    /// is absent rather than listed at the average.
    pub scores: Vec<Score>,
    /// The left-hand advantage. Sides are randomised per ballot, so near zero means the
    /// randomisation worked and this parameter cost nothing.
    pub side_bias: f64,
    pub iterations: u32,
    /// Votes the fit actually used.
    pub votes: usize,
    pub self_matches: usize,
}

impl Fit {
    pub fn score(&self, picture: ApodDate) -> Option<&Score> {
        self.scores
            .binary_search_by_key(&picture, |score| score.picture)
            .ok()
            .map(|at| &self.scores[at])
    }
}

struct Pair {
    left: usize,
    right: usize,
    to_left: f64,
    weight: f64,
}

pub fn fit(votes: &[Vote], grouping: &Grouping, prior: &Prior) -> Fit {
    let mut index: HashMap<ApodDate, usize> = HashMap::new();
    let mut pictures: Vec<ApodDate> = Vec::new();
    let mut pairs: Vec<Pair> = Vec::with_capacity(votes.len());
    let mut self_matches = 0;

    for vote in votes {
        let (left, right) = (grouping.group(vote.left), grouping.group(vote.right));
        if left == right {
            self_matches += 1;
            continue;
        }

        pairs.push(Pair {
            left: slot(&mut index, &mut pictures, left),
            right: slot(&mut index, &mut pictures, right),
            to_left: vote.outcome.to_left(),
            weight: vote.weight.max(0.0),
        });
    }

    for picture in prior.pictures() {
        slot(&mut index, &mut pictures, picture);
    }

    let count = pictures.len();
    let mean: Vec<f64> = pictures.iter().map(|&p| prior.mean(p)).collect();
    let precision: Vec<f64> = pictures.iter().map(|&p| prior.precision_for(p)).collect();
    let bias_precision = SIDE_BIAS_SIGMA.powi(-2);

    let mut theta = mean.clone();
    let mut bias = 0.0;
    let mut gradient = vec![0.0; count];
    let mut information = vec![0.0; count];
    let mut iterations = 0;

    for step in 1..=MAX_ITERATIONS {
        iterations = step;
        let bias_slope = accumulate(&pairs, &theta, bias, &mut gradient, &mut information);
        let mut moved: f64 = 0.0;

        for at in 0..count {
            let curvature = information[at] + precision[at];
            if curvature <= 0.0 {
                continue;
            }

            let slope = gradient[at] - (theta[at] - mean[at]) * precision[at];
            let move_by = DAMPING * slope / curvature;
            theta[at] += move_by;
            moved = moved.max(move_by.abs());
        }

        let move_bias = DAMPING * (bias_slope.slope - bias * bias_precision)
            / (bias_slope.curvature + bias_precision);
        bias += move_bias;
        moved = moved.max(move_bias.abs());

        moved = moved.max(realign(&mut theta, &mean, &precision).abs());

        if moved < TOLERANCE {
            break;
        }
    }

    accumulate(&pairs, &theta, bias, &mut gradient, &mut information);

    let mut comparisons = vec![0u32; count];
    for pair in &pairs {
        comparisons[pair.left] += 1;
        comparisons[pair.right] += 1;
    }

    let mut scores: Vec<Score> = (0..count)
        .map(|at| Score {
            picture: pictures[at],
            score: theta[at],
            stderr: (information[at] + precision[at])
                .max(f64::MIN_POSITIVE)
                .sqrt()
                .recip(),
            comparisons: comparisons[at],
        })
        .collect();
    scores.sort_unstable_by_key(|score| score.picture);

    Fit {
        scores,
        side_bias: bias,
        iterations,
        votes: pairs.len(),
        self_matches,
    }
}

struct Slope {
    slope: f64,
    curvature: f64,
}

fn accumulate(
    pairs: &[Pair],
    theta: &[f64],
    bias: f64,
    gradient: &mut [f64],
    information: &mut [f64],
) -> Slope {
    gradient.fill(0.0);
    information.fill(0.0);

    let mut bias_slope = Slope {
        slope: 0.0,
        curvature: 0.0,
    };

    for pair in pairs {
        let expected = logistic(theta[pair.left] - theta[pair.right] + bias);
        let residual = pair.weight * (pair.to_left - expected);
        let curvature = pair.weight * expected * (1.0 - expected);

        gradient[pair.left] += residual;
        gradient[pair.right] -= residual;
        information[pair.left] += curvature;
        information[pair.right] += curvature;
        bias_slope.slope += residual;
        bias_slope.curvature += curvature;
    }

    bias_slope
}

fn slot(
    index: &mut HashMap<ApodDate, usize>,
    pictures: &mut Vec<ApodDate>,
    picture: ApodDate,
) -> usize {
    *index.entry(picture).or_insert_with(|| {
        pictures.push(picture);
        pictures.len() - 1
    })
}

fn realign(theta: &mut [f64], mean: &[f64], precision: &[f64]) -> f64 {
    if theta.is_empty() {
        return 0.0;
    }

    let weight: f64 = precision.iter().sum();
    let shift = match weight > 0.0 {
        true => {
            theta
                .iter()
                .zip(mean)
                .zip(precision)
                .map(|((value, mean), precision)| precision * (mean - value))
                .sum::<f64>()
                / weight
        }
        false => -theta.iter().sum::<f64>() / theta.len() as f64,
    };

    theta.iter_mut().for_each(|value| *value += shift);
    shift
}

fn logistic(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

pub fn tiers(ranked: &[Score]) -> Vec<u32> {
    let mut out = Vec::with_capacity(ranked.len());
    let mut tier = 0;
    let mut leader = f64::INFINITY;

    for score in ranked {
        if score.upper() < leader {
            tier += 1;
            leader = score.lower();
        }
        out.push(tier);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(n: i32) -> ApodDate {
        ApodDate::from_days(n)
    }

    fn votes(left: ApodDate, right: ApodDate, wins: usize, losses: usize) -> Vec<Vote> {
        let mut out = Vec::new();
        out.extend((0..wins).map(|_| Vote::new(left, right, Outcome::Left)));
        out.extend((0..losses).map(|_| Vote::new(left, right, Outcome::Right)));
        out
    }

    fn balanced(one: ApodDate, two: ApodDate, wins: usize, losses: usize) -> Vec<Vote> {
        (0..wins + losses)
            .map(|at| {
                let (won, on_left) = (at < wins, at % 2 == 0);
                let outcome = match won == on_left {
                    true => Outcome::Left,
                    false => Outcome::Right,
                };

                match on_left {
                    true => Vote::new(one, two, outcome),
                    false => Vote::new(two, one, outcome),
                }
            })
            .collect()
    }

    fn difference(fit: &Fit, left: ApodDate, right: ApodDate) -> f64 {
        fit.score(left).unwrap().score - fit.score(right).unwrap().score
    }

    #[test]
    fn seven_wins_in_ten_is_the_log_odds_of_seven_to_three_exactly() {
        let (a, b) = (day(0), day(1));
        let fit = fit(&votes(a, b, 7, 3), &Grouping::default(), &Prior::none());

        assert!(
            (difference(&fit, a, b) - (7.0f64 / 3.0).ln()).abs() < 1e-9,
            "expected ln(7/3) = {}, got {}",
            (7.0f64 / 3.0).ln(),
            difference(&fit, a, b)
        );
        assert_eq!(fit.votes, 10);
        assert_eq!(fit.scores.len(), 2);
        assert!(
            fit.side_bias.abs() < 1e-9,
            "with every vote the same way round there is no side habit to find, got {}",
            fit.side_bias
        );
    }

    #[test]
    fn an_even_split_and_a_pile_of_ties_both_say_the_two_are_level() {
        let (a, b) = (day(0), day(1));

        let split = fit(&votes(a, b, 5, 5), &Grouping::default(), &Prior::none());
        assert!(difference(&split, a, b).abs() < 1e-9);

        let drawn: Vec<Vote> = (0..10).map(|_| Vote::new(a, b, Outcome::Tie)).collect();
        let tied = fit(&drawn, &Grouping::default(), &Prior::none());
        assert!(difference(&tied, a, b).abs() < 1e-9);
        assert_eq!(tied.votes, 10, "a tie is a comparison, not a discard");
    }

    #[test]
    fn two_ties_carry_what_one_win_and_one_loss_carry() {
        let (a, b) = (day(0), day(1));
        let mut mixed = votes(a, b, 6, 2);
        mixed.push(Vote::new(a, b, Outcome::Tie));
        mixed.push(Vote::new(a, b, Outcome::Tie));

        let fit = fit(&mixed, &Grouping::default(), &Prior::none());
        assert!(
            (difference(&fit, a, b) - (7.0f64 / 3.0).ln()).abs() < 1e-9,
            "six wins, two losses and two ties is seven of ten"
        );
    }

    #[test]
    fn a_picture_that_has_never_lost_gets_a_finite_score_from_the_prior() {
        let (a, b) = (day(0), day(1));
        let fit = fit(&balanced(a, b, 20, 0), &Grouping::default(), &Prior::weak());

        let unbeaten = fit.score(a).unwrap();
        assert!(
            unbeaten.score.is_finite() && unbeaten.score < 6.0,
            "the prior has to bound it, got {}",
            unbeaten.score
        );
        assert!(unbeaten.score > fit.score(b).unwrap().score);
    }

    #[test]
    fn the_prior_pulls_a_thin_record_towards_average_and_leaves_a_thick_one_alone() {
        let (a, b, c, d) = (day(0), day(1), day(2), day(3));

        let thin = fit(&balanced(a, b, 3, 0), &Grouping::default(), &Prior::weak());
        let thick = fit(
            &balanced(c, d, 300, 0),
            &Grouping::default(),
            &Prior::weak(),
        );

        assert!(
            difference(&thin, a, b) < difference(&thick, c, d),
            "three votes must not claim what three hundred claim"
        );
    }

    #[test]
    fn a_picture_nobody_can_separate_from_average_has_a_wider_interval_than_a_settled_one() {
        let (a, b, c, d) = (day(0), day(1), day(2), day(3));

        let mut log = balanced(a, b, 3, 2);
        log.extend(balanced(c, d, 150, 150));
        let fit = fit(&log, &Grouping::default(), &Prior::weak());

        let thin = fit.score(a).unwrap();
        let thick = fit.score(c).unwrap();
        assert!(thin.stderr > thick.stderr);
        assert_eq!(thin.comparisons, 5);
        assert_eq!(thick.comparisons, 300);
    }

    #[test]
    fn the_standard_error_lands_near_two_over_the_root_of_the_comparison_count() {
        let (a, b) = (day(0), day(1));
        let fit = fit(
            &balanced(a, b, 50, 50),
            &Grouping::default(),
            &Prior::weak(),
        );

        let stderr = fit.score(a).unwrap().stderr;
        let expected = 2.0 / 100.0f64.sqrt();
        assert!(
            (stderr - expected).abs() < 0.01,
            "expected about {expected}, got {stderr}"
        );
    }

    #[test]
    fn scores_recover_a_known_ordering_across_a_connected_graph() {
        let truth: Vec<f64> = vec![2.0, 1.0, 0.0, -1.0, -2.0];
        let mut log = Vec::new();

        // Every pair meets, and each meeting goes the way the true scores say it should as
        // often as the model would have it.
        for i in 0..truth.len() {
            for j in (i + 1)..truth.len() {
                let expected = logistic(truth[i] - truth[j]);
                let rounds = 400;
                let wins = (expected * rounds as f64).round() as usize;
                log.extend(votes(day(i as i32), day(j as i32), wins, rounds - wins));
            }
        }

        let fit = fit(&log, &Grouping::default(), &Prior::weak());
        for (at, &expected) in truth.iter().enumerate() {
            let found = fit.score(day(at as i32)).unwrap().score;
            assert!(
                (found - expected).abs() < 0.15,
                "picture {at} should be near {expected}, got {found}"
            );
        }
    }

    #[test]
    fn a_left_hand_habit_is_found_rather_than_smeared_across_the_scores() {
        let mut log = Vec::new();
        // Twenty evenly matched pictures, and the left one wins four times in five.
        for at in 0..10 {
            let (left, right) = (day(at * 2), day(at * 2 + 1));
            log.extend(votes(left, right, 80, 20));
            log.extend(votes(right, left, 80, 20));
        }

        let fit = fit(&log, &Grouping::default(), &Prior::weak());
        assert!(
            (fit.side_bias - 4.0f64.ln()).abs() < 0.1,
            "expected a bias near ln(4), got {}",
            fit.side_bias
        );

        let spread = fit
            .scores
            .iter()
            .map(|score| score.score.abs())
            .fold(0.0f64, f64::max);
        assert!(spread < 0.1, "the pictures are level and should read level");
    }

    #[test]
    fn randomised_sides_leave_the_bias_at_nothing() {
        let (a, b) = (day(0), day(1));
        let mut log = votes(a, b, 35, 15);
        log.extend(votes(b, a, 15, 35));

        let fit = fit(&log, &Grouping::default(), &Prior::weak());
        assert!(fit.side_bias.abs() < 1e-6, "got {}", fit.side_bias);
    }

    #[test]
    fn a_vote_records_the_entry_and_the_fit_collapses_it_to_the_picture() {
        let (first, rerun, other) = (day(0), day(100), day(200));
        let grouping = Grouping::new([(first, first), (rerun, first), (other, other)]);

        let mut log = votes(first, other, 4, 1);
        log.extend(votes(rerun, other, 3, 2));

        let fit = fit(&log, &grouping, &Prior::weak());
        assert_eq!(fit.scores.len(), 2, "the rerun is not its own competitor");
        assert_eq!(fit.score(first).unwrap().comparisons, 10);
        assert!(fit.score(rerun).is_none());
    }

    #[test]
    fn a_vote_that_regrouping_turned_into_a_self_match_is_dropped() {
        let (first, rerun) = (day(0), day(100));
        let log = votes(first, rerun, 3, 2);

        let apart = fit(&log, &Grouping::default(), &Prior::weak());
        assert_eq!(apart.votes, 5);
        assert_eq!(apart.self_matches, 0);

        let together = fit(
            &log,
            &Grouping::new([(first, first), (rerun, first)]),
            &Prior::weak(),
        );
        assert_eq!(together.votes, 0);
        assert_eq!(together.self_matches, 5);
        assert!(
            together.scores.is_empty(),
            "with nothing usable left the fit has nothing to say about the picture"
        );
    }

    #[test]
    fn a_baseline_anchor_starts_a_picture_where_it_was_left_and_live_votes_move_it() {
        let (a, b) = (day(0), day(1));
        let anchors = [
            Anchor {
                picture: a,
                score: 1.5,
                ess: 50.0,
            },
            Anchor {
                picture: b,
                score: 0.0,
                ess: 50.0,
            },
        ];

        let cold = fit(&[], &Grouping::default(), &Prior::weak().anchored(anchors));
        assert!((cold.score(a).unwrap().score - 1.5).abs() < 1e-6);
        assert_eq!(
            cold.score(a).unwrap().comparisons,
            0,
            "an inherited score is not a comparison anybody made"
        );

        let live = fit(
            &balanced(a, b, 0, 50),
            &Grouping::default(),
            &Prior::weak().anchored(anchors),
        );
        assert!(
            live.score(a).unwrap().score < 1.0,
            "fifty losses have to be able to argue with the baseline, got {}",
            live.score(a).unwrap().score
        );
    }

    #[test]
    fn a_capped_anchor_cannot_outvote_what_comes_after_it() {
        let (a, b) = (day(0), day(1));
        let anchors = [Anchor {
            picture: a,
            score: 3.0,
            ess: 50.0,
        }];

        let fit = fit(
            &balanced(a, b, 100, 900),
            &Grouping::default(),
            &Prior::weak().anchored(anchors),
        );
        assert!(
            fit.score(a).unwrap().score < fit.score(b).unwrap().score,
            "a thousand votes saying otherwise has to win, got {}",
            fit.score(a).unwrap().score
        );
    }

    #[test]
    fn a_voter_the_fit_stopped_believing_moves_nothing() {
        let (a, b) = (day(0), day(1));
        let honest = balanced(a, b, 5, 5);

        let mut with_troll = honest.clone();
        with_troll.extend((0..100).map(|_| Vote::new(a, b, Outcome::Left).weighted(0.0)));

        let plain = fit(&honest, &Grouping::default(), &Prior::weak());
        let weighted = fit(&with_troll, &Grouping::default(), &Prior::weak());

        assert!((difference(&plain, a, b) - difference(&weighted, a, b)).abs() < 1e-9);
        assert_eq!(
            weighted.score(a).unwrap().comparisons,
            110,
            "a discounted vote still happened"
        );
    }

    #[test]
    fn an_empty_log_is_an_empty_board_rather_than_a_panic() {
        let fit = fit(&[], &Grouping::default(), &Prior::weak());
        assert!(fit.scores.is_empty());
        assert_eq!(fit.side_bias, 0.0);
        assert_eq!(fit.votes, 0);
    }

    #[test]
    fn the_same_log_in_a_different_order_gives_the_same_scores() {
        let (a, b, c) = (day(0), day(1), day(2));
        let mut log = balanced(a, b, 7, 3);
        log.extend(balanced(b, c, 6, 4));
        log.extend(balanced(a, c, 8, 2));

        let forward = fit(&log, &Grouping::default(), &Prior::weak());
        log.reverse();
        let backward = fit(&log, &Grouping::default(), &Prior::weak());

        for (one, two) in forward.scores.iter().zip(&backward.scores) {
            assert_eq!(one.picture, two.picture);
            assert!(
                (one.score - two.score).abs() < 1e-9,
                "Elo would disagree with itself here and this must not"
            );
        }
    }

    #[test]
    fn a_thin_record_ranks_below_a_thick_one_it_beat_on_the_scoreboard() {
        let (unbeaten, solid, filler) = (day(0), day(1), day(2));
        let mut log = balanced(unbeaten, filler, 3, 0);
        log.extend(balanced(solid, filler, 180, 120));

        let fit = fit(&log, &Grouping::default(), &Prior::weak());
        let thin = fit.score(unbeaten).unwrap();
        let thick = fit.score(solid).unwrap();

        assert!(thin.score > thick.score, "on the point estimate it leads");
        assert!(
            thin.lower() < thick.lower(),
            "and on the lower bound it does not, which is the one the board sorts on"
        );
    }

    #[test]
    fn overlapping_intervals_are_one_tier_and_a_clear_gap_starts_another() {
        let ranked = vec![
            Score {
                picture: day(0),
                score: 2.0,
                stderr: 0.1,
                comparisons: 400,
            },
            Score {
                picture: day(1),
                score: 1.95,
                stderr: 0.1,
                comparisons: 400,
            },
            Score {
                picture: day(2),
                score: 0.5,
                stderr: 0.1,
                comparisons: 400,
            },
        ];

        assert_eq!(tiers(&ranked), vec![1, 1, 2]);
        assert!(tiers(&[]).is_empty());
    }

    #[test]
    fn a_wide_interval_keeps_a_picture_in_the_tier_above_it() {
        let ranked = vec![
            Score {
                picture: day(0),
                score: 2.0,
                stderr: 0.05,
                comparisons: 1_000,
            },
            Score {
                picture: day(1),
                score: 1.0,
                stderr: 0.8,
                comparisons: 6,
            },
        ];

        assert_eq!(
            tiers(&ranked),
            vec![1, 1],
            "six comparisons cannot be called worse than a thousand"
        );
    }
}
