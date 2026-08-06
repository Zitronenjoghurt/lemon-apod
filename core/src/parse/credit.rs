use crate::entry::Credit;
use crate::html;
use regex::Regex;
use scraper::Html;
use std::ops::Range;
use std::sync::LazyLock;
use url::Url;

/// The words APOD builds its attribution labels from. A label is a run of these ending in a
/// colon, which is what makes `Image Credit & Copyright:` and `Music:` the same shape.
///
/// This is a vocabulary rather than a pattern on purpose: thirty years of hand-written pages
/// invent new roles occasionally, and `apod-archiver quality` surfaces them one at a time.
const ROLE_WORDS: &[&str] = &[
    "acknowledgement",
    "acknowledgment",
    "animation",
    "audio",
    "capture",
    "composition",
    "compositing",
    "copyright",
    "courtesy",
    "credit",
    "credits",
    "data",
    "design",
    "digital",
    "editing",
    "illustration",
    "image",
    "images",
    "licence",
    "license",
    "montage",
    "mosaic",
    "music",
    "narration",
    "photo",
    "photograph",
    "photography",
    "processing",
    "production",
    "simulation",
    "sonification",
    "sound",
    "text",
    "translation",
    "video",
    "visualisation",
    "visualization",
    "writing",
];

/// Words that make a label open the credit block rather than continue it.
const CREDIT_WORDS: &[&str] = &["credit", "credits", "copyright", "courtesy"];

static LABEL: LazyLock<Regex> = LazyLock::new(|| {
    let words = ROLE_WORDS.join("|");
    Regex::new(&format!(
        r"(?i)\b(?:{words})(?:[\s&/,-]+(?:and[\s]+)?(?:{words})\b){{0,3}}\s*:\s*"
    ))
    .expect("the role vocabulary builds a valid pattern")
});

static LICENSE_HREF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)href="(https?://(?:[^"]*\.)?(?:creativecommons\.org|opensource\.org)/[^"]*)""#,
    )
    .expect("static pattern is valid")
});

const STOPS: &[&str] = &[
    "explanation:",
    "tomorrow's picture",
    "tomorrow\u{2019}s picture",
    "authors & editors",
    "nasa official:",
    "a service of:",
];

pub struct Credits {
    pub segments: Vec<Credit>,
    /// The label claims copyright, so the media is not NASA public domain.
    pub has_copyright: bool,
    /// Where a label linked its licence, for the handful credited `Image Credit & License:`.
    pub license_url: Option<String>,
}

struct Label {
    /// Covers the label and the whitespace after its colon.
    range: Range<usize>,
    role: String,
}

impl Label {
    fn is_credit(&self) -> bool {
        let role = self.role.to_ascii_lowercase();
        CREDIT_WORDS.iter().any(|word| {
            role.split(|c: char| !c.is_alphanumeric())
                .any(|w| w == *word)
        })
    }
}

pub fn parse(doc: &Html, base: &Url) -> Option<Credits> {
    let container = super::find_container(doc, |text| labels(text).iter().any(Label::is_credit))?;

    let flat = html::flatten(container, base);
    let labels = labels(flat.text());
    let first = labels.iter().position(Label::is_credit)?;
    let stop = flat.stop(labels[first].range.end, STOPS);

    let mut segments = Vec::new();
    let mut license_url = None;

    for (index, label) in labels.iter().enumerate().skip(first) {
        if label.range.start >= stop {
            break;
        }

        let start = label.range.end;
        let end = labels
            .get(index + 1)
            .map_or(stop, |next| next.range.start)
            .min(stop);
        if start >= end {
            continue;
        }

        let fragment = flat.slice(trim_value(flat.text(), start..end));
        if fragment.is_empty() {
            continue;
        }

        if license_url.is_none() {
            license_url = license_in(&flat.slice(label.range.clone()).html);
        }

        segments.push(Credit {
            role: label.role.clone(),
            html: fragment.html,
            text: fragment.text,
        });
    }

    if segments.is_empty() {
        return None;
    }

    let has_copyright = segments
        .iter()
        .any(|segment| segment.role.to_ascii_lowercase().contains("copyright"));

    Some(Credits {
        segments,
        has_copyright,
        license_url,
    })
}

fn labels(text: &str) -> Vec<Label> {
    LABEL
        .find_iter(text)
        .map(|found| Label {
            range: found.range(),
            role: role_name(found.as_str()),
        })
        .collect()
}

