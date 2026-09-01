use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;

static KEYWORDS: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"meta[name="keywords" i]"#).unwrap());

static TOMORROW: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\btomorrow['’]s\s+picture\s*:\s*").unwrap());

const TEASER_STOPS: &[&str] = &[
    "authors & editors",
    "author:",
    "nasa official:",
    "a service of:",
    "we keep an archive file",
    "<",
    // The 1995 and 1996 pages put their navigation on the same line as the teaser, separated by
    // literal pipes: "Tomorrow's picture: A Venus Landing | Archive | Glossary | About APOD |".
    // A teaser is a picture's name and never contains a pipe, so the first one ends it. This is the
    // whole rule: matching the nav's words instead would need a list that grows with the site.
    "|",
];

const TEASER_MAX_CHARS: usize = 120;

pub fn keywords(doc: &Html) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    for meta in doc.select(&KEYWORDS) {
        let Some(content) = meta.value().attr("content") else {
            continue;
        };

        for keyword in content.split(',') {
            let keyword = keyword.split_whitespace().collect::<Vec<_>>().join(" ");
            if keyword.is_empty() || out.iter().any(|k| k.eq_ignore_ascii_case(&keyword)) {
                continue;
            }
            out.push(keyword);
        }
    }

    out
}

pub fn tomorrow_teaser(doc: &Html) -> Option<String> {
    let container = super::find_container(doc, |text| TOMORROW.is_match(text))?;
    let text = container.text().collect::<String>();
    let found = TOMORROW.find(&text)?;

    let mut rest = &text[found.end()..];
    let haystack = rest.to_ascii_lowercase();
    if let Some(cut) = TEASER_STOPS
        .iter()
        .filter_map(|needle| haystack.find(needle))
        .min()
    {
        rest = &rest[..cut];
    }

    let teaser = rest.split_whitespace().collect::<Vec<_>>().join(" ");
    (!teaser.is_empty() && teaser.chars().count() <= TEASER_MAX_CHARS).then_some(teaser)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_deduplicates_keywords() {
        let doc = Html::parse_document(
            r#"<meta name="keywords" content="nebula, Orion ,  nebula , ,star formation">"#,
        );
        assert_eq!(keywords(&doc), vec!["nebula", "Orion", "star formation"]);
    }

    #[test]
    fn no_keywords_meta_means_no_keywords() {
        assert!(keywords(&Html::parse_document("<body></body>")).is_empty());
    }

    #[test]
    fn reads_the_teaser() {
        let doc = Html::parse_document("<body><p>Tomorrow's picture: open water</p></body>");
        assert_eq!(tomorrow_teaser(&doc).as_deref(), Some("open water"));
    }

    #[test]
    fn stops_the_teaser_before_the_footer() {
        let doc = Html::parse_document(
            "<body><p>Tomorrow's picture: open water Authors & editors: Someone</p></body>",
        );
        assert_eq!(tomorrow_teaser(&doc).as_deref(), Some("open water"));
    }

    #[test]
    fn stops_the_teaser_before_the_page_navigation() {
        let doc = Html::parse_document(
            "<body><p>Tomorrow's picture: A Venus Landing | Archive | Glossary | About APOD |</p></body>",
        );
        assert_eq!(tomorrow_teaser(&doc).as_deref(), Some("A Venus Landing"));
    }

    #[test]
    fn rejects_an_implausibly_long_teaser() {
        let long = "word ".repeat(60);
        let doc = Html::parse_document(&format!("<body><p>Tomorrow's picture: {long}</p></body>"));
        assert!(tomorrow_teaser(&doc).is_none());
    }
}
