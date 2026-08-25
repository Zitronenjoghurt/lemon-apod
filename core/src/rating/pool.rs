use crate::date::ApodDate;
use std::collections::HashMap;

pub const BAND_WIDTH: f64 = 0.25;
pub const INFORMATIVE_SHARE: u32 = 70;
pub const UNIFORM_SHARE: u32 = 25;
pub const PROBE_SHARE: u32 = 5;

const TOURNAMENT: usize = 4;
const ATTEMPTS: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub picture: ApodDate,
    pub score: f64,
    pub stderr: f64,
    pub comparisons: u32,
}

impl Candidate {
    pub fn unseen(picture: ApodDate) -> Self {
        Self {
            picture,
            score: 0.0,
            stderr: super::DEFAULT_SIGMA,
            comparisons: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pairing {
    Informative,
    Uniform,
    Probe,
}

impl Pairing {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Informative => "informative",
            Self::Uniform => "uniform",
            Self::Probe => "probe",
        }
    }

    pub fn pick(roll: u64, probe_available: bool) -> Self {
        let share = (roll % 100) as u32;

        if share < PROBE_SHARE {
            return if probe_available {
                Self::Probe
            } else {
                Self::Informative
            };
        }
        if share < PROBE_SHARE + UNIFORM_SHARE {
            return Self::Uniform;
        }
        Self::Informative
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Draw {
    pub left: ApodDate,
    pub right: ApodDate,
    pub pairing: Pairing,
}

impl Draw {
    pub fn holds(&self, picture: ApodDate) -> bool {
        self.left == picture || self.right == picture
    }
}

#[derive(Debug, Clone)]
struct Band {
    uncertainty: f64,
    members: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct Pool {
    candidates: Vec<Candidate>,
    at: HashMap<ApodDate, usize>,
    contenders: usize,
    bands: Vec<Band>,
    uncertainty: f64,
}

impl Pool {
    pub fn new(candidates: Vec<Candidate>) -> Self {
        Self::build(candidates, None)
    }

    pub fn focused(candidates: Vec<Candidate>, keep: usize) -> Self {
        Self::build(candidates, Some(keep))
    }

    fn build(mut candidates: Vec<Candidate>, keep: Option<usize>) -> Self {
        candidates.sort_unstable_by_key(|candidate| candidate.picture);
        candidates.dedup_by_key(|candidate| candidate.picture);

        let at: HashMap<ApodDate, usize> = candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| (candidate.picture, index))
            .collect();

        let focus: Vec<usize> = match keep {
            None => (0..candidates.len()).collect(),
            Some(keep) => {
                let mut order: Vec<usize> = (0..candidates.len()).collect();
                order.sort_unstable_by(|&one, &two| {
                    upper(&candidates[two])
                        .total_cmp(&upper(&candidates[one]))
                        .then(candidates[one].picture.cmp(&candidates[two].picture))
                });
                order.truncate(keep.max(2).min(candidates.len()));
                order
            }
        };

        let mut grouped: HashMap<i64, Band> = HashMap::new();
        for &index in &focus {
            let candidate = &candidates[index];
            let band = grouped
                .entry((candidate.score / BAND_WIDTH).floor() as i64)
                .or_insert_with(|| Band {
                    uncertainty: 0.0,
                    members: Vec::new(),
                });
            band.uncertainty += candidate.stderr * candidate.stderr;
            band.members.push(index);
        }

        let mut keys: Vec<i64> = grouped.keys().copied().collect();
        keys.sort_unstable();
        let bands: Vec<Band> = keys
            .into_iter()
            .map(|key| grouped.remove(&key).expect("key came from the map"))
            .collect();

        Self {
            uncertainty: bands.iter().map(|band| band.uncertainty).sum(),
            contenders: focus.len(),
            candidates,
            at,
            bands,
        }
    }

    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// How many of them the informative draws are choosing between.
    pub fn contenders(&self) -> usize {
        self.contenders
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    pub fn bands(&self) -> usize {
        self.bands.len()
    }

    pub fn contains(&self, picture: ApodDate) -> bool {
        self.at.contains_key(&picture)
    }

    pub fn candidate(&self, picture: ApodDate) -> Option<&Candidate> {
        self.at.get(&picture).map(|&index| &self.candidates[index])
    }

    pub fn draw(&self, seed: u64, pairing: Pairing, avoid: &[ApodDate]) -> Option<Draw> {
        if self.candidates.len() < 2 {
            return None;
        }

        let mut rng = Rng::new(seed);
        let picked = match pairing {
            Pairing::Informative => self
                .informative(&mut rng, avoid)
                .or_else(|| self.uniform(&mut rng, avoid)),
            Pairing::Uniform | Pairing::Probe => self.uniform(&mut rng, avoid),
        }?;

        Some(self.sided(picked, pairing, &mut rng))
    }

    pub fn repeat(&self, left: ApodDate, right: ApodDate) -> Option<Draw> {
        let (one, two) = (*self.at.get(&left)?, *self.at.get(&right)?);
        if one == two {
            return None;
        }

        Some(Draw {
            left: self.candidates[two].picture,
            right: self.candidates[one].picture,
            pairing: Pairing::Probe,
        })
    }

    fn sided(&self, (one, two): (usize, usize), pairing: Pairing, rng: &mut Rng) -> Draw {
        let (left, right) = match rng.next() & 1 == 0 {
            true => (one, two),
            false => (two, one),
        };

        Draw {
            left: self.candidates[left].picture,
            right: self.candidates[right].picture,
            pairing,
        }
    }

    fn informative(&self, rng: &mut Rng, avoid: &[ApodDate]) -> Option<(usize, usize)> {
        for _ in 0..ATTEMPTS {
            let band = self.band(rng)?;
            if band.members.len() < 2 {
                continue;
            }

            let one = self.thinnest(rng, &band.members, avoid, usize::MAX)?;
            if let Some(two) = self.thinnest(rng, &band.members, avoid, one) {
                return Some((one, two));
            }
        }

        None
    }

    fn uniform(&self, rng: &mut Rng, avoid: &[ApodDate]) -> Option<(usize, usize)> {
        let mut first = None;

        for _ in 0..ATTEMPTS * 2 {
            let index = rng.below(self.candidates.len());
            if self.barred(index, avoid) {
                continue;
            }

            match first {
                None => first = Some(index),
                Some(one) if one != index => return Some((one, index)),
                Some(_) => {}
            }
        }

        None
    }

    fn thinnest(
        &self,
        rng: &mut Rng,
        members: &[usize],
        avoid: &[ApodDate],
        taken: usize,
    ) -> Option<usize> {
        let mut best: Option<usize> = None;

        for _ in 0..TOURNAMENT {
            let index = members[rng.below(members.len())];
            if index == taken || self.barred(index, avoid) {
                continue;
            }
            if best.is_none_or(|held| {
                self.candidates[index].comparisons < self.candidates[held].comparisons
            }) {
                best = Some(index);
            }
        }

        best
    }

    fn barred(&self, index: usize, avoid: &[ApodDate]) -> bool {
        avoid.contains(&self.candidates[index].picture)
    }

    fn band(&self, rng: &mut Rng) -> Option<&Band> {
        if self.uncertainty <= 0.0 {
            return self.bands.get(rng.below(self.bands.len().max(1)));
        }

        let mut target = rng.fraction() * self.uncertainty;
        for band in &self.bands {
            target -= band.uncertainty;
            if target <= 0.0 {
                return Some(band);
            }
        }

        self.bands.last()
    }
}

fn upper(candidate: &Candidate) -> f64 {
    candidate.score + super::Z * candidate.stderr
}

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, limit: usize) -> usize {
        match limit {
            0 => 0,
            limit => (self.next() % limit as u64) as usize,
        }
    }

    fn fraction(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(n: i32) -> ApodDate {
        ApodDate::from_days(n)
    }

    fn scored(picture: i32, score: f64, comparisons: u32) -> Candidate {
        Candidate {
            picture: day(picture),
            score,
            stderr: 2.0 / (comparisons.max(1) as f64).sqrt(),
            comparisons,
        }
    }

    fn spread(count: i32) -> Pool {
        Pool::new(
            (0..count)
                .map(|at| scored(at, at as f64 / 10.0 - count as f64 / 20.0, 20))
                .collect(),
        )
    }

    #[test]
    fn the_shares_come_out_near_seventy_twenty_five_and_five() {
        let mut counts = [0; 3];
        for seed in 0..10_000u64 {
            let roll = Rng::new(seed).next();
            match Pairing::pick(roll, true) {
                Pairing::Informative => counts[0] += 1,
                Pairing::Uniform => counts[1] += 1,
                Pairing::Probe => counts[2] += 1,
            }
        }

        assert!((6_700..7_300).contains(&counts[0]), "{counts:?}");
        assert!((2_300..2_700).contains(&counts[1]), "{counts:?}");
        assert!((400..600).contains(&counts[2]), "{counts:?}");
    }

    #[test]
    fn a_voter_with_nothing_to_repeat_gets_an_informative_pair_instead() {
        let mut probes = 0;
        for seed in 0..1_000u64 {
            if Pairing::pick(Rng::new(seed).next(), false) == Pairing::Probe {
                probes += 1;
            }
        }
        assert_eq!(probes, 0, "a repeat needs a history to repeat from");
    }

    #[test]
    fn an_informative_pair_is_close_enough_to_be_worth_asking_about() {
        let pool = spread(400);
        let mut close = 0;
        let mut drawn = 0;

        for seed in 0..500u64 {
            let Some(draw) = pool.draw(seed, Pairing::Informative, &[]) else {
                continue;
            };
            drawn += 1;

            let one = pool.candidate(draw.left).unwrap().score;
            let two = pool.candidate(draw.right).unwrap().score;
            if (one - two).abs() <= BAND_WIDTH {
                close += 1;
            }
        }

        assert!(drawn > 400, "only {drawn} of 500 draws produced a pair");
        assert!(
            close as f64 / drawn as f64 > 0.9,
            "only {close} of {drawn} pairs landed inside a band"
        );
    }

    #[test]
    fn a_uniform_pair_reaches_right_across_the_archive() {
        let pool = spread(400);
        let mut far = 0;

        for seed in 0..500u64 {
            let draw = pool.draw(seed, Pairing::Uniform, &[]).unwrap();
            let one = pool.candidate(draw.left).unwrap().score;
            let two = pool.candidate(draw.right).unwrap().score;
            if (one - two).abs() > 5.0 {
                far += 1;
            }
        }

        assert!(
            far > 50,
            "uniform pairs are what keep the comparison graph connected, and only {far} of 500 \
             crossed the archive"
        );
    }

    #[test]
    fn the_votes_go_to_the_pictures_that_have_had_the_fewest() {
        let mut candidates: Vec<Candidate> = (0..200).map(|at| scored(at, 0.0, 100)).collect();
        candidates.extend((200..210).map(|at| scored(at, 0.0, 0)));
        let pool = Pool::new(candidates);

        let mut thin = 0;
        for seed in 0..1_000u64 {
            let draw = pool.draw(seed, Pairing::Informative, &[]).unwrap();
            for picture in [draw.left, draw.right] {
                if pool.candidate(picture).unwrap().comparisons == 0 {
                    thin += 1;
                }
            }
        }

        assert!(
            thin > 300,
            "the unjudged pictures were picked {thin} times in 2,000, which is barely a preference"
        );
    }

    #[test]
    fn a_band_holding_more_uncertainty_gets_more_of_the_votes() {
        let mut candidates: Vec<Candidate> = (0..50).map(|at| scored(at, 3.0, 500)).collect();
        candidates.extend((50..100).map(|at| scored(at, -3.0, 5)));
        let pool = Pool::new(candidates);

        let mut unsettled = 0;
        for seed in 0..1_000u64 {
            let draw = pool.draw(seed, Pairing::Informative, &[]).unwrap();
            if pool.candidate(draw.left).unwrap().comparisons == 5 {
                unsettled += 1;
            }
        }

        assert!(
            unsettled > 700,
            "only {unsettled} of 1,000 went to the half of the pool that is still unsettled"
        );
    }

    #[test]
    fn a_picture_the_voter_should_not_see_again_yet_is_never_drawn() {
        let pool = spread(60);
        let avoid: Vec<ApodDate> = (0..30).map(day).collect();

        for seed in 0..2_000u64 {
            for pairing in [Pairing::Informative, Pairing::Uniform] {
                if let Some(draw) = pool.draw(seed, pairing, &avoid) {
                    assert!(!avoid.contains(&draw.left), "{draw:?}");
                    assert!(!avoid.contains(&draw.right), "{draw:?}");
                }
            }
        }
    }

    #[test]
    fn a_pair_is_never_a_picture_against_itself() {
        let pool = spread(4);
        for seed in 0..5_000u64 {
            for pairing in [Pairing::Informative, Pairing::Uniform] {
                if let Some(draw) = pool.draw(seed, pairing, &[]) {
                    assert_ne!(draw.left, draw.right);
                }
            }
        }
    }

    #[test]
    fn sides_are_randomised_rather_than_settled_by_the_pair() {
        let pool = spread(40);
        let mut left = 0;
        let mut total = 0;

        for seed in 0..1_000u64 {
            let draw = pool.draw(seed, Pairing::Uniform, &[]).unwrap();
            if draw.left < draw.right {
                left += 1;
            }
            total += 1;
        }

        assert!(
            (400..600).contains(&left),
            "the earlier picture went left {left} times in {total}"
        );
    }

    #[test]
    fn a_repeat_puts_the_voters_own_pair_back_with_the_sides_swapped() {
        let pool = spread(40);
        let (one, two) = (day(3), day(11));

        let draw = pool.repeat(one, two).unwrap();
        assert_eq!(draw.pairing, Pairing::Probe);
        assert_eq!(
            (draw.left, draw.right),
            (two, one),
            "half the probes coming back the original way round would measure nothing"
        );
        assert_eq!(pool.repeat(two, one).unwrap().left, one, "and back again");
    }

    #[test]
    fn a_repeat_of_something_outside_the_pool_is_refused() {
        let pool = spread(10);
        assert!(pool.repeat(day(3), day(999)).is_none());
        assert!(pool.repeat(day(3), day(3)).is_none(), "not against itself");
    }

    #[test]
    fn a_focused_pool_asks_its_questions_of_the_contenders() {
        let mut candidates: Vec<Candidate> = (0..500).map(|at| scored(at, -2.0, 40)).collect();
        candidates.extend((500..550).map(|at| scored(at, 3.0, 40)));
        let pool = Pool::focused(candidates, 50);

        assert_eq!(pool.len(), 550, "the whole archive is still reachable");
        assert_eq!(pool.contenders(), 50);

        let mut inside = 0;
        for seed in 0..1_000u64 {
            let draw = pool.draw(seed, Pairing::Informative, &[]).unwrap();
            if draw.left.days() >= 500 && draw.right.days() >= 500 {
                inside += 1;
            }
        }

        assert!(
            inside > 950,
            "only {inside} of 1,000 informative pairs stayed among the contenders"
        );
    }

    #[test]
    fn a_uniform_draw_still_reaches_past_the_cut() {
        let mut candidates: Vec<Candidate> = (0..500).map(|at| scored(at, -2.0, 40)).collect();
        candidates.extend((500..550).map(|at| scored(at, 3.0, 40)));
        let pool = Pool::focused(candidates, 50);

        let mut outside = 0;
        for seed in 0..1_000u64 {
            let draw = pool.draw(seed, Pairing::Uniform, &[]).unwrap();
            if draw.left.days() < 500 || draw.right.days() < 500 {
                outside += 1;
            }
        }

        assert!(
            outside > 900,
            "the cut has to stay connected to the archive, and only {outside} of 1,000 reached it"
        );
    }

    #[test]
    fn the_cut_is_made_on_the_upper_bound_so_a_thin_record_is_not_dropped_for_being_thin() {
        let settled = scored(0, 1.0, 400);
        let unasked = Candidate::unseen(day(1));
        let poor = scored(2, -1.0, 400);
        let pool = Pool::focused(vec![settled, unasked, poor], 2);

        assert_eq!(pool.contenders(), 2);
        let mut seen = 0;
        for seed in 0..500u64 {
            let draw = pool.draw(seed, Pairing::Informative, &[]).unwrap();
            if draw.holds(day(1)) {
                seen += 1;
            }
        }
        assert!(seen > 0, "a picture nobody has judged cannot be cut for it");
    }

    #[test]
    fn a_cut_below_a_pair_still_leaves_a_pair() {
        let pool = Pool::focused(vec![scored(0, 1.0, 5), scored(1, 0.0, 5)], 0);
        assert_eq!(pool.contenders(), 2);
        assert!(pool.draw(1, Pairing::Informative, &[]).is_some());

        let empty = Pool::focused(Vec::new(), 50);
        assert_eq!(empty.contenders(), 0);
        assert!(empty.draw(1, Pairing::Informative, &[]).is_none());
    }

    #[test]
    fn an_unfocused_pool_holds_everything_in_contention() {
        let pool = spread(200);
        assert_eq!(pool.contenders(), pool.len());
    }

    #[test]
    fn a_pool_too_small_to_pair_hands_out_nothing() {
        assert!(Pool::default().draw(1, Pairing::Uniform, &[]).is_none());
        assert!(
            Pool::new(vec![scored(0, 0.0, 0)])
                .draw(1, Pairing::Uniform, &[])
                .is_none()
        );
    }

    #[test]
    fn a_pool_the_voter_has_exhausted_hands_out_nothing_rather_than_a_barred_pair() {
        let pool = spread(3);
        let avoid: Vec<ApodDate> = (0..3).map(day).collect();
        assert!(pool.draw(1, Pairing::Uniform, &avoid).is_none());
        assert!(pool.draw(1, Pairing::Informative, &avoid).is_none());
    }

    #[test]
    fn a_fresh_pool_with_no_scores_yet_still_deals_pairs() {
        let pool = Pool::new((0..100).map(|at| Candidate::unseen(day(at))).collect());
        assert_eq!(pool.bands(), 1, "everything starts in the same band");

        for seed in 0..200u64 {
            let draw = pool.draw(seed, Pairing::Informative, &[]).unwrap();
            assert_ne!(draw.left, draw.right);
        }
    }

    #[test]
    fn one_picture_listed_twice_is_one_picture() {
        let pool = Pool::new(vec![
            scored(0, 1.0, 5),
            scored(0, 2.0, 9),
            scored(1, 0.0, 3),
        ]);
        assert_eq!(pool.len(), 2);
        assert!(pool.contains(day(0)) && pool.contains(day(1)));
    }

    #[test]
    fn the_same_seed_deals_the_same_pair_and_a_different_one_does_not() {
        let pool = spread(200);
        assert_eq!(
            pool.draw(7, Pairing::Informative, &[]),
            pool.draw(7, Pairing::Informative, &[])
        );
        assert_ne!(
            pool.draw(7, Pairing::Informative, &[]),
            pool.draw(8, Pairing::Informative, &[])
        );
    }
}