/// `"Image Credit &\n   Copyright:  "` becomes `"Image Credit & Copyright"`.
fn role_name(matched: &str) -> String {
    matched
        .trim()
        .trim_end_matches(':')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A credited name never ends in the punctuation APOD uses to chain roles together.
fn trim_value(text: &str, range: Range<usize>) -> Range<usize> {
    let separator = |c: char| c.is_whitespace() || c == ';' || c == ',';
    let slice = &text[range.clone()];

    let head = slice.trim_start_matches(separator);
    let start = range.end - head.len();
    let end = start + head.trim_end_matches(separator).len();
    start..end
}

fn license_in(label_html: &str) -> Option<String> {
    LICENSE_HREF
        .captures(label_html)
        .map(|caps| caps[1].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://apod.nasa.gov/apod/ap240305.html").unwrap()
    }

    fn parse_body(body: &str) -> Option<Credits> {
        parse(&Html::parse_document(body), &base())
    }

    fn roles(credits: &Credits) -> Vec<&str> {
        credits
            .segments
            .iter()
            .map(|segment| segment.role.as_str())
            .collect()
    }

    #[test]
    fn reads_the_credit_and_notices_copyright() {
        let credits = parse_body(
            r#"<body><center><b>Title</b><br>
               <b>Image Credit &amp; Copyright:</b>
               <a href="http://example.com/">Jane Doe</a></center>
               <p><b>Explanation:</b> prose</p></body>"#,
        )
        .unwrap();

        assert_eq!(roles(&credits), ["Image Credit & Copyright"]);
        assert_eq!(credits.segments[0].text, "Jane Doe");
        assert_eq!(
            credits.segments[0].html,
            r#"<a href="http://example.com/">Jane Doe</a>"#
        );
        assert!(credits.has_copyright);
    }

    #[test]
    fn a_plain_credit_is_not_copyrighted() {
        let credits =
            parse_body("<body><center><b>Image Credit:</b> NASA, ESA, Hubble</center></body>")
                .unwrap();

        assert_eq!(credits.segments[0].text, "NASA, ESA, Hubble");
        assert!(!credits.has_copyright);
        assert_eq!(credits.license_url, None);
    }

    #[test]
    fn handles_video_and_illustration_labels() {
        for label in ["Video Credit", "Illustration Credit", "Credits"] {
            let credits = parse_body(&format!(
                "<body><center><b>{label}:</b> Someone</center></body>"
            ))
            .unwrap();
            assert_eq!(credits.segments[0].text, "Someone");
        }
    }

    #[test]
    fn returns_none_when_there_is_no_credit() {
        assert!(parse_body("<body><p>nothing here</p></body>").is_none());
    }

    #[test]
    fn a_label_split_across_a_link_still_reads() {
        // The modern house style links the word `Copyright` to the rights page, which puts an
        // element boundary in the middle of the label.
        let credits = parse_body(
            r#"<body><center><b>Buck Moon</b> <br>
               <b>Image Credit &amp;
               <a href="lib/about_apod.html#srapply">Copyright</a>:</b>
               <a href="https://www.instagram.com/bnastro000/">Branko Nadj</a>
               </center> <p> <b> Explanation: </b> prose</body>"#,
        )
        .unwrap();

        assert_eq!(roles(&credits), ["Image Credit & Copyright"]);
        assert_eq!(credits.segments[0].text, "Branko Nadj");
        assert!(credits.has_copyright);
    }

    #[test]
    fn splits_a_block_into_one_segment_per_role() {
        let credits = parse_body(
            r#"<body><center><b>Corona Australis</b> <br>
               <b>Image Credit:</b> DES/DOE/FNAL/DECam/CTIO/NOIRLab/NSF/AURA
               <br><b>Image Processing:</b> T.A. Rector (UAA/NOIRLab), R. Colombari
               <br><b>Text:</b> <a href="https://kerockcliffe.com/">Keighley Rockcliffe</a>
               </center> <p> <b> Explanation: </b> prose</body>"#,
        )
        .unwrap();

        assert_eq!(
            roles(&credits),
            ["Image Credit", "Image Processing", "Text"]
        );
        assert_eq!(
            credits.segments[0].text,
            "DES/DOE/FNAL/DECam/CTIO/NOIRLab/NSF/AURA"
        );
        assert_eq!(
            credits.segments[1].text,
            "T.A. Rector (UAA/NOIRLab), R. Colombari"
        );
        assert_eq!(credits.segments[2].text, "Keighley Rockcliffe");
    }

    #[test]
    fn a_semicolon_chained_role_splits_too() {
        let credits = parse_body(
            r#"<body><center><b>Image Credit:</b> NASA/JPL-Caltech/MSSS;
               Processing &amp;
               <a href="https://creativecommons.org/licenses/by/4.0/">License</a>:
               <a href="https://www.flickr.com/people/195227719@N04/">Thomas Thomopoulos</a>
               </center></body>"#,
        )
        .unwrap();

        assert_eq!(roles(&credits), ["Image Credit", "Processing & License"]);
        assert_eq!(credits.segments[0].text, "NASA/JPL-Caltech/MSSS");
        assert_eq!(credits.segments[1].text, "Thomas Thomopoulos");
        assert_eq!(
            credits.license_url.as_deref(),
            Some("https://creativecommons.org/licenses/by/4.0/")
        );
        assert!(!credits.has_copyright);
    }

    #[test]
    fn stops_before_the_page_footer() {
        let credits = parse_body(
            "<body><center><b>Image Credit:</b> NASA
             <p><b>Authors &amp; editors:</b> Robert Nemiroff</center></body>",
        )
        .unwrap();

        assert_eq!(roles(&credits), ["Image Credit"]);
        assert_eq!(credits.segments[0].text, "NASA");
    }

    #[test]
    fn a_title_that_contains_a_colon_is_not_mistaken_for_a_label() {
        let credits = parse_body(
            "<body><center><b>Simulation TNG50: A Galaxy Cluster Forms</b> <br>
             <b>Video Credit:</b> IllustrisTNG Project</center></body>",
        )
        .unwrap();

        assert_eq!(roles(&credits), ["Video Credit"]);
        assert_eq!(credits.segments[0].text, "IllustrisTNG Project");
    }
}
