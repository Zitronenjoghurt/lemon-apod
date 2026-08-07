use crate::html::collapse;
use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;

static CENTER: LazyLock<Selector> = LazyLock::new(|| Selector::parse("center").unwrap());
static BOLD: LazyLock<Selector> = LazyLock::new(|| Selector::parse("b").unwrap());
static TITLE_TAG: LazyLock<Selector> = LazyLock::new(|| Selector::parse("title").unwrap());

static CREDIT_MARKER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bcredits?\b").unwrap());
static TRAILING_CREDIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s*\bcredits?\b.*$").unwrap());

const BOILERPLATE: &[&str] = &[
    "astronomy picture of the day",
    "astronomy picture of the day archive",
    "discover the cosmos!",
];

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
            let raw = bold.text().collect::<String>();

            if CREDIT_MARKER.is_match(&raw) {
                continue;
            }

            let candidate = clean(&raw);
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
}

fn clean(raw: &str) -> String {
    collapse(&TRAILING_CREDIT.replace(raw, ""))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn collapses_titles_wrapped_across_source_lines() {
        let doc = Html::parse_document(
            "<center>x</center><center><b>A Very\n   Long\n   Title</b></center><center>y</center>",
        );
        assert_eq!(parse(&doc).as_deref(), Some("A Very Long Title"));
    }
}
