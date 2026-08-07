use std::collections::BTreeMap;

const ABBREVIATIONS: &[&str] = &[
    "dr", "mr", "mrs", "ms", "st", "prof", "vs", "etc", "no", "fig", "figs", "univ", "inc", "jr",
    "sr", "mt", "approx", "ca", "est", "al",
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextStats {
    pub words: u32,
    pub unique_words: u32,
    pub chars: u32,
    pub sentences: u32,
}

pub fn word_counts(text: &str) -> BTreeMap<String, u32> {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for word in split_words(text).into_iter().filter_map(normalise) {
        *counts.entry(word).or_default() += 1;
    }
    counts
}

pub fn stats(text: &str, counts: &BTreeMap<String, u32>) -> TextStats {
    TextStats {
        words: counts.values().sum(),
        unique_words: counts.len() as u32,
        chars: text.chars().count() as u32,
        sentences: count_sentences(text),
    }
}

fn split_words(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut previous = '\0';
    let mut chars = text.char_indices().peekable();

    while let Some((index, ch)) = chars.next() {
        let next = chars.peek().map_or('\0', |(_, next)| *next);

        let inside = if ch.is_alphanumeric() {
            true
        } else if matches!(ch, '\'' | '\u{2019}' | '-') {
            start.is_some() && next.is_alphanumeric()
        } else if matches!(ch, ',' | '.') {
            previous.is_numeric() && next.is_numeric()
        } else {
            false
        };

        match (inside, start) {
            (true, None) => start = Some(index),
            (false, Some(from)) => {
                out.push(&text[from..index]);
                start = None;
            }
            _ => {}
        }
        previous = ch;
    }

    if let Some(from) = start {
        out.push(&text[from..]);
    }
    out
}

fn normalise(raw: &str) -> Option<String> {
    raw.chars().any(char::is_alphabetic).then(|| {
        raw.chars()
            .map(|ch| if ch == '\u{2019}' { '\'' } else { ch })
            .flat_map(char::to_lowercase)
            .collect()
    })
}

fn count_sentences(text: &str) -> u32 {
    let chars: Vec<char> = text.chars().collect();
    let mut sentences = 0;
    let mut word_start = 0;

    for (index, &ch) in chars.iter().enumerate() {
        if index < word_start {
            continue;
        }
        if ch.is_whitespace() {
            word_start = index + 1;
            continue;
        }
        if !matches!(ch, '.' | '!' | '?') {
            continue;
        }

        let mut after = index + 1;
        while chars.get(after).is_some_and(|next| {
            matches!(next, '.' | '!' | '?' | '"' | '\'' | ')' | ']' | '\u{201d}')
        }) {
            after += 1;
        }

        let ends = match chars.get(after) {
            None => true,
            Some(next) if next.is_whitespace() => chars[after..]
                .iter()
                .find(|next| !next.is_whitespace())
                .is_some_and(|next| !next.is_lowercase()),
            Some(_) => false,
        };

        if ends && !is_abbreviation(&chars[word_start..index]) {
            sentences += 1;
            word_start = after;
        }
    }

    if sentences == 0 && text.chars().any(char::is_alphanumeric) {
        return 1;
    }
    sentences
}

fn is_abbreviation(word: &[char]) -> bool {
    if word.len() == 1 {
        return true;
    }

    let lower: String = word
        .iter()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect();
    ABBREVIATIONS.contains(&lower.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counted(text: &str) -> Vec<(String, u32)> {
        word_counts(text).into_iter().collect()
    }

    #[test]
    fn counts_words_case_folded() {
        assert_eq!(
            counted("The star and the Star."),
            vec![
                ("and".to_owned(), 1),
                ("star".to_owned(), 2),
                ("the".to_owned(), 2)
            ]
        );
    }

    #[test]
    fn keeps_hyphens_and_apostrophes_inside_words() {
        let words: Vec<String> = word_counts("A co-rotating star's light-year")
            .into_keys()
            .collect();
        assert_eq!(words, vec!["a", "co-rotating", "light-year", "star's"]);
    }

    #[test]
    fn a_double_hyphen_is_a_dash_not_a_hyphen() {
        let words: Vec<String> = word_counts("gravity--the real problem--wins")
            .into_keys()
            .collect();
        assert_eq!(words, vec!["gravity", "problem", "real", "the", "wins"]);
    }

    #[test]
    fn a_curly_apostrophe_is_the_same_word_as_a_straight_one() {
        assert_eq!(
            counted("star\u{2019}s star's"),
            vec![("star's".to_owned(), 2)]
        );
    }

    #[test]
    fn a_closing_quote_ends_a_word_rather_than_joining_it() {
        let words: Vec<String> = word_counts("the stars' light").into_keys().collect();
        assert_eq!(words, vec!["light", "stars", "the"]);
    }

    #[test]
    fn a_grouped_number_stays_attached_to_what_it_measures() {
        let words: Vec<String> = word_counts("some 150,000-kilometer jets, 2.5 across")
            .into_keys()
            .collect();
        assert_eq!(words, vec!["150,000-kilometer", "across", "jets", "some"]);
    }

    #[test]
    fn numbers_are_not_words_but_designations_are() {
        let words: Vec<String> = word_counts("M31 is 2.5 million light years away, in 1995")
            .into_keys()
            .collect();
        assert_eq!(
            words,
            vec!["away", "in", "is", "light", "m31", "million", "years"]
        );
    }

    #[test]
    fn totals_come_from_the_same_counts_the_catalogue_stores() {
        let text = "The star is bright. The star is far.";
        let counts = word_counts(text);
        let stats = stats(text, &counts);

        assert_eq!(stats.words, 8);
        assert_eq!(stats.unique_words, 5);
        assert_eq!(stats.chars, text.chars().count() as u32);
        assert_eq!(stats.sentences, 2);
    }

    #[test]
    fn an_abbreviation_or_an_initial_does_not_end_a_sentence() {
        assert_eq!(count_sentences("Dr. J. Smith looked up. Then he left."), 2);
        assert_eq!(count_sentences("Roughly 2.5 million light years away."), 1);
        assert_eq!(count_sentences("It is big, e.g. bigger than Earth."), 1);
    }

    /// Thirty years of hand-written prose contains every way of running punctuation together,
    /// and the counter walks characters, so this is the shape that used to index backwards.
    #[test]
    fn runs_of_punctuation_do_not_walk_off_the_end() {
        assert_eq!(
            count_sentences("It ended.. Then it began?! And stopped..."),
            3
        );
        assert_eq!(count_sentences("...Leading dots. Then more."), 2);
        assert_eq!(count_sentences("Quoted. \"Yes!\" she said. Done."), 3);
    }

    #[test]
    fn unterminated_prose_still_holds_a_sentence() {
        assert_eq!(count_sentences("No full stop here"), 1);
        assert_eq!(count_sentences("   "), 0);
    }

    #[test]
    fn multibyte_text_is_counted_by_character_not_by_byte() {
        let text = "The n\u{e9}bula glows.";
        let counts = word_counts(text);
        assert!(counts.contains_key("n\u{e9}bula"));
        assert_eq!(stats(text, &counts).chars, 17, "17 characters, 18 bytes");
    }
}
