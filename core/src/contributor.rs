use crate::entry::ApodEntry;
use crate::table;
use regex::Regex;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Person,
    Group,
    Unknown,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Group => "group",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    pub id: String,
    pub name: String,
    pub role: String,
    pub url: Option<String>,
    pub kind: Kind,
}

static ANCHOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)<a[^>]*href="([^"]*)"[^>]*>(.*?)</a>"#).unwrap());
static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
static SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static AFFILIATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\([^)]*\)\s*$").unwrap());
static UNIVERSITY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:u|univ|university)(?:\s+of)?\s+").unwrap());
static EMPTY_BRACKET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\([^A-Za-z0-9]*\)").unwrap());

const MIN_IDENTITY_CHARS: usize = 2;
const MIN_PERSONAL_NAME_WORDS: usize = 2;
const MAX_PERSONAL_NAME_WORDS: usize = 4;

struct Words {
    group: Vec<String>,
    prefix: Vec<String>,
    ignore: Vec<String>,
    strip: Vec<String>,
}

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    let mut words = Words {
        group: Vec::new(),
        prefix: Vec::new(),
        ignore: Vec::new(),
        strip: Vec::new(),
    };

    for row in table::rows(include_str!("../tables/credit-words.csv"), "role,word") {
        let Some(word) = row.get(1) else { continue };
        match row.column(0) {
            "group" => words.group.push(word.to_owned()),
            "prefix" => words.prefix.push(word.to_owned()),
            "ignore" => words.ignore.push(word.to_owned()),
            "strip" => words.strip.push(word.to_owned()),
            _ => {}
        }
    }

    words
        .strip
        .sort_by_key(|phrase| std::cmp::Reverse(phrase.len()));
    words
});

struct Judgements {
    merges: Vec<(String, String)>,
    groups: Vec<String>,
}

static JUDGED: LazyLock<Judgements> = LazyLock::new(|| {
    let mut judged = Judgements {
        merges: Vec::new(),
        groups: Vec::new(),
    };

    for row in table::rows(include_str!("../tables/credits.csv"), "name,id,kind") {
        let (Some(name), Some(id)) = (row.get(0), row.get(1)) else {
            continue;
        };
        if name != id {
            judged.merges.push((name.to_owned(), id.to_owned()));
        }
        if row.column(2) == "group" {
            judged.groups.push(id.to_owned());
        }
    }

    judged
});

pub fn mentions(entry: &ApodEntry) -> Vec<Mention> {
    let mut found: Vec<Mention> = Vec::new();

    for credit in &entry.credits {
        let role = SPACE.replace_all(credit.role.trim(), " ").into_owned();

        for (url, name) in parties(credit) {
            let Some(id) = identity(&name) else { continue };

            match found.iter_mut().find(|seen| seen.id == id) {
                Some(seen) => {
                    if seen.url.is_none() {
                        seen.url = url;
                    }
                }
                None => {
                    let kind = kind(&id, &name);
                    found.push(Mention {
                        id,
                        name,
                        role: role.clone(),
                        url,
                        kind,
                    });
                }
            }
        }
    }

    found
}

fn parties(credit: &crate::entry::Credit) -> Vec<(Option<String>, String)> {
    let mut out: Vec<(Option<String>, String)> = Vec::new();

    for capture in ANCHOR.captures_iter(&credit.html) {
        let name = clean(&TAGS.replace_all(&capture[2], ""));
        if name.is_empty() {
            continue;
        }

        let url = capture[1].trim();
        out.push(((!url.is_empty()).then(|| url.to_owned()), name));
    }

    let outside_anchors = ANCHOR.replace_all(&credit.html, " ");
    let unescaped = crate::html::unescape(&TAGS.replace_all(&outside_anchors, " "));

    for piece in unescaped
        .split([',', ';', '&'])
        .flat_map(|piece| piece.split(" and "))
    {
        let name = clean(piece);
        if name.is_empty() || is_affiliation(&name) {
            continue;
        }
        if !out.iter().any(|(_, seen)| seen == &name) {
            out.push((None, name));
        }
    }

    out
}

fn is_affiliation(piece: &str) -> bool {
    piece.starts_with('(')
        || piece.chars().filter(|c| *c == ')').count() > piece.chars().filter(|c| *c == '(').count()
}

const EDGE: [char; 5] = [':', ',', '-', '.', ';'];

