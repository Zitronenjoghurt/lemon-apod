use apod_core::date::ApodDate;
use apod_core::rating::{
    Candidate, Grouping, Outcome, Pairing, Pool, Prior, Progress, Vote, fit, tiers,
};

const POOL: usize = 9_582;
const REFIT_EVERY: u64 = 10_000;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn fraction(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn normal(&mut self) -> f64 {
        let (one, two) = (self.fraction().max(1e-12), self.fraction());
        (-2.0 * one.ln()).sqrt() * (std::f64::consts::TAU * two).cos()
    }
}

fn day(at: usize) -> ApodDate {
    ApodDate::from_days(at as i32)
}

fn logistic(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// What `Rating::refit` builds, without the database in the way.
fn rebuild(truth_len: usize, log: &[Vote], votes: u64) -> Pool {
    let done = fit(log, &Grouping::default(), &Prior::weak());
    let candidates: Vec<Candidate> = (0..truth_len)
        .map(|at| match done.score(day(at)) {
            Some(score) => Candidate {
                picture: day(at),
                score: score.score,
                stderr: score.stderr,
                comparisons: score.comparisons,
            },
            None => Candidate::unseen(day(at)),
        })
        .collect();

    match Progress::of(truth_len as u64, votes).stage.contenders() {
        None => Pool::new(candidates),
        Some(keep) => Pool::focused(candidates, keep as usize),
    }
}

#[test]
#[ignore = "a full funnel takes a moment; run with --ignored"]
fn the_funnel_spends_its_last_stage_on_the_pictures_in_question() {
    let mut rng = Rng(20_260_818);
    let truth: Vec<f64> = (0..POOL).map(|_| rng.normal() * 1.2).collect();

    let mut order: Vec<usize> = (0..POOL).collect();
    order.sort_by(|&one, &two| truth[two].total_cmp(&truth[one]));
    let best: Vec<usize> = order[..100].to_vec();

    let mut pool = Pool::new((0..POOL).map(|at| Candidate::unseen(day(at))).collect());
    let mut log: Vec<Vote> = Vec::new();
    let mut touched = vec![0u32; POOL];
    let budget = Progress::of(POOL as u64, 0).total;

    for cast in 0..budget {
        let seed = rng.next();
        let Some(draw) = pool.draw(seed, Pairing::pick(seed, false), &[]) else {
            continue;
        };

        let (left, right) = (draw.left.days() as usize, draw.right.days() as usize);
        touched[left] += 1;
        touched[right] += 1;

        let outcome = match rng.fraction() < logistic(truth[left] - truth[right]) {
            true => Outcome::Left,
            false => Outcome::Right,
        };
        log.push(Vote::new(draw.left, draw.right, outcome));

        if (cast + 1) % REFIT_EVERY == 0 || cast + 1 == budget {
            pool = rebuild(POOL, &log, cast + 1);
        }
    }

    let done = fit(&log, &Grouping::default(), &Prior::weak());
    let mut board: Vec<_> = done
        .scores
        .iter()
        .filter(|score| score.comparisons >= 5)
        .copied()
        .collect();
    board.sort_by(|one, two| two.lower().total_cmp(&one.lower()));

    let mut ranked = touched.clone();
    ranked.sort_unstable();
    let median = ranked[POOL / 2];
    let deepest: u32 = order[..50].iter().map(|&at| touched[at]).sum::<u32>() / 50;

    println!("top-50 averaged {deepest} comparisons against a median of {median}");
    assert!(
        deepest > median * 10,
        "the last stage is meant to go deep on the contenders, and the top 50 averaged \
         {deepest} against a median of {median}"
    );

    let found: Vec<usize> = board
        .iter()
        .take(100)
        .map(|score| score.picture.days() as usize)
        .collect();
    let recovered = found.iter().filter(|at| best.contains(at)).count();
    println!("top-100 recovered: {recovered}");
    assert!(
        recovered >= 65,
        "only {recovered} of the hundred best pictures reached the top hundred"
    );

    let numbers = tiers(&board);
    let bands = numbers
        .iter()
        .take(100)
        .collect::<std::collections::HashSet<_>>()
        .len();
    println!("distinct tiers in the top 100: {bands}");
    assert!(
        bands >= 4,
        "a top hundred worth reading has to be sorted into more than {bands} bands"
    );

    println!(
        "top row: {} comparisons, stderr {:.3}",
        board[0].comparisons, board[0].stderr
    );
    assert!(
        board[0].stderr < 0.2,
        "the top of the board is still {:.3} wide",
        board[0].stderr
    );
}
