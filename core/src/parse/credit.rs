use crate::entry::Credit;
use crate::html::{self, Flat};
use regex::Regex;
use scraper::{ElementRef, Html};
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
    "additional",
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
    "science",
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

const UNLABELLED_ROLE: &str = "Credit";
const MAX_UNLABELLED_CHARS: usize = 300;
const DASH: &str = r"[-\x{2013}\x{2014}]";

fn role_run() -> String {
    let words = ROLE_WORDS.join("|");
    format!(r"\b(?:{words})(?:[\s&/,-]+(?:and[\s]+)?(?:{words})\b){{0,4}}")
}

pub(super) fn is_all_role_words(text: &str) -> bool {
    let mut counted = false;

    for word in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty() && !word.eq_ignore_ascii_case("and"))
    {
        if !ROLE_WORDS
            .iter()
            .any(|role| word.eq_ignore_ascii_case(role))
        {
            return false;
        }
        counted = true;
    }

    counted
}

static LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i){}(?:\s*\([^)]*\))?(?:\s*:\s*|\s+{DASH}\s+)",
        role_run()
    ))
    .expect("the role vocabulary builds a valid pattern")
});

static BARE_LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?i)^\s*{}\s+", role_run()))
        .expect("the role vocabulary builds a valid pattern")
});

static LABEL_NOTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*\([^)]*\)$").expect("static pattern is valid"));

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