/// Longest phrase first, because "image processing by" has to be read before "image by".
fn without_role(name: &str) -> &str {
    let mut rest = name;

    loop {
        let lowered = rest.to_ascii_lowercase();

        if let Some(phrase) = WORDS
            .strip
            .iter()
            .find(|phrase| lowered.starts_with(phrase.as_str()))
        {
            rest = trim_edge(&rest[phrase.len()..]);
            continue;
        }

        if let Some(phrase) = WORDS
            .strip
            .iter()
            .find(|phrase| lowered.ends_with(phrase.as_str()))
        {
            rest = trim_edge(&rest[..rest.len() - phrase.len()]);
            continue;
        }

        let shorter = rest
            .strip_suffix(" by")
            .or_else(|| rest.strip_suffix(" of"))
            .or_else(|| rest.strip_suffix(" from"))
            .or_else(|| rest.strip_suffix(" with"));

        match shorter {
            Some(shorter) => rest = trim_edge(shorter),
            None => return rest,
        }
    }
}

fn trim_edge(piece: &str) -> &str {
    piece.trim_matches(|c: char| c.is_whitespace() || EDGE.contains(&c))
}

fn clean(raw: &str) -> String {
    let unescaped = crate::html::unescape(raw);
    let decoded = EMPTY_BRACKET.replace_all(&unescaped, " ");
    let collapsed = SPACE.replace_all(decoded.trim(), " ");
    let collapsed = without_role(&collapsed);
    let trimmed = collapsed
        .trim_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '/' | ':' | '*' | '+'))
        .trim_end_matches('.');

    let folded = trimmed.to_ascii_lowercase();
    match WORDS
        .ignore
        .iter()
        .any(|word| word == folded.trim_end_matches('.'))
    {
        true => String::new(),
        false => trimmed.to_owned(),
    }
}

pub fn identity(name: &str) -> Option<String> {
    let without_affiliation = AFFILIATION.replace(name, "");
    let folded: String = without_affiliation
        .nfd()
        .filter(|c| !is_combining(*c))
        .filter_map(|c| match c {
            c if c.is_alphanumeric() => Some(c.to_lowercase().next().unwrap_or(c)),
            '\'' => None,
            _ => Some(' '),
        })
        .collect();

    let key = SPACE.replace_all(folded.trim(), " ").into_owned();

    if key.chars().count() < MIN_IDENTITY_CHARS || key.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let key = UNIVERSITY.replace(&key, "univ ").into_owned();

    Some(
        JUDGED
            .merges
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, id)| id.clone())
            .unwrap_or(key),
    )
}

fn is_combining(c: char) -> bool {
    is_combining_mark(c)
}

fn kind(id: &str, name: &str) -> Kind {
    if JUDGED.groups.iter().any(|group| group == id) {
        return Kind::Group;
    }

    let without_affiliation = AFFILIATION.replace(name, "").to_lowercase();
    let dotless = without_affiliation.replace('.', " ");
    if WORDS
        .group
        .iter()
        .any(|word| without_affiliation.contains(word.as_str()))
        || WORDS
            .prefix
            .iter()
            .any(|prefix| dotless.split_whitespace().next() == Some(prefix.as_str()))
    {
        return Kind::Group;
    }

    let words: Vec<&str> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();

    if !words.is_empty() && words.iter().all(|word| is_acronym(word)) {
        return Kind::Group;
    }

    if words
        .iter()
        .any(|word| JUDGED.groups.contains(&word.to_lowercase()))
    {
        return Kind::Group;
    }

    let looks_personal = (MIN_PERSONAL_NAME_WORDS..=MAX_PERSONAL_NAME_WORDS).contains(&words.len())
        && !name.chars().any(|c| c.is_ascii_digit())
        && words.iter().all(|word| {
            is_particle(word)
                || word
                    .chars()
                    .next()
                    .is_some_and(|first| first.is_uppercase() || !first.is_alphabetic())
        })
        && !words.iter().all(|word| is_particle(word))
        && !words.iter().all(|word| is_acronym(word));

    match looks_personal {
        true => Kind::Person,
        false => Kind::Unknown,
    }
}

/// "Davide de Martin" and "P. van Dokkum" are one person each, and a lowercase particle in the
/// middle is the only reason they read as neither a person nor a group.
fn is_particle(word: &str) -> bool {
    const PARTICLES: [&str; 20] = [
        "de", "van", "der", "von", "da", "di", "del", "della", "dello", "la", "le", "du", "dos",
        "den", "ter", "bin", "ibn", "af", "av", "zu",
    ];

    word.chars().next().is_some_and(char::is_lowercase) && PARTICLES.contains(&word)
}

