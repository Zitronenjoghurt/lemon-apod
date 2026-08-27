use crate::date::ApodDate;
use crate::html::collapse;
use chrono::NaiveDate;
use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;

static CENTER: LazyLock<Selector> = LazyLock::new(|| Selector::parse("center").unwrap());
static BOLD: LazyLock<Selector> = LazyLock::new(|| Selector::parse("b").unwrap());
static TITLE_TAG: LazyLock<Selector> = LazyLock::new(|| Selector::parse("title").unwrap());

static TRAILING_CREDIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s*\bcredits?\b.*$").unwrap());

static TEASER_LABEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^tomorrow['\u{2019}]s\s+picture\b").unwrap());

static MODERN_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{4})\s+([A-Za-z]+)(?:\s+(\d{1,2}))?\s*[\u{2013}\u{2014}-]\s*").unwrap()
});

const BOILERPLATE: &[&str] = &[
    "astronomy picture of the day",
    "astronomy picture of the day archive",
    "discover the cosmos!",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModernTitle {
    pub date: Option<ApodDate>,
    pub title: String,
}

pub fn modern(rendered: &str) -> ModernTitle {
    let decoded = collapse(
        &Html::parse_fragment(rendered)
            .root_element()
            .text()
            .collect::<String>(),
    );
    let body = decoded
        .strip_prefix("APOD:")
        .map(str::trim_start)
        .unwrap_or(&decoded);

    let Some(prefix) = MODERN_PREFIX.captures(body) else {
        return ModernTitle {
            date: None,
            title: body.to_owned(),
        };
    };

    let date = prefix.get(3).and_then(|day| {
        NaiveDate::parse_from_str(
            &format!("{} {} {}", &prefix[1], &prefix[2], day.as_str()),
            "%Y %B %d",
        )
        .ok()
        .map(ApodDate::from)
    });

    ModernTitle {
        date,
        title: body[prefix.get(0).unwrap().end()..].to_owned(),
    }
}

pub fn parse(doc: &Html) -> Option<String> {
    from_center(doc)
        .or_else(|| from_title_tag(doc))
        .filter(|title| !title.is_empty())
}

pub fn raw_title(doc: &Html) -> Option<String> {
    let raw = doc.select(&TITLE_TAG).next()?.text().collect::<String>();
    let trimmed = collapse(&raw);
    (!trimmed.is_empty()).then_some(trimmed)
}

fn from_center(doc: &Html) -> Option<String> {
    let centers: Vec<_> = doc.select(&CENTER).collect();
    if centers.is_empty() {
        return None;
    }

    let preferred = if centers.len() == 2 { 0 } else { 1 };

    for index in std::iter::once(preferred).chain(0..centers.len()) {
        let Some(center) = centers.get(index) else {
            continue;
        };

        for bold in center.select(&BOLD) {
            let candidate = clean(&bold.text().collect::<String>());
            if is_plausible(&candidate) {
                return Some(candidate);
            }
        }
    }

    None
}

fn from_title_tag(doc: &Html) -> Option<String> {
    let raw = raw_title(doc)?;
    let candidate = clean(raw.rsplit(" - ").next().unwrap_or(&raw));
    is_plausible(&candidate).then_some(candidate)
}

fn is_plausible(candidate: &str) -> bool {
    !candidate.is_empty()
        && candidate.chars().count() < 300
        && !BOILERPLATE.contains(&candidate.to_ascii_lowercase().as_str())
        && !TEASER_LABEL.is_match(candidate)
        && !super::credit::is_all_role_words(candidate)
}

fn clean(raw: &str) -> String {
    collapse(&TRAILING_CREDIT.replace(raw, ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modern_date(rendered: &str) -> Option<String> {
        modern(rendered).date.map(|date| date.to_string())
    }

    #[test]
    fn reads_the_date_and_the_title_out_of_a_modern_title() {
        let parsed =
            modern("APOD: 2026 August 26 &#8211; JWST Images The Lion&#8217;s Head Nebula");
        assert_eq!(
            parsed.date.map(|d| d.to_string()).as_deref(),
            Some("2026-08-26")
        );
        assert_eq!(parsed.title, "JWST Images The Lion\u{2019}s Head Nebula");
    }

    #[test]
    fn a_hyphen_separates_as_well_as_an_en_dash() {
        let parsed = modern("APOD: 2007 February 17 - Supernova Remnant and Shock Wave");
        assert_eq!(
            parsed.date.map(|d| d.to_string()).as_deref(),
            Some("2007-02-17")
        );
        assert_eq!(parsed.title, "Supernova Remnant and Shock Wave");
    }

    #[test]
    fn a_prefix_missing_its_day_still_yields_the_title() {
        let parsed = modern("APOD: 2026 April &#8211; Caught in the Web");
        assert_eq!(
            parsed.date, None,
            "a date the source never wrote is not one to invent"
        );
        assert_eq!(parsed.title, "Caught in the Web");
    }

    #[test]
    fn a_title_that_was_never_prefixed_is_left_whole() {
        let parsed = modern("Albert Einstein: 1879 - 1955");
        assert_eq!(parsed.date, None);
        assert_eq!(
            parsed.title, "Albert Einstein: 1879 - 1955",
            "the oldest entries predate the prefix and their titles are not date ranges"
        );
    }

    #[test]
    fn the_title_keeps_the_punctuation_that_follows_the_separator() {
        assert_eq!(
            modern("APOD: 2007 July 16 - Manhattanhenge: A New York Sunset").title,
            "Manhattanhenge: A New York Sunset"
        );
        assert_eq!(
            modern_date("APOD: 2007 July 16 - Manhattanhenge: A New York Sunset").as_deref(),
            Some("2007-07-16")
        );
    }

    #[test]
    fn a_month_or_day_that_is_not_one_yields_no_date() {
        assert_eq!(modern_date("APOD: 2026 Smarch 3 - Nowhere"), None);
        assert_eq!(modern_date("APOD: 2026 February 31 - Nowhere"), None);
    }

    #[test]
    fn takes_the_title_from_the_second_center_block() {
        let doc = Html::parse_document(
            r#"<center><h1>Astronomy Picture of the Day</h1></center>
               <center><b> The Great Nebula </b><br>
               <b>Image Credit &amp; Copyright:</b> Someone</center>
               <center><a href="ap240304.html">&lt;</a></center>"#,
        );
        assert_eq!(parse(&doc).as_deref(), Some("The Great Nebula"));
    }

    #[test]
    fn skips_the_credit_bold_rather_than_mistaking_it_for_a_title() {
        let doc = Html::parse_document(
            r#"<center><b>Image Credit &amp; Copyright:</b> Someone<br>
               <b>Actual Title</b></center>
               <center>nav</center><center>nav</center>"#,
        );
        assert_eq!(parse(&doc).as_deref(), Some("Actual Title"));
    }

    #[test]
    fn falls_back_to_the_title_tag() {
        let doc =
            Html::parse_document("<title>APOD: 2024 March 5 - Orion Rising</title><body></body>");
        assert_eq!(parse(&doc).as_deref(), Some("Orion Rising"));
    }

    #[test]
    fn rejects_the_site_banner() {
        let doc = Html::parse_document(
            "<center><b>Astronomy Picture of the Day</b></center><center><b>Real Title</b></center>",
        );
        assert_eq!(parse(&doc).as_deref(), Some("Real Title"));
    }

    #[test]
    fn recovers_a_title_from_a_bold_that_never_closed_before_its_credit() {
        let doc = Html::parse_document(
            r#"<center>banner</center>
               <center>September 16, 1996 <br>
               <b>The Sun Erupts <br>
               <b>Credit:</b></b> <a href="http://www.nasa.gov/">NASA</a></center>
               <center><b>Tomorrow's picture: </b>Comet Hale-Bopp Fades</center>
               <center>nav</center>"#,
        );
        assert_eq!(parse(&doc).as_deref(), Some("The Sun Erupts"));
    }

    #[test]
    fn never_mistakes_the_next_days_teaser_for_a_title() {
        let doc = Html::parse_document(
            r#"<title>APOD: October 3, 1996 - Barnard's Loop</title>
               <center>banner</center><center>no bold here</center>
               <center><b>Tomorrow's picture: </b>Something Else</center>"#,
        );
        assert_eq!(
            parse(&doc).as_deref(),
            Some("Barnard's Loop"),
            "the teaser label should be skipped in favour of the title tag"
        );
    }

    #[test]
    fn a_bold_that_is_only_a_label_is_never_a_title() {
        for label in [
            "Image Credit & Copyright:",
            "Video Credit:",
            "Credit:",
            "Image and Video Credit & Copyright:",
        ] {
            let doc = Html::parse_document(&format!(
                "<center>x</center><center><b>{label}</b> Someone</center><center>y</center>"
            ));
            assert_eq!(parse(&doc), None, "{label} is a label, not a title");
        }
    }

    #[test]
    fn collapses_titles_wrapped_across_source_lines() {
        let doc = Html::parse_document(
            "<center>x</center><center><b>A Very\n   Long\n   Title</b></center><center>y</center>",
        );
        assert_eq!(parse(&doc).as_deref(), Some("A Very Long Title"));
    }
}