struct Segment {
    role: String,
    label: Range<usize>,
    value: Range<usize>,
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

pub fn parse(doc: &Html, base: &Url, title: &str) -> Option<Credits> {
    labelled(doc, base).or_else(|| after_title(doc, base, title))
}

pub fn attributes_anyone(doc: &Html) -> bool {
    let text = doc.root_element().text().collect::<String>();
    labels(&text).iter().any(Label::is_credit)
}

fn labelled(doc: &Html, base: &Url) -> Option<Credits> {
    let container = super::find_container(doc, |text| labels(text).iter().any(Label::is_credit))?;
    from_element(container, base)
}

pub(super) fn from_element(container: ElementRef<'_>, base: &Url) -> Option<Credits> {
    let flat = html::flatten(container, base);
    let labels = labels(flat.text());
    let first = labels.iter().position(Label::is_credit)?;
    let stop = flat.stop(labels[first].range.end, STOPS);

    finish(&flat, split(&labels[first..], stop))
}

fn after_title(doc: &Html, base: &Url, title: &str) -> Option<Credits> {
    let container = super::find_container(doc, |text| html::collapse(text).contains(title))?;
    let flat = html::flatten(container, base);

    let start = flat.text().find(title)? + title.len();
    let stop = flat.stop(start, STOPS);
    if stop.saturating_sub(start) > MAX_UNLABELLED_CHARS {
        return None;
    }

    let labels: Vec<Label> = labels(flat.text())
        .into_iter()
        .filter(|label| (start..stop).contains(&label.range.start))
        .collect();

    let head = start..labels.first().map_or(stop, |label| label.range.start);
    let mut segments: Vec<Segment> = unpunctuated(flat.text(), head).into_iter().collect();
    segments.extend(split(&labels, stop));

    finish(&flat, segments)
}

fn split(labels: &[Label], stop: usize) -> Vec<Segment> {
    labels
        .iter()
        .enumerate()
        .take_while(|(_, label)| label.range.start < stop)
        .map(|(index, label)| Segment {
            role: label.role.clone(),
            value: label.range.end
                ..labels
                    .get(index + 1)
                    .map_or(stop, |next| next.range.start)
                    .min(stop),
            label: label.range.clone(),
        })
        .collect()
}

fn unpunctuated(text: &str, range: Range<usize>) -> Option<Segment> {
    let (role, start) = match BARE_LABEL.find(&text[range.clone()]) {
        Some(found) => (role_name(found.as_str()), range.start + found.end()),
        None => (UNLABELLED_ROLE.to_owned(), range.start),
    };

    (start < range.end).then_some(Segment {
        role,
        label: start..start,
        value: start..range.end,
    })
}

fn finish(flat: &Flat, segments: Vec<Segment>) -> Option<Credits> {
    let mut credits = Vec::new();
    let mut has_copyright = false;
    let mut license_url = None;

    for segment in segments {
        has_copyright |= segment.role.to_ascii_lowercase().contains("copyright");
        if license_url.is_none() && !segment.label.is_empty() {
            license_url = license_in(&flat.slice(segment.label).html);
        }

        if segment.value.start >= segment.value.end {
            continue;
        }

        let fragment = flat.slice(trim_value(flat.text(), segment.value));
        if fragment.is_empty() {
            continue;
        }

        credits.push(Credit {
            role: segment.role,
            html: fragment.html,
            text: fragment.text,
        });
    }

    if credits.is_empty() {
        return None;
    }

    Some(Credits {
        segments: credits,
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

fn role_name(matched: &str) -> String {
    let roles = matched
        .trim()
        .trim_end_matches([':', '-', '\u{2013}', '\u{2014}']);
    html::collapse(&LABEL_NOTE.replace(roles.trim_end(), ""))
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

    const NO_TITLE: &str = "A Title On No Page Below";

    fn parse_body(body: &str) -> Option<Credits> {
        parse_titled(body, NO_TITLE)
    }

    fn parse_titled(body: &str, title: &str) -> Option<Credits> {
        parse(&Html::parse_document(body), &base(), title)
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

    #[test]
    fn a_role_set_in_italics_ends_at_its_dash() {
        let credits = parse_titled(
            r#"<body><center><b>Young Star Cluster NGC 346</b> <br>
               <i>Science</i> - <a href="https://www.nasa.gov">NASA</a>, ESA, CSA
               <br><i>Processing</i> - Alyssa Pagan (STScI)
               </center> <p> <b>Explanation:</b> prose</body>"#,
            "Young Star Cluster NGC 346",
        )
        .unwrap();

        assert_eq!(roles(&credits), ["Science", "Processing"]);
        assert_eq!(credits.segments[0].text, "NASA, ESA, CSA");
        assert_eq!(credits.segments[1].text, "Alyssa Pagan (STScI)");
    }

    #[test]
    fn a_label_that_never_got_its_colon_still_reads() {
        let credits = parse_titled(
            r#"<body><center><b>Herschel Crater on Mimas</b> <br>
               <b>Image Credit</b>
               <a href="http://ciclops.org/">Cassini Imaging Team</a>, JPL, ESA, NASA
               </center> <p> <b>Explanation:</b> prose</body>"#,
            "Herschel Crater on Mimas",
        )
        .unwrap();

        assert_eq!(roles(&credits), ["Image Credit"]);
        assert_eq!(
            credits.segments[0].text,
            "Cassini Imaging Team, JPL, ESA, NASA"
        );
        assert!(
            credits.segments[0]
                .html
                .contains(r#"<a href="http://ciclops.org/">"#)
        );
    }

    #[test]
    fn an_attribution_with_no_label_at_all_is_still_an_attribution() {
        let credits = parse_titled(
            r#"<body><center><b>Saturn at Night</b> <br>
               <a href="https://www.nasa.gov/">NASA</a>, JPL-Caltech,
               <a href="https://www.spacescience.org/">Space Science Institute</a>
               </center> <p> <b>Explanation:</b> prose</body>"#,
            "Saturn at Night",
        )
        .unwrap();

        assert_eq!(roles(&credits), [UNLABELLED_ROLE]);
        assert_eq!(
            credits.segments[0].text,
            "NASA, JPL-Caltech, Space Science Institute"
        );
        assert!(!credits.has_copyright);
    }

    #[test]
    fn prose_under_the_title_is_not_mistaken_for_an_attribution() {
        let prose = "This kilometer high cliff occurs on the surface of a comet. ".repeat(10);
        assert!(
            parse_titled(
                &format!("<body><center><b>A Cliff</b> <br> {prose}</center></body>"),
                "A Cliff",
            )
            .is_none()
        );
    }

    #[test]
    fn a_label_spelling_out_its_licence_keeps_only_the_roles() {
        let credits = parse_titled(
            r#"<body><center><b>A Kilometer High Cliff</b> <br>
               <b>Image Credit &amp;
               <a href="https://creativecommons.org/licenses/by-sa/3.0/igo/">Licence
               (CC BY-SA 3.0 IGO)</a>: </b>
               <a href="http://www.esa.int/">ESA</a>, Rosetta spacecraft, NAVCAM;
               <b>Additional Processing:</b> Stuart Atkinson
               </center> <p> <b>Explanation:</b> prose</body>"#,
            "A Kilometer High Cliff",
        )
        .unwrap();

        assert_eq!(
            roles(&credits),
            ["Image Credit & Licence", "Additional Processing"]
        );
        assert_eq!(credits.segments[0].text, "ESA, Rosetta spacecraft, NAVCAM");
        assert_eq!(credits.segments[1].text, "Stuart Atkinson");
        assert_eq!(
            credits.license_url.as_deref(),
            Some("https://creativecommons.org/licenses/by-sa/3.0/igo/")
        );
    }

    #[test]
    fn a_header_label_keeps_its_copyright_when_sub_roles_take_every_name() {
        let credits = parse_body(
            r#"<body><center><b>NGC 1365</b> <br>
               <b>Image Credit &amp;
               <a href="lib/about_apod.html#srapply">Copyright</a>: </b>
               <i>Processing</i> - <a href="https://millenniumphoton.com/">J.-B. Auroux</a>,
               <i>Data</i> - <a href="https://throughlightandtime.com/">Mike Selby</a>
               </center> <p> <b>Explanation:</b> prose</body>"#,
        )
        .unwrap();

        assert_eq!(roles(&credits), ["Processing", "Data"]);
        assert!(
            credits.has_copyright,
            "the header claims the copyright even though it credits nobody directly"
        );
    }
}
