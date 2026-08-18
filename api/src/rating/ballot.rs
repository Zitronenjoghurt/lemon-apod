use apod_core::ApodDate;
use apod_core::rating::store::VoterId;
use apod_core::rating::{Category, Pairing};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

const VERSION: u8 = 1;
const TAG_BYTES: usize = 16;
const BODY_BYTES: usize = 27;
const VOTER_BYTES: usize = 16;

const PROBE: u8 = 1 << 0;
const BOUND: u8 = 1 << 1;

type Tag = Hmac<Sha256>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BallotError {
    #[error("that ballot was not one this server issued")]
    Forged,
    #[error("that ballot has been spent")]
    Spent,
    #[error("that ballot is too old to vote with; here is another")]
    Stale,
    #[error("that ballot belongs to a different voter")]
    Misattributed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ballot {
    pub category: Category,
    pub left: ApodDate,
    pub right: ApodDate,
    pub voter: Option<VoterId>,
    pub probe: bool,
    pub issued_at: DateTime<Utc>,
    pub nonce: u64,
}

impl Ballot {
    pub fn new(
        category: Category,
        left: ApodDate,
        right: ApodDate,
        pairing: Pairing,
        voter: Option<VoterId>,
        issued_at: DateTime<Utc>,
        nonce: u64,
    ) -> Self {
        Self {
            category,
            left,
            right,
            voter,
            probe: pairing == Pairing::Probe,
            issued_at,
            nonce,
        }
    }

    pub fn sign(&self, secret: &[u8]) -> String {
        let body = self.body();
        let mut out = body.clone();
        out.extend_from_slice(&tag(secret, &body));
        URL_SAFE_NO_PAD.encode(out)
    }

    pub fn open(secret: &[u8], token: &str) -> Result<Self, BallotError> {
        let raw = URL_SAFE_NO_PAD
            .decode(token)
            .map_err(|_| BallotError::Forged)?;
        if raw.len() < BODY_BYTES + TAG_BYTES {
            return Err(BallotError::Forged);
        }

        let (body, offered) = raw.split_at(raw.len() - TAG_BYTES);
        let mut mac = Tag::new_from_slice(secret).expect("HMAC takes a key of any length");
        mac.update(body);
        mac.verify_truncated_left(offered)
            .map_err(|_| BallotError::Forged)?;

        read(body).ok_or(BallotError::Forged)
    }

    pub fn fresh(&self, now: DateTime<Utc>, life: TimeDelta) -> bool {
        let age = now - self.issued_at;
        age >= -TimeDelta::minutes(1) && age <= life
    }

    pub fn response(&self, now: DateTime<Utc>) -> TimeDelta {
        now - self.issued_at
    }

    pub fn belongs_to(&self, voter: Option<VoterId>) -> Result<(), BallotError> {
        match (self.voter, voter) {
            (None, _) => Ok(()),
            (Some(theirs), Some(ours)) if theirs == ours => Ok(()),
            _ => Err(BallotError::Misattributed),
        }
    }

    fn body(&self) -> Vec<u8> {
        let mut flags = 0;
        if self.probe {
            flags |= PROBE;
        }
        if self.voter.is_some() {
            flags |= BOUND;
        }

        let mut out = Vec::with_capacity(BODY_BYTES + VOTER_BYTES);
        out.push(VERSION);
        out.push(match self.category {
            Category::Beautiful => 0,
            Category::Fascinating => 1,
        });
        out.push(flags);
        out.extend_from_slice(&self.left.days().to_be_bytes());
        out.extend_from_slice(&self.right.days().to_be_bytes());
        out.extend_from_slice(&self.issued_at.timestamp_millis().to_be_bytes());
        out.extend_from_slice(&self.nonce.to_be_bytes());
        if let Some(voter) = &self.voter {
            out.extend_from_slice(voter.bytes());
        }

        out
    }
}

fn read(body: &[u8]) -> Option<Ballot> {
    if body[0] != VERSION {
        return None;
    }

    let category = match body[1] {
        0 => Category::Beautiful,
        1 => Category::Fascinating,
        _ => return None,
    };

    let flags = body[2];
    let left = i32::from_be_bytes(body[3..7].try_into().ok()?);
    let right = i32::from_be_bytes(body[7..11].try_into().ok()?);
    let issued = i64::from_be_bytes(body[11..19].try_into().ok()?);
    let nonce = u64::from_be_bytes(body[19..27].try_into().ok()?);

    let voter = match flags & BOUND != 0 {
        false => None,
        true => {
            let bytes: [u8; VOTER_BYTES] = body.get(BODY_BYTES..)?.try_into().ok()?;
            Some(VoterId::new(bytes))
        }
    };

    Some(Ballot {
        category,
        left: ApodDate::from_days(left),
        right: ApodDate::from_days(right),
        voter,
        probe: flags & PROBE != 0,
        issued_at: Utc
            .timestamp_millis_opt(issued)
            .single()
            .unwrap_or_default(),
        nonce,
    })
}

fn tag(secret: &[u8], body: &[u8]) -> [u8; TAG_BYTES] {
    let mut mac = Tag::new_from_slice(secret).expect("HMAC takes a key of any length");
    mac.update(body);

    let mut out = [0u8; TAG_BYTES];
    out.copy_from_slice(&mac.finalize().into_bytes()[..TAG_BYTES]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"a deployment secret";

    fn day(n: i32) -> ApodDate {
        ApodDate::from_days(n)
    }

    fn voter(seed: u8) -> VoterId {
        VoterId::new([seed; VOTER_BYTES])
    }

    fn at(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn ballot(voter: Option<VoterId>) -> Ballot {
        Ballot::new(
            Category::Beautiful,
            day(1_000),
            day(9_000),
            Pairing::Informative,
            voter,
            at("2026-08-17T12:00:00Z"),
            0x1234_5678_9abc_def0,
        )
    }

    #[test]
    fn a_ballot_the_server_signed_reads_back_exactly_as_it_was_issued() {
        for held in [None, Some(voter(7))] {
            let issued = ballot(held);
            let token = issued.sign(SECRET);
            assert_eq!(Ballot::open(SECRET, &token).unwrap(), issued);
        }
    }

    #[test]
    fn a_probe_survives_the_round_trip_because_the_log_has_to_know() {
        let mut probe = ballot(Some(voter(1)));
        probe.probe = true;

        let read = Ballot::open(SECRET, &probe.sign(SECRET)).unwrap();
        assert!(read.probe);
        assert!(
            !Ballot::open(SECRET, &ballot(Some(voter(1))).sign(SECRET))
                .unwrap()
                .probe
        );
    }

    #[test]
    fn a_ballot_nobody_signed_is_refused() {
        for token in ["", "nonsense", "!!!!", &"A".repeat(60)] {
            assert_eq!(
                Ballot::open(SECRET, token),
                Err(BallotError::Forged),
                "{token}"
            );
        }
    }

    #[test]
    fn a_ballot_signed_with_another_secret_is_refused() {
        let token = ballot(None).sign(b"someone else's secret");
        assert_eq!(Ballot::open(SECRET, &token), Err(BallotError::Forged));
    }

    #[test]
    fn a_client_cannot_edit_the_pair_it_was_handed() {
        let token = ballot(None).sign(SECRET);
        let mut raw = URL_SAFE_NO_PAD.decode(&token).unwrap();

        for at in 0..BODY_BYTES {
            let mut tampered = raw.clone();
            tampered[at] ^= 0x01;
            assert_eq!(
                Ballot::open(SECRET, &URL_SAFE_NO_PAD.encode(&tampered)),
                Err(BallotError::Forged),
                "byte {at} was not covered"
            );
        }

        raw.truncate(raw.len() - 1);
        assert_eq!(
            Ballot::open(SECRET, &URL_SAFE_NO_PAD.encode(&raw)),
            Err(BallotError::Forged)
        );
    }

    #[test]
    fn a_ballot_does_not_spell_out_the_pair_it_stands_for() {
        let token = ballot(None).sign(SECRET);
        for fragment in ["1000", "9000", "1998", "2020"] {
            assert!(!token.contains(fragment), "{token} leaks {fragment}");
        }
    }

    #[test]
    fn a_ballot_issued_to_one_voter_is_not_another_voters_to_spend() {
        let theirs = ballot(Some(voter(1)));

        assert!(theirs.belongs_to(Some(voter(1))).is_ok());
        assert_eq!(
            theirs.belongs_to(Some(voter(2))),
            Err(BallotError::Misattributed)
        );
        assert_eq!(theirs.belongs_to(None), Err(BallotError::Misattributed));
    }

    #[test]
    fn the_first_ballot_of_a_session_belongs_to_whoever_votes_with_it() {
        let unbound = ballot(None);
        assert!(unbound.belongs_to(None).is_ok());
        assert!(
            unbound.belongs_to(Some(voter(3))).is_ok(),
            "somebody who cleared their cookie mid-session should not be told off"
        );
    }

    #[test]
    fn a_ballot_older_than_its_life_is_stale_and_one_from_the_future_is_not_ours() {
        let issued = ballot(None);
        let life = TimeDelta::minutes(5);

        assert!(issued.fresh(at("2026-08-17T12:00:01Z"), life));
        assert!(issued.fresh(at("2026-08-17T12:04:59Z"), life));
        assert!(!issued.fresh(at("2026-08-17T12:05:01Z"), life));
        assert!(
            issued.fresh(at("2026-08-17T11:59:50Z"), life),
            "a few seconds of clock skew is not an attack"
        );
        assert!(!issued.fresh(at("2026-08-17T11:50:00Z"), life));
    }

    #[test]
    fn the_response_time_is_measured_from_when_the_pair_went_up() {
        let issued = ballot(None);
        assert_eq!(
            issued.response(at("2026-08-17T12:00:03Z")),
            TimeDelta::seconds(3)
        );
        assert!(
            issued.response(at("2026-08-17T12:00:00.200Z")) < TimeDelta::milliseconds(400),
            "a fifth of a second was not a judgment"
        );
    }

    #[test]
    fn a_ballot_is_short_enough_to_sit_in_a_json_body_without_comment() {
        assert!(ballot(None).sign(SECRET).len() < 80);
        assert!(ballot(Some(voter(1))).sign(SECRET).len() < 100);
    }

    #[test]
    fn each_category_signs_to_something_different() {
        let mut other = ballot(None);
        other.category = Category::Fascinating;

        assert_ne!(ballot(None).sign(SECRET), other.sign(SECRET));
        assert_eq!(
            Ballot::open(SECRET, &other.sign(SECRET)).unwrap().category,
            Category::Fascinating
        );
    }

    #[test]
    fn swapping_the_sides_is_a_different_ballot() {
        let one = ballot(None);
        let two = Ballot {
            left: one.right,
            right: one.left,
            ..one
        };

        assert_ne!(one.sign(SECRET), two.sign(SECRET));
    }
}
