use crate::entry::ApodEntry;
use crate::table;
use regex::Regex;
use std::sync::LazyLock;

pub const TITLE_WEIGHT: u32 = 4;
pub const EXPLANATION_WEIGHT: u32 = 2;
pub const KEYWORDS_WEIGHT: u32 = 1;
pub const REPEAT_CAP: u32 = 3;

/// How prominently an entry's own words treat an object. Never a claim about what the picture
/// shows, which nothing in the archive can see.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Basis {
    pub in_title: bool,
    pub in_keywords: bool,
    pub in_explanation: bool,
    pub explanation_hits: u32,
}

impl Basis {
    pub fn score(self) -> u32 {
        u32::from(self.in_title) * TITLE_WEIGHT
            + u32::from(self.in_explanation) * EXPLANATION_WEIGHT
            + u32::from(self.in_keywords) * KEYWORDS_WEIGHT
            + self.explanation_hits.saturating_sub(1).min(REPEAT_CAP)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    pub id: String,
    pub catalog: String,
    pub name: String,
    pub basis: Basis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Title,
    Keywords,
    Explanation,
}

#[derive(Debug, Clone)]
struct Hit {
    id: String,
    catalog: String,
    name: String,
    hits: u32,
}

struct Catalog {
    prefix: String,
    catalog: String,
    format: String,
    max: u32,
}

static CATALOGS: LazyLock<Vec<Catalog>> = LazyLock::new(|| {
    table::rows(
        include_str!("../tables/catalogs.csv"),
        "prefix,catalog,format,max",
    )
    .filter_map(|row| {
        Some(Catalog {
            prefix: row.get(0)?.to_owned(),
            catalog: row.get(1)?.to_owned(),
            format: row.get(2)?.to_owned(),
            max: row.get(3)?.parse().ok()?,
        })
    })
    .collect()
});

static DESIGNATION: LazyLock<Regex> = LazyLock::new(|| {
    let mut longest_first: Vec<&str> = CATALOGS
        .iter()
        .map(|catalog| catalog.prefix.as_str())
        .collect();
    longest_first.sort_by_key(|prefix| std::cmp::Reverse(prefix.len()));

    let alternation = longest_first
        .iter()
        .map(|prefix| match *prefix {
            "sh2" => r"Sh\s?2".to_owned(),
            prefix => regex::escape(prefix),
        })
        .collect::<Vec<_>>()
        .join("|");

    Regex::new(&format!(r"(?i)\b({alternation})[\s-]?(\d{{1,5}})\b")).unwrap()
});

static SUPERNOVA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bSN\s?(\d{4}[A-Za-z]{0,2})\b").unwrap());

static COMET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([CP]/\d{4}\s?[A-Z]\d{0,2})\b").unwrap());

struct ByName {
    phrase: String,
    id: String,
    catalog: String,
}

static BY_NAME_LONGEST_FIRST: LazyLock<Vec<ByName>> = LazyLock::new(|| {
    let mut names: Vec<ByName> =
        table::rows(include_str!("../tables/objects.csv"), "name,id,catalog")
            .filter_map(|row| {
                Some(ByName {
                    phrase: row.get(0)?.to_owned(),
                    id: row.get(1)?.to_owned(),
                    catalog: row.get(2)?.to_owned(),
                })
            })
            .collect();

    names.sort_by_key(|named: &ByName| std::cmp::Reverse(named.phrase.len()));
    names
});

pub fn named(entry: &ApodEntry) -> Vec<Named> {
    let keywords = entry.keywords.join(" ");
    let blocks_strongest_first = [
        (entry.title.as_str(), Field::Title),
        (keywords.as_str(), Field::Keywords),
        (entry.explanation_text.as_str(), Field::Explanation),
    ];

    let mut found: Vec<Named> = Vec::new();

    for (text, field) in blocks_strongest_first {
        for hit in in_text(text) {
            let seen = match found.iter().position(|seen| seen.id == hit.id) {
                Some(at) => &mut found[at],
                None => {
                    found.push(Named {
                        id: hit.id,
                        catalog: hit.catalog,
                        name: hit.name,
                        basis: Basis::default(),
                    });
                    found.last_mut().expect("just pushed")
                }
            };

            match field {
                Field::Title => seen.basis.in_title = true,
                Field::Keywords => seen.basis.in_keywords = true,
                Field::Explanation => {
                    seen.basis.in_explanation = true;
                    seen.basis.explanation_hits = hit.hits;
                }
            }
        }
    }

    found.sort_by(|one, two| {
        two.basis
            .score()
            .cmp(&one.basis.score())
            .then_with(|| one.catalog.cmp(&two.catalog))
            .then_with(|| number(&one.id).cmp(&number(&two.id)))
            .then_with(|| one.id.cmp(&two.id))
    });

    found
}

fn in_text(text: &str) -> Vec<Hit> {
    let mut found: Vec<Hit> = Vec::new();
    let mut claimed: Vec<(usize, usize)> = Vec::new();

    let mut push = |id: String, catalog: &str, name: &str| match found
        .iter_mut()
        .find(|seen: &&mut Hit| seen.id == id)
    {
        Some(seen) => seen.hits += 1,
        None => found.push(Hit {
            id,
            catalog: catalog.to_owned(),
            name: name.to_owned(),
            hits: 1,
        }),
    };

    for capture in DESIGNATION.captures_iter(text) {
        if let Some(whole) = capture.get(0) {
            claimed.push((whole.start(), whole.end()));
        }
        let prefix = capture[1].to_lowercase().replace(char::is_whitespace, "");
        let Some(catalog) = CATALOGS.iter().find(|entry| entry.prefix == prefix) else {
            continue;
        };
        let Ok(number) = capture[2].parse::<u32>() else {
            continue;
        };
        if number == 0 || number > catalog.max {
            continue;
        }

        push(
            catalog.format.replace("{}", &number.to_string()),
            &catalog.catalog,
            &capture[0],
        );
    }

    for (pattern, catalog) in [(&*SUPERNOVA, "supernova"), (&*COMET, "comet")] {
        for capture in pattern.captures_iter(text) {
            if let Some(whole) = capture.get(0) {
                claimed.push((whole.start(), whole.end()));
            }
            let designation = match catalog {
                "supernova" => format!("SN {}", capture[1].to_uppercase()),
                _ => capture[1].to_uppercase(),
            };
            push(designation, catalog, &capture[0]);
        }
    }

    for named in BY_NAME_LONGEST_FIRST.iter() {
        while let Some((start, end)) = word_span(text, &named.phrase, &claimed) {
            claimed.push((start, end));
            push(named.id.clone(), &named.catalog, &text[start..end]);
        }
    }

    found
}

fn word_span(text: &str, phrase: &str, claimed: &[(usize, usize)]) -> Option<(usize, usize)> {
    let (haystack, needle) = (text.as_bytes(), phrase.as_bytes());
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }

    for start in 0..=haystack.len() - needle.len() {
        let end = start + needle.len();
        if !haystack[start..end].eq_ignore_ascii_case(needle) {
            continue;
        }

        let before_is_word = start > 0 && is_word_byte(haystack[start - 1]);
        let after_is_word = end < haystack.len() && is_word_byte(haystack[end]);
        if before_is_word || after_is_word {
            continue;
        }
        if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
            continue;
        }

        if claimed.iter().any(|(from, to)| start < *to && end > *from) {
            continue;
        }

        return Some((start, end));
    }

    None
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte >= 0x80
}

fn number(id: &str) -> u32 {
    id.chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ApodDate;
    use crate::media::{Media, MediaKind};

    fn entry(title: &str, keywords: &[&str], explanation: &str) -> ApodEntry {
        ApodEntry {
            date: ApodDate::START,
            title: title.into(),
            title_raw: None,
            explanation_html: String::new(),
            explanation_text: explanation.into(),
            credits: Vec::new(),
            has_copyright: false,
            license_url: None,
            tomorrow_teaser: None,
            keywords: keywords.iter().map(|word| (*word).to_owned()).collect(),
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

    fn ids(title: &str) -> Vec<String> {
        named(&entry(title, &[], ""))
            .into_iter()
            .map(|found| found.id)
            .collect()
    }

    #[test]
    fn every_way_apod_writes_a_messier_number_lands_on_one_object() {
        for written in [
            "M31",
            "M 31",
            "M-31",
            "Messier 31",
            "messier 31",
            "MESSIER 31",
        ] {
            assert_eq!(ids(written), ["M31"], "from '{written}'");
        }
    }

    #[test]
    fn the_spelling_the_entry_used_is_kept_beside_the_one_the_archive_uses() {
        let found = named(&entry("Messier 51: The Whirlpool Galaxy", &[], ""));
        assert_eq!(
            found.len(),
            1,
            "one galaxy named twice is one row: {found:?}"
        );
        assert_eq!(found[0].id, "M51");
        assert_eq!(
            found[0].name, "Messier 51",
            "an entry that spelled it out is quotable, and the archive is not the source"
        );
    }

    #[test]
    fn a_number_outside_its_catalogue_is_not_a_designation() {
        assert!(
            ids("A Sky Full of Stars in M 2024").is_empty(),
            "Messier stops at 110, so a year is not one of his"
        );
        assert!(
            ids("M110 in Andromeda").contains(&"M110".to_owned()),
            "and 110 is one of his"
        );
        assert!(ids("Caldwell 400").is_empty(), "Caldwell stops at 109");
        assert!(ids("Trumpler 90").is_empty(), "Trumpler stops at 37");
        assert!(ids("M0 and NGC 0").is_empty(), "nothing is numbered zero");
    }

    #[test]
    fn the_catalogues_apod_reaches_for_are_all_read() {
        let found = named(&entry(
            "NGC 7000, IC 1396, Sh2-155, Abell 1689, Caldwell 14, Barnard 33, Arp 273, \
             LDN 1622, vdB 152, Melotte 15, Collinder 399, Hickson 44, Terzan 5, Palomar 6, \
             UGC 2885, PGC 54559, RCW 86, Gum 29, Simeis 147 and HH 24",
            &[],
            "",
        ));
        let ids: Vec<&str> = found.iter().map(|f| f.id.as_str()).collect();

        for wanted in [
            "NGC 7000",
            "IC 1396",
            "Sh2-155",
            "Abell 1689",
            "Caldwell 14",
            "Barnard 33",
            "Arp 273",
            "LDN 1622",
            "vdB 152",
            "Melotte 15",
            "Collinder 399",
            "Hickson 44",
            "Terzan 5",
            "Palomar 6",
            "UGC 2885",
            "PGC 54559",
            "RCW 86",
            "Gum 29",
            "Simeis 147",
            "HH 24",
        ] {
            assert!(ids.contains(&wanted), "{wanted} is missing from {ids:?}");
        }
    }

    #[test]
    fn a_supernova_and_a_comet_designation_are_read_in_their_own_shapes() {
        let both = ids("SN 1987A in the Large Magellanic Cloud");
        assert!(both.contains(&"SN 1987A".to_owned()), "{both:?}");
        assert!(
            both.contains(&"Large Magellanic Cloud".to_owned()),
            "{both:?}"
        );
        assert!(ids("Comet C/2020 F3 NEOWISE").contains(&"C/2020 F3".to_owned()));
    }

    #[test]
    fn the_bodies_of_the_solar_system_are_the_largest_part_of_the_archive() {
        assert_eq!(ids("Sunset on Mars"), ["Mars"]);
        assert_eq!(ids("Jupiter and Io"), ["Io", "Jupiter"]);
        assert_eq!(ids("Saturn's Rings from Titan"), ["Saturn", "Titan"]);
        assert_eq!(
            ids("The Sun and the Moon"),
            ["Moon", "Sun"],
            "the two most photographed things in the archive"
        );
    }

    #[test]
    fn a_plural_moon_is_not_our_moon() {
        assert!(
            !ids("The Moons of Saturn").contains(&"Moon".to_owned()),
            "the moons of another planet are not Earth's"
        );
        assert_eq!(ids("A Full Moon"), ["Moon"]);
    }

    #[test]
    fn a_body_named_inside_a_longer_word_is_not_named_at_all() {
        for title in ["Sunspots and Sunsets", "Marsquake", "Earthshine Rising"] {
            assert!(ids(title).is_empty(), "'{title}' should name nothing");
        }
    }

    #[test]
    fn the_milky_way_and_the_clouds_beside_it_are_objects_the_archive_knows() {
        assert_eq!(ids("The Milky Way over Chile"), ["Milky Way"]);
        assert_eq!(
            ids("The Large Magellanic Cloud"),
            ["Large Magellanic Cloud"]
        );
        assert_eq!(ids("Star Formation in the LMC"), ["LMC"]);
    }

    #[test]
    fn a_common_name_the_archive_has_ruled_on_lands_on_its_designation() {
        for (written, wanted) in [
            ("The Orion Nebula", "M42"),
            ("The Andromeda Galaxy", "M31"),
            ("The Pleiades", "M45"),
            ("The Horsehead Nebula", "Barnard 33"),
            ("The Helix Nebula", "NGC 7293"),
            ("The Elephant's Trunk Nebula", "IC 1396"),
        ] {
            assert_eq!(ids(written), [wanted], "from '{written}'");
        }
    }

    #[test]
    fn an_entry_naming_an_object_twice_records_it_once_with_what_it_wrote() {
        let found = named(&entry("M42: The Orion Nebula", &[], ""));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].id, "M42");
        assert_eq!(
            found[0].name, "M42",
            "the designation is read first, so that is the spelling recorded"
        );
    }

    #[test]
    fn a_name_with_two_accepted_meanings_is_left_alone() {
        let found = ids("The Hercules Cluster");

        assert!(
            !found.contains(&"M13".to_owned()) && !found.contains(&"Abell 2151".to_owned()),
            "M13 to one reader and Abell 2151 to another, so the archive names neither: {found:?}"
        );
        assert_eq!(
            found,
            ["Hercules"],
            "the constellation it sits in is true whichever cluster was meant"
        );
    }

    #[test]
    fn a_longer_name_claims_its_words_before_a_shorter_one_inside_it_looks() {
        assert_eq!(
            ids("The Orion Nebula"),
            ["M42"],
            "five letters cannot be a nebula and the constellation around it at once"
        );
        assert_eq!(
            ids("Orion over Iceland"),
            ["Orion"],
            "bare, it is the constellation"
        );
        assert_eq!(
            ids("The Orion Nebula, deep in Orion"),
            ["Orion", "M42"],
            "and naming the constellation separately still counts"
        );

        assert_eq!(
            ids("The Large Magellanic Cloud"),
            ["Large Magellanic Cloud"],
            "not the Small one's name hiding inside the Large one's"
        );
    }

    #[test]
    fn the_things_a_person_photographs_that_have_no_catalogue_number() {
        assert_eq!(ids("The Perseids over Norway"), ["Perseids"]);
        assert_eq!(
            ids("A Perseid Meteor"),
            ["Perseids"],
            "one of them is the shower"
        );
        assert_eq!(ids("Betelgeuse Imagined"), ["Betelgeuse"]);
        assert_eq!(
            ids("The International Space Station Transits the Sun"),
            ["International Space Station", "Sun"]
        );
        assert_eq!(
            ids("ISS and the Moon"),
            ["International Space Station", "Moon"]
        );
        assert_eq!(ids("Toward the Galactic Center"), ["Galactic Centre"]);
    }

    #[test]
    fn what_an_entry_leads_with_outranks_what_it_mentions_in_passing() {
        let found = named(&entry(
            "M81: A Grand Spiral",
            &["M81"],
            "Unlike M31, which is far larger, this one fits the field.",
        ));

        let subject = found.iter().find(|f| f.id == "M81").expect("in the title");
        assert!(subject.basis.in_title);
        assert!(subject.basis.in_keywords);
        assert_eq!(subject.basis.score(), TITLE_WEIGHT + KEYWORDS_WEIGHT);

        let aside = found.iter().find(|f| f.id == "M31").expect("in the body");
        assert!(!aside.basis.in_title);
        assert_eq!(aside.basis.score(), EXPLANATION_WEIGHT);
        assert!(
            aside.basis.score() < subject.basis.score(),
            "an entry explaining M81 that reaches for M31 as a comparison is about M81"
        );
        assert_eq!(found[0].id, "M81", "and the list leads with it: {found:?}");
    }

    #[test]
    fn every_place_an_object_is_named_adds_to_its_score_and_repeats_are_capped() {
        let found = named(&entry(
            "M13: The Great Globular Cluster",
            &["M13", "globular cluster"],
            "M13 sits in Hercules. M13 is a fine sight. M13 rewards a long look. M13 again.",
        ));

        let m13 = found.iter().find(|one| one.id == "M13").expect("named");
        assert_eq!(
            found.iter().filter(|one| one.id == "M13").count(),
            1,
            "one object named many times is one row: {found:?}"
        );
        assert_eq!(m13.basis.explanation_hits, 4);
        assert_eq!(
            m13.basis.score(),
            TITLE_WEIGHT + KEYWORDS_WEIGHT + EXPLANATION_WEIGHT + REPEAT_CAP,
            "four in the body earns the cap and no more"
        );
    }

    #[test]
    fn a_tag_nothing_else_supports_scores_lowest_of_all() {
        let found = named(&entry("A Grand Design Spiral", &["M74"], ""));
        assert_eq!(found[0].id, "M74");
        assert_eq!(
            found[0].basis.score(),
            KEYWORDS_WEIGHT,
            "the weakest signal the archive has, and the one NASA gets wrong most often"
        );
        assert!(found[0].basis.score() < EXPLANATION_WEIGHT);
    }

    #[test]
    fn a_designation_inside_a_longer_token_is_not_one() {
        for text in ["IMAX3000", "NGCX 42", "SM31", "M31X"] {
            assert!(ids(text).is_empty(), "'{text}' should name nothing");
        }
    }

    #[test]
    fn a_bare_letter_and_a_number_is_left_alone_rather_than_guessed_at() {
        assert!(
            ids("B 33 and C 14 in the dark").is_empty(),
            "Barnard and Caldwell have to be written out, or half the archive becomes an object"
        );
    }

    #[test]
    fn a_leading_zero_does_not_make_a_second_object() {
        let found = named(&entry("NGC 0224 and NGC 224", &[], ""));
        assert_eq!(found.len(), 1, "one galaxy written two ways: {found:?}");
        assert_eq!(found[0].id, "NGC 224");
    }

    #[test]
    fn objects_come_out_in_an_order_a_person_would_have_put_them_in() {
        let found = named(&entry("M10, M9, M100 and NGC 1", &[], ""));
        let ids: Vec<&str> = found.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["M9", "M10", "M100", "NGC 1"]);
    }

    #[test]
    fn an_entry_that_names_nothing_yields_nothing() {
        assert!(ids("Aurora over Norway").is_empty());
        assert!(named(&entry("A Nebula", &["nebula"], "The sky glowed.")).is_empty());
    }
}
