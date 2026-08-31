use crate::entry::ApodEntry;
use regex::Regex;
use std::sync::LazyLock;
use url::Url;

const APOD_HOSTS: &[&str] = &["apod.nasa.gov", "antwrp.gsfc.nasa.gov"];
const APOD_FURNITURE: &[&str] = &[
    "/apod/lib/",
    "/apod/calendar/",
    "/apod/archivepix",
    "/apod/archive",
    "/apod/index",
    "/cgi-bin/",
    "/lib/",
];
const MEDIA_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "bmp", "tif", "tiff", "svg", "webp", "ico", "mp4", "m4v", "mov",
    "avi", "wmv", "mpg", "mpeg", "flv", "swf", "webm", "mp3", "wav", "ogg", "m4a",
];

static ANCHOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)<a href="([^"]*)">(.*?)</a>"#).unwrap());
static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
static APOD_ENTRY_PAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)/ap\d{6}\.html$").unwrap());

pub const HTTP: &str = "http";
pub const HTTPS: &str = "https";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub key: String,
    pub scheme: String,
    pub host: String,
    pub anchor: String,
    pub in_credit: bool,
    pub count: u32,
}

impl Link {
    pub fn url(&self) -> String {
        url(&self.scheme, &self.key)
    }
}

pub fn url(scheme: &str, key: &str) -> String {
    format!("{scheme}://{key}")
}

pub fn links(entry: &ApodEntry) -> Vec<Link> {
    let mut found: Vec<Link> = Vec::new();

    let blocks = std::iter::once((entry.explanation_html.as_str(), false)).chain(
        entry
            .credits
            .iter()
            .map(|credit| (credit.html.as_str(), true)),
    );

    for (html, in_credit) in blocks {
        for capture in ANCHOR.captures_iter(html) {
            let Some(link) = link(&capture[1], &capture[2], in_credit) else {
                continue;
            };

            match found.iter_mut().find(|seen| seen.key == link.key) {
                Some(seen) => {
                    seen.count += 1;
                    seen.in_credit |= in_credit;
                    if link.scheme == HTTPS {
                        seen.scheme = link.scheme;
                    }
                }
                None => found.push(link),
            }
        }
    }

    found
}

fn link(href: &str, anchor_html: &str, in_credit: bool) -> Option<Link> {
    let url = Url::parse(&unescape(href)).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }

    let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }

    let path = match url.path().trim_end_matches('/') {
        "" => "/",
        trimmed => trimmed,
    };

    if is_media(path) || is_apod_furniture(&host, path) {
        return None;
    }

    let authority = match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.clone(),
    };
    let query = url.query().map_or(String::new(), |q| format!("?{q}"));

    Some(Link {
        key: format!("{authority}{path}{query}"),
        scheme: url.scheme().to_owned(),
        host: host.strip_prefix("www.").unwrap_or(&host).to_owned(),
        anchor: crate::html::collapse(&unescape(&TAGS.replace_all(anchor_html, ""))),
        in_credit,
        count: 1,
    })
}

fn is_media(path: &str) -> bool {
    path.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .is_some_and(|extension| MEDIA_EXTENSIONS.contains(&extension.as_str()))
}

fn is_apod_furniture(host: &str, path: &str) -> bool {
    APOD_HOSTS.contains(&host)
        && (path == "/"
            || path == "/apod"
            || APOD_ENTRY_PAGE.is_match(path)
            || APOD_FURNITURE
                .iter()
                .any(|prefix| path.to_ascii_lowercase().starts_with(prefix)))
}

