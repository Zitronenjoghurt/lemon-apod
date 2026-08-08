use apod_core::ApodDate;

const HALF: u32 = 14;
const MASK: u32 = (1 << HALF) - 1;
const ROUNDS: u32 = 4;
const LENGTH: usize = 6;

const ALPHABET: &[u8] = b"0123456789bcdfghjkmnpqrstvwxyz-_";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Picture,
    Round,
}

impl Kind {
    const fn key(self) -> u32 {
        match self {
            Self::Picture => 0x5f2a_1c7d,
            Self::Round => 0x27b9_e3a5,
        }
    }
}

pub fn encode(kind: Kind, date: ApodDate) -> String {
    let value = u32::try_from(date.days()).unwrap_or(0) & (MASK << HALF | MASK);
    render(feistel(value, kind, true))
}

pub fn decode(kind: Kind, token: &str) -> Option<ApodDate> {
    let value = parse(token)?;
    let days = feistel(value, kind, false);
    i32::try_from(days).ok().map(ApodDate::from_days)
}

fn feistel(value: u32, kind: Kind, forward: bool) -> u32 {
    let (mut left, mut right) = (value >> HALF & MASK, value & MASK);

    for step in 0..ROUNDS {
        let round = if forward { step } else { ROUNDS - 1 - step };
        let key = kind.key().wrapping_mul(round + 1) ^ (round << 19);

        if forward {
            (left, right) = (right, left ^ scramble(right, key));
        } else {
            (left, right) = (right ^ scramble(left, key), left);
        }
    }

    left << HALF | right
}

fn scramble(half: u32, key: u32) -> u32 {
    let mixed = (half ^ key).wrapping_mul(0x9e37_79b1);
    (mixed ^ (mixed >> 13)) & MASK
}

fn render(value: u32) -> String {
    (0..LENGTH)
        .rev()
        .map(|position| ALPHABET[(value >> (position * 5) & 0x1f) as usize] as char)
        .collect()
}

fn parse(token: &str) -> Option<u32> {
    let bytes = token.as_bytes();
    if bytes.len() != LENGTH {
        return None;
    }

    let mut value: u32 = 0;
    for &byte in bytes {
        let digit = ALPHABET.iter().position(|&letter| letter == byte)?;
        value = value << 5 | digit as u32;
    }

    (value >> (HALF * 2) == 0).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> ApodDate {
        text.parse().unwrap()
    }

    #[test]
    fn a_token_decodes_back_to_the_entry_it_refers_to() {
        for text in ["1995-06-16", "2019-09-20", "2026-08-08", "2099-12-31"] {
            let token = encode(Kind::Picture, date(text));
            assert_eq!(token.len(), 6, "{token}");
            assert_eq!(decode(Kind::Picture, &token), Some(date(text)), "{text}");
        }
    }

    #[test]
    fn a_token_does_not_spell_out_the_date_it_stands_for() {
        let token = encode(Kind::Picture, date("2019-09-20"));
        for fragment in ["2019", "09", "20", "8862"] {
            assert!(!token.contains(fragment), "{token} leaks {fragment}");
        }
    }

    #[test]
    fn the_same_entry_looks_different_as_a_picture_and_as_a_round() {
        let day = date("2022-07-13");
        assert_ne!(encode(Kind::Picture, day), encode(Kind::Round, day));
        assert_ne!(
            decode(Kind::Round, &encode(Kind::Picture, day)),
            Some(day),
            "a token only means anything to the kind that wrote it"
        );
    }

    #[test]
    fn neighbouring_dates_do_not_produce_neighbouring_tokens() {
        let first = encode(Kind::Picture, date("2020-01-01"));
        let second = encode(Kind::Picture, date("2020-01-02"));

        let shared = first
            .chars()
            .zip(second.chars())
            .filter(|(a, b)| a == b)
            .count();
        assert!(
            shared < 4,
            "{first} and {second} are a day apart and look it"
        );
    }

    #[test]
    fn a_token_that_was_not_one_of_ours_is_refused() {
        assert_eq!(decode(Kind::Picture, ""), None);
        assert_eq!(decode(Kind::Picture, "short"), None);
        assert_eq!(decode(Kind::Picture, "waytoolong"), None);
        assert_eq!(decode(Kind::Picture, "aeiou!"), None, "not in the alphabet");
        assert_eq!(decode(Kind::Picture, "zzzzzz"), None, "out of range");
    }

    #[test]
    fn every_date_in_the_archive_has_its_own_token() {
        let mut seen = std::collections::HashSet::new();
        for days in 0..20_000 {
            let date = ApodDate::from_days(days);
            let token = encode(Kind::Picture, date);
            assert!(seen.insert(token.clone()), "{token} came up twice");
            assert_eq!(decode(Kind::Picture, &token), Some(date));
        }
    }
}
