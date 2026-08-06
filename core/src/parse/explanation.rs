use crate::html::{self, Fragment, Options};
use regex::Regex;
use scraper::Html;
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

pub fn parse(doc: &Html, base: &Url) -> Option<Fragment> {
    let container = super::find_container(doc, |text| MARKER.is_match(text))?;
    let fragment = html::sanitize(
        container,
        base,
        &Options {
            start_after: Some(&MARKER),
            stop_at: STOPS,
        },
    )?;

    (!fragment.is_empty()).then_some(fragment)
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
    fn returns_none_without_a_marker() {
        let doc = Html::parse_document("<body><p>just some text</p></body>");
        assert!(parse(&doc, &base()).is_none());
    }
}
