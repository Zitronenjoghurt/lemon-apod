use crate::html::{self, Fragment, Options};
use regex::Regex;
use scraper::Html;
use std::sync::LazyLock;
use url::Url;

static MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bcredits?\b[^:\n]{0,40}:\s*").unwrap());

const STOPS: &[&str] = &[
    "explanation:",
    "tomorrow's picture",
    "tomorrow’s picture",
    "authors & editors",
    "nasa official:",
    "a service of:",
];

pub struct Credit {
    pub fragment: Fragment,
    pub has_copyright: bool,
}

pub fn parse(doc: &Html, base: &Url) -> Option<Credit> {
    let container = super::find_container(doc, &MARKER)?;

    let label = MARKER
        .find(&container.text().collect::<String>())?
        .as_str()
        .to_owned();
    let has_copyright = label.to_ascii_lowercase().contains("copyright");

    let fragment = html::sanitize(
        container,
        base,
        &Options {
            start_after: Some(&MARKER),
            stop_at: STOPS,
        },
    )?;

    (!fragment.is_empty()).then_some(Credit {
        fragment,
        has_copyright,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://apod.nasa.gov/apod/ap240305.html").unwrap()
    }

    #[test]
    fn reads_the_credit_and_notices_copyright() {
        let doc = Html::parse_document(
            r#"<body><center><b>Title</b><br>
               <b>Image Credit &amp; Copyright:</b>
               <a href="http://example.com/">Jane Doe</a></center>
               <p><b>Explanation:</b> prose</p></body>"#,
        );

        let credit = parse(&doc, &base()).unwrap();
        assert_eq!(credit.fragment.text, "Jane Doe");
        assert_eq!(
            credit.fragment.html,
            r#"<a href="http://example.com/">Jane Doe</a>"#
        );
        assert!(credit.has_copyright);
    }

    #[test]
    fn a_plain_credit_is_not_copyrighted() {
        let doc = Html::parse_document(
            "<body><center><b>Image Credit:</b> NASA, ESA, Hubble</center></body>",
        );

        let credit = parse(&doc, &base()).unwrap();
        assert_eq!(credit.fragment.text, "NASA, ESA, Hubble");
        assert!(!credit.has_copyright);
    }

    #[test]
    fn handles_video_and_illustration_labels() {
        for label in ["Video Credit", "Illustration Credit", "Credits"] {
            let doc = Html::parse_document(&format!(
                "<body><center><b>{label}:</b> Someone</center></body>"
            ));
            assert_eq!(parse(&doc, &base()).unwrap().fragment.text, "Someone");
        }
    }

    #[test]
    fn returns_none_when_there_is_no_credit() {
        let doc = Html::parse_document("<body><p>nothing here</p></body>");
        assert!(parse(&doc, &base()).is_none());
    }
}