fn is_acronym(word: &str) -> bool {
    if matches!(word, "II" | "III" | "IV" | "V" | "VI") {
        return true;
    }

    let letters = word.chars().filter(|c| c.is_alphabetic()).count();
    letters >= 2
        && word
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(char::is_uppercase)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ApodDate;
    use crate::entry::Credit;
    use crate::media::{Media, MediaKind};

    fn entry(credits: &[(&str, &str)]) -> ApodEntry {
        ApodEntry {
            date: ApodDate::START,
            title: "Title".into(),
            title_raw: None,
            explanation_html: String::new(),
            explanation_text: String::new(),
            credits: credits
                .iter()
                .map(|(role, html)| Credit {
                    role: (*role).to_owned(),
                    html: (*html).to_owned(),
                    text: String::new(),
                })
                .collect(),
            has_copyright: false,
            license_url: None,
            tomorrow_teaser: None,
            keywords: Vec::new(),
            media: Media::new(MediaKind::ImageJpg, None, None),
            extra_media: Vec::new(),
            legacy_media_url: None,
            first_stored_at: None,
            alt: None,
            authors: Vec::new(),
            provenance: crate::entry::Provenance::LegacyOnly,
            source_url: ApodDate::START.source_url(),
            picture: None,
        }
    }

    fn ids(credits: &[(&str, &str)]) -> Vec<String> {
        mentions(&entry(credits))
            .into_iter()
            .map(|found| found.id)
            .collect()
    }

    #[test]
    fn a_linked_credit_keeps_the_name_the_role_and_the_link() {
        let found = mentions(&entry(&[(
            "Image Credit & Copyright",
            r#"<a href="https://www.instagram.com/paulofwild/">Paulo Ferreira</a>"#,
        )]));

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Paulo Ferreira");
        assert_eq!(found[0].id, "paulo ferreira");
        assert_eq!(found[0].role, "Image Credit & Copyright");
        assert_eq!(
            found[0].url.as_deref(),
            Some("https://www.instagram.com/paulofwild/")
        );
        assert_eq!(found[0].kind, Kind::Person);
    }

    #[test]
    fn a_credit_line_of_several_bodies_is_several_credits() {
        let ids = ids(&[(
            "Image Credit",
            r#"<a href="https://www.nasa.gov/">NASA</a>, <a href="https://esahubble.org/">ESA</a>, and the <a href="https://www.stsci.edu/">STScI</a>"#,
        )]);

        assert!(ids.contains(&"nasa".to_owned()), "{ids:?}");
        assert!(ids.contains(&"esa".to_owned()), "{ids:?}");
        assert!(ids.contains(&"stsci".to_owned()), "{ids:?}");
    }

    #[test]
    fn people_named_without_a_link_are_still_credited() {
        let found = mentions(&entry(&[(
            "Acknowledgment",
            "R. Chandar (U. Toledo) &amp; J. Miller (U. Michigan)",
        )]));

        let ids: Vec<&str> = found.iter().map(|f| f.id.as_str()).collect();
        assert!(ids.contains(&"r chandar"), "{ids:?}");
        assert!(ids.contains(&"j miller"), "{ids:?}");
        assert!(
            found.iter().all(|found| found.url.is_none()),
            "nothing may invent a link that was not there"
        );
        assert_eq!(
            found[0].name, "R. Chandar (U. Toledo)",
            "the affiliation is dropped from the identity but not from what the entry printed"
        );
    }

    #[test]
    fn one_party_named_twice_on_an_entry_is_one_credit() {
        let found = mentions(&entry(&[
            (
                "Image Credit",
                r#"<a href="https://www.nasa.gov/">NASA</a>"#,
            ),
            ("Processing", "NASA"),
        ]));

        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0].url.as_deref(),
            Some("https://www.nasa.gov/"),
            "the linked sighting is the one that knows the address"
        );
    }

    #[test]
    fn the_spellings_of_one_institute_land_on_one_identity() {
        for written in [
            "STScI",
            "StScI",
            "stsci",
            "Space Telescope Science Institute",
            "Space Telescope Scientific Institute",
        ] {
            assert_eq!(
                identity(written).as_deref(),
                Some("stsci"),
                "from '{written}'"
            );
        }

        assert_eq!(
            identity("HST").as_deref(),
            Some("hst"),
            "the telescope shares an address with the institute and is still not the institute"
        );
    }

    #[test]
    fn a_typo_in_the_source_does_not_become_a_second_body() {
        assert_eq!(identity("NAS A").as_deref(), Some("nasa"));
        assert_eq!(identity("NASA /").as_deref(), Some("nasa"));
        assert_eq!(identity("JPL-Caltech").as_deref(), Some("jpl"));
        assert_eq!(identity("JPL / Caltech").as_deref(), Some("jpl"));
    }

    #[test]
    fn a_name_written_with_and_without_its_diacritics_is_one_person() {
        assert_eq!(identity("José Mtanous"), identity("Jose Mtanous"));
        assert_eq!(
            identity("Jean-Luc Dauvergne"),
            identity("Jean Luc Dauvergne")
        );
    }

    #[test]
    fn what_is_not_a_name_is_not_filed_as_one() {
        assert_eq!(identity(""), None);
        assert_eq!(identity("  "), None);
        assert_eq!(identity("2024"), None);
        assert_eq!(identity(" - / "), None);
    }

    #[test]
    fn an_escaped_separator_does_not_become_a_credited_party() {
        let ids = ids(&[("Image Credit", "Rolf Geissinger &amp; Bray Falls")]);

        assert_eq!(ids, ["rolf geissinger", "bray falls"], "{ids:?}");
        assert!(
            !ids.iter().any(|id| id == "amp"),
            "splitting the markup rather than the text files the entity as a person: {ids:?}"
        );
    }

    #[test]
    fn a_body_is_told_from_a_person_only_where_the_name_says_so() {
        let guess = |name: &str| kind(&identity(name).unwrap_or_default(), name);

        assert_eq!(guess("NASA"), Kind::Group);
        assert_eq!(guess("ESO"), Kind::Group);
        assert_eq!(guess("Mount Wilson Observatory"), Kind::Group);
        assert_eq!(guess("Hubble Heritage Team"), Kind::Group);
        assert_eq!(guess("Spitzer Space Telescope"), Kind::Group);
        assert_eq!(
            guess("CRESST II"),
            Kind::Group,
            "a pair of acronyms is not a person"
        );
        assert_eq!(guess("UMBC CSST"), Kind::Group);
        assert_eq!(
            guess("JPL-Caltech"),
            Kind::Group,
            "an alias lands on an identity that has been ruled on by hand"
        );

        assert_eq!(
            guess("ESA/Webb"),
            Kind::Group,
            "a body inside a name makes the whole credit a body, not a person called Webb"
        );
        assert_eq!(guess("NASA/JPL"), Kind::Group);

        assert_eq!(guess("Paulo Ferreira"), Kind::Person);
        assert_eq!(guess("R. Chandar"), Kind::Person);
        assert_eq!(guess("Jean-Luc Dauvergne"), Kind::Person);

        assert_eq!(
            guess("Gemini"),
            Kind::Unknown,
            "a single capitalised word could be either, and guessing wrong is worse than waiting"
        );
    }

    #[test]
    fn an_affiliation_left_beside_a_linked_name_is_not_a_second_person() {
        let found = mentions(&entry(&[(
            "Image Credit & Copyright",
            r#"<a href="https://example.com/block">Adam Block</a> (<a href="https://www.as.arizona.edu/">U. Arizona</a>)"#,
        )]));

        let ids: Vec<&str> = found.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["adam block", "univ arizona"], "{ids:?}");
        assert!(
            !ids.contains(&"univ arizona )"),
            "a dangling bracket must not become an identity of its own: {ids:?}"
        );
        assert_eq!(
            found[0].name, "Adam Block",
            "and the bracket the links emptied must not stay stuck to the name"
        );
    }

    #[test]
    fn a_bracket_nobody_linked_stays_with_the_person_it_belongs_to() {
        let found = mentions(&entry(&[("Acknowledgment", "R. Chandar (U. Toledo)")]));

        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].id, "r chandar");
        assert_eq!(found[0].name, "R. Chandar (U. Toledo)");
    }

    #[test]
    fn a_wavelength_is_not_a_photographer() {
        let ids = ids(&[
            ("Credit", "X-Ray"),
            (
                "image",
                r#"<a href="https://www.nasa.gov/">NASA</a>/ <a href="http://chandra.harvard.edu/">CXC</a><br> Optical"#,
            ),
        ]);

        assert_eq!(ids, ["nasa", "cxc"], "{ids:?}");
    }

    #[test]
    fn a_university_written_the_way_apod_writes_one_is_a_body() {
        let guess = |name: &str| kind(&identity(name).unwrap_or_default(), name);

        assert_eq!(guess("U. Arizona"), Kind::Group);
        assert_eq!(guess("U. Toledo"), Kind::Group);
        assert_eq!(guess("Univ. of Hawaii"), Kind::Group);
        assert_eq!(
            identity("U. Arizona"),
            identity("University of Arizona"),
            "one university written three ways is one university"
        );
        assert_eq!(identity("Univ. Arizona").as_deref(), Some("univ arizona"));
        assert_eq!(guess("Johns Hopkins Univ./APL"), Kind::Group);
        assert_eq!(
            guess("Adam Block (U. Arizona)"),
            Kind::Person,
            "a person's affiliation is not what they are"
        );
        assert_eq!(
            guess("R. Chandar"),
            Kind::Person,
            "an initial and a surname still has to read as a person"
        );
    }

    #[test]
    fn what_the_split_leaves_behind_is_not_a_person() {
        for debris in [
            "A", "s", "'s", "T", "i", "by", "at", "et", "al", "Jr", "courtesy",
        ] {
            let ids = ids(&[("Image Credit", debris)]);
            assert!(ids.is_empty(), "'{debris}' should name nobody, got {ids:?}");
        }

        assert_eq!(
            identity("UA").as_deref(),
            Some("ua"),
            "two letters is how APOD abbreviates a university, and that is somebody"
        );
    }

    #[test]
    fn a_body_reached_by_one_address_under_several_names_is_one_body() {
        for written in ["STScI", "St. Sci.", "NASA/STScI"] {
            assert_eq!(
                identity(written).as_deref(),
                Some("stsci"),
                "from '{written}'"
            );
        }
        for written in ["CXO", "Chandra", "Chandra X-ray Observatory", "NASA/CXC"] {
            assert_eq!(
                identity(written).as_deref(),
                Some("cxc"),
                "from '{written}'"
            );
        }
        assert_eq!(
            identity("HST").as_deref(),
            Some("hst"),
            "the telescope has always linked the institute's page and is still not the institute"
        );
        assert_eq!(
            identity("JPL").as_deref(),
            Some("jpl"),
            "and a laboratory that links nasa.gov is not NASA"
        );
    }

    #[test]
    fn an_entry_with_no_credits_yields_none() {
        assert!(ids(&[]).is_empty());
    }

    #[test]
    fn a_role_printed_inside_the_name_does_not_split_one_photographer_into_two() {
        for printed in [
            "David Malin",
            "photograph by David Malin",
            "Photograph by David Malin",
            "Colour photography by David Malin",
        ] {
            assert_eq!(
                identity(&clean(printed)).as_deref(),
                Some("david malin"),
                "'{printed}' has to fold onto the same photographer"
            );
        }
    }

    #[test]
    fn a_credit_that_is_only_a_role_is_not_an_identity() {
        for printed in ["Courtesy of", "courtesy", "Courtesy \" \"", "Crew"] {
            assert_eq!(
                identity(&clean(printed)),
                None,
                "'{printed}' names nobody: {:?}",
                clean(printed)
            );
        }
    }

    #[test]
    fn a_role_printed_after_the_name_folds_the_same_way_as_one_printed_before() {
        for printed in [
            "Arne Henden Image Processed by",
            "Arne Henden - stitched by",
        ] {
            assert_eq!(
                identity(&clean(printed)).as_deref(),
                Some("arne henden"),
                "'{printed}' has to fold onto the same person"
            );
        }
    }

    #[test]
    fn a_lowercase_particle_in_the_middle_of_a_name_still_reads_as_a_person() {
        for printed in [
            "Davide de Martin",
            "P. van Dokkum",
            "T. von Hippel",
            "Richard de Grijs",
        ] {
            let name = clean(printed);
            assert_eq!(
                kind(&identity(&name).unwrap(), &name),
                Kind::Person,
                "'{printed}' is one person"
            );
        }
    }

    #[test]
    fn a_group_named_in_another_language_is_not_mistaken_for_a_person() {
        for printed in [
            "Observatoire de Paris",
            "Observatorio del Teide",
            "Groupe Astronomie",
        ] {
            let name = clean(printed);
            assert_eq!(
                kind(&identity(&name).unwrap(), &name),
                Kind::Group,
                "'{printed}' is a place, and the particle rule must not turn it into somebody"
            );
        }
    }

    #[test]
    fn stripping_a_role_word_never_eats_the_start_of_a_real_name() {
        for printed in [
            "Optical Society",
            "Image Science Laboratory",
            "Data Analysis Group",
        ] {
            assert_eq!(
                clean(printed),
                printed,
                "only the role phrases in credit-words.csv come off, not any word that resembles one"
            );
        }
    }
}
