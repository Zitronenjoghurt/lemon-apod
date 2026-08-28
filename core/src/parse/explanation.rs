use crate::html::{self, Fragment, Options};
use regex::Regex;
use scraper::{ElementRef, Html};
use std::sync::LazyLock;
use url::Url;

static MARKER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bexplanation\s*:\s*").unwrap());

const STOPS: &[&str] = &[
    "tomorrow's picture",
    "tomorrow’s picture",
    "authors & editors",
    "author:",
    "authors:",
    "we keep an archive file",
    "nasa official:",
    "a service of:",
    "< archive",
];

/// What separates the prose from whatever APOD appended below it on a migrated record. Those
/// additions are open-ended (a lecture, a gallery, a stream, the move announcement itself), so the
/// boundary has to be the paragraph break they all sit behind rather than a list of the ones seen
/// so far.
const PARAGRAPH: usize = 2;

pub(super) fn strip_label(text: &str) -> &str {
    match MARKER.find(text) {
        Some(found) if found.start() == 0 => &text[found.end()..],
        _ => text,
    }
}

/// Read the prose out of a container the caller already found, which is what the modern record
/// needs: its article body is one paragraph inside a page of navigation and footers.
pub(super) fn from_element(container: ElementRef<'_>, base: &Url) -> Option<Fragment> {
    let read = |start_after| {
        html::sanitize(
            container,
            base,
            &Options {
                start_after,
                stop_at: STOPS,
                stop_after_breaks: Some(PARAGRAPH),
            },
        )
        .filter(|fragment| !fragment.is_empty())
    };

    read(Some(&MARKER)).or_else(|| read(None))
}

pub fn parse(doc: &Html, base: &Url) -> Option<Fragment> {
    let options = Options {
        start_after: Some(&MARKER),
        stop_at: STOPS,
        stop_after_breaks: None,
    };

    super::containers(doc, |text| MARKER.is_match(text))
        .into_iter()
        .find_map(|container| {
            html::sanitize(container, base, &options).filter(|fragment| !fragment.is_empty())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://apod.nasa.gov/apod/ap240305.html").unwrap()
    }

    #[test]
    fn extracts_prose_and_keeps_links() {
        let doc = Html::parse_document(
            r#"<body><center><b>Title</b></center>
               <p> <b> Explanation: </b> The <a href="ap990101.html">nebula</a> glows.
               <p> Tomorrow's picture: <a href="ap240306.html">something</a></body>"#,
        );

        let out = parse(&doc, &base()).unwrap();
        assert_eq!(out.text, "The nebula glows.");
        assert_eq!(
            out.html,
            r#"The <a href="https://apod.nasa.gov/apod/ap990101.html">nebula</a> glows."#
        );
    }

    #[test]
    fn handles_the_old_single_table_cell_layout() {
        let doc = Html::parse_document(
            "<body><table><tr><td><b>Explanation:</b> Old prose here. \
             Authors & editors: Someone</td></tr></table></body>",
        );

        assert_eq!(parse(&doc, &base()).unwrap().text, "Old prose here.");
    }

    #[test]
    fn reaches_prose_that_sits_beside_the_marker_rather_than_under_it() {
        let doc = Html::parse_document(
            r#"<body><center><b>Conjunction Haiku</b></center>
               <p> <b> Explanation: </b>
               <center><i>Sister planet stands<br>together with sister stars.</i></center>
               <p> <center><b>Tomorrow's picture: </b>moon with a view</center></body>"#,
        );

        let out = parse(&doc, &base()).unwrap();
        assert_eq!(out.text, "Sister planet stands together with sister stars.");
        assert!(out.html.contains("<br>"), "the line breaks are the poem");
    }

    #[test]
    fn still_prefers_the_tightest_container_that_holds_the_prose() {
        let doc = Html::parse_document(
            r#"<body><center><b>Title</b></center>
               <p><b>Explanation:</b> The real prose.</p>
               <p>Unrelated trailing paragraph.</p></body>"#,
        );
        assert_eq!(parse(&doc, &base()).unwrap().text, "The real prose.");
    }

    #[test]
    fn returns_none_without_a_marker() {
        let doc = Html::parse_document("<body><p>just some text</p></body>");
        assert!(parse(&doc, &base()).is_none());
    }
}