fn unescape(raw: &str) -> String {
    raw.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ApodDate;
    use crate::entry::Credit;
    use crate::media::{Media, MediaKind};

    fn entry(explanation: &str, credits: &[&str]) -> ApodEntry {
        ApodEntry {
            date: ApodDate::START,
            title: "Title".into(),
            title_raw: None,
            explanation_html: explanation.into(),
            explanation_text: String::new(),
            credits: credits
                .iter()
                .map(|html| Credit {
                    role: "Image Credit".into(),
                    html: (*html).into(),
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

    fn keys(explanation: &str) -> Vec<String> {
        links(&entry(explanation, &[]))
            .into_iter()
            .map(|link| link.key)
            .collect()
    }

    #[test]
    fn catalogues_an_outbound_link_with_its_text() {
        let found = links(&entry(
            r#"The <a href="https://en.wikipedia.org/wiki/Crab_Nebula">Crab Nebula</a> glows."#,
            &[],
        ));

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].key, "en.wikipedia.org/wiki/Crab_Nebula");
        assert_eq!(found[0].url(), "https://en.wikipedia.org/wiki/Crab_Nebula");
        assert_eq!(found[0].host, "en.wikipedia.org");
        assert_eq!(found[0].anchor, "Crab Nebula");
        assert!(!found[0].in_credit);
    }

    #[test]
    fn apods_own_pages_are_cross_references_not_resources() {
        assert!(
            keys(r#"<a href="https://apod.nasa.gov/apod/ap240304.html">yesterday</a>"#).is_empty()
        );
        assert!(
            keys(r#"<a href="https://antwrp.gsfc.nasa.gov/apod/ap960101.html">old</a>"#).is_empty()
        );
        assert!(
            keys(r#"<a href="https://apod.nasa.gov/apod/lib/about_apod.html">about</a>"#)
                .is_empty()
        );
        assert!(
            keys(r#"<a href="https://apod.nasa.gov/apod/archivepix.html">archive</a>"#).is_empty()
        );
        assert!(keys(r#"<a href="https://apod.nasa.gov/">home</a>"#).is_empty());

        assert_eq!(
            keys(r#"<a href="https://apod.nasa.gov/apod/emission_nebulae.html">nebulae</a>"#),
            vec!["apod.nasa.gov/apod/emission_nebulae.html"],
            "APOD's subject index pages are real referenced pages"
        );
    }

    #[test]
    fn a_picture_or_a_recording_is_media_not_a_resource() {
        for href in [
            "https://apod.nasa.gov/apod/image/2403/M31.jpg",
            "https://example.com/clip.MP4",
            "https://i.pinimg.com/originals/a/b/c.png",
        ] {
            assert!(
                keys(&format!(r#"<a href="{href}">x</a>"#)).is_empty(),
                "{href} should not be catalogued"
            );
        }

        assert_eq!(
            keys(r#"<a href="https://arxiv.org/pdf/2401.01234.pdf">the paper</a>"#).len(),
            1,
            "a paper is something to read, whatever its file extension"
        );
    }

    #[test]
    fn the_same_resource_spelled_two_ways_is_one_resource() {
        let found = links(&entry(
            r#"<a href="http://www.nasa.gov/">NASA</a> and
               <a href="https://www.nasa.gov/#top">NASA again</a> and
               <a href="https://www.nasa.gov">NASA once more</a>"#,
            &[],
        ));

        assert_eq!(
            found.len(),
            1,
            "scheme, fragment and trailing slash are noise"
        );
        assert_eq!(found[0].count, 3);
        assert_eq!(found[0].key, "www.nasa.gov/");
        assert_eq!(
            found[0].url(),
            "https://www.nasa.gov/",
            "a link the catalogue hands out should be the one that still works"
        );
        assert_eq!(found[0].host, "nasa.gov", "the site groups without its www");
        assert_eq!(found[0].anchor, "NASA", "the first text APOD used names it");
    }

    #[test]
    fn a_credited_link_is_marked_as_one() {
        let found = links(&entry(
            "no links here",
            &[r#"<a href="https://www.astrobin.com/users/jdoe/">Jane Doe</a>"#],
        ));
        assert_eq!(found.len(), 1);
        assert!(found[0].in_credit);
        assert_eq!(found[0].anchor, "Jane Doe");
    }

    #[test]
    fn a_resource_linked_from_both_places_is_marked_as_credited() {
        let found = links(&entry(
            r#"see <a href="https://example.com/x">this</a>"#,
            &[r#"<a href="https://example.com/x">Jane Doe</a>"#],
        ));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].count, 2);
        assert!(found[0].in_credit);
    }

    #[test]
    fn anchor_text_survives_the_markup_inside_it() {
        let found = links(&entry(
            r#"<a href="https://example.com/x">Jane <b>Q</b>  Doe &amp; Co</a>"#,
            &[],
        ));
        assert_eq!(found[0].anchor, "Jane Q Doe & Co");
    }

    #[test]
    fn only_the_web_is_catalogued() {
        assert!(keys(r#"<a href="mailto:someone@example.com">write</a>"#).is_empty());
        assert!(keys(r#"<a href="ap240304.html">relative</a>"#).is_empty());
    }

    #[test]
    fn a_port_stays_part_of_the_address() {
        assert_eq!(
            keys(r#"<a href="http://example.com:8080/page">x</a>"#),
            vec!["example.com:8080/page"]
        );
    }

    #[test]
    fn a_query_string_distinguishes_two_pages() {
        let found = keys(
            r#"<a href="https://e.com/s?id=1">one</a> <a href="https://e.com/s?id=2">two</a>"#,
        );
        assert_eq!(found, vec!["e.com/s?id=1", "e.com/s?id=2"]);
    }
}
