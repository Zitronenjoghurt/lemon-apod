use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct Nonces {
    generations: Mutex<Generations>,
    life: Duration,
}

struct Generations {
    current: HashSet<u64>,
    previous: HashSet<u64>,
    rotated: Instant,
}

impl Nonces {
    pub fn new(life: Duration) -> Self {
        Self {
            generations: Mutex::new(Generations {
                current: HashSet::new(),
                previous: HashSet::new(),
                rotated: Instant::now(),
            }),
            life,
        }
    }

    pub fn claim(&self, nonce: u64) -> bool {
        self.claim_at(nonce, Instant::now())
    }

    fn claim_at(&self, nonce: u64, now: Instant) -> bool {
        let mut held = self.poisoned();

        if now.duration_since(held.rotated) >= self.life {
            held.previous = std::mem::take(&mut held.current);
            held.rotated = now;
        }

        if held.previous.contains(&nonce) {
            return false;
        }

        held.current.insert(nonce)
    }

    pub fn spent(&self) -> usize {
        let held = self.poisoned();
        held.current.len() + held.previous.len()
    }

    fn poisoned(&self) -> std::sync::MutexGuard<'_, Generations> {
        self.generations
            .lock()
            .unwrap_or_else(|held| held.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nonces() -> Nonces {
        Nonces::new(Duration::from_secs(300))
    }

    #[test]
    fn a_nonce_can_be_spent_once() {
        let nonces = nonces();

        assert!(nonces.claim(42));
        assert!(
            !nonces.claim(42),
            "a replayed winning vote is the whole point"
        );
        assert!(nonces.claim(43));
        assert_eq!(nonces.spent(), 2);
    }

    #[test]
    fn a_nonce_spent_within_the_last_lifetime_is_still_refused_across_a_rotation() {
        let nonces = nonces();
        let start = Instant::now();

        assert!(nonces.claim_at(1, start));
        assert!(
            !nonces.claim_at(1, start + Duration::from_secs(301)),
            "one rotation is not enough to forget it"
        );
    }

    #[test]
    fn a_nonce_older_than_two_lifetimes_is_forgotten_and_the_ballot_is_stale_anyway() {
        let nonces = nonces();
        let start = Instant::now();

        assert!(nonces.claim_at(1, start));
        nonces.claim_at(2, start + Duration::from_secs(301));
        nonces.claim_at(3, start + Duration::from_secs(601));

        assert!(
            nonces.claim_at(1, start + Duration::from_secs(602)),
            "the freshness check is what refuses this one, not the nonce set"
        );
    }

    #[test]
    fn rotating_keeps_memory_to_two_lifetimes_rather_than_to_uptime() {
        let nonces = nonces();
        let start = Instant::now();

        for at in 0..1_000u64 {
            nonces.claim_at(at, start);
        }
        assert_eq!(nonces.spent(), 1_000);

        for at in 1_000..1_010u64 {
            nonces.claim_at(at, start + Duration::from_secs(301));
        }
        assert_eq!(nonces.spent(), 1_010, "one generation back is still held");

        nonces.claim_at(9_999, start + Duration::from_secs(601));
        assert_eq!(nonces.spent(), 11, "and the older one has gone");
    }

    #[test]
    fn many_threads_spending_one_nonce_leave_exactly_one_winner() {
        let nonces = std::sync::Arc::new(nonces());
        let winners = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let threads: Vec<_> = (0..16)
            .map(|_| {
                let (nonces, winners) = (nonces.clone(), winners.clone());
                std::thread::spawn(move || {
                    if nonces.claim(7) {
                        winners.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                })
            })
            .collect();

        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(winners.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
