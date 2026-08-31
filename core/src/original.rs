use crate::date::ApodDate;
use crate::decode::decode_html;
use crate::entry::ApodEntry;
use crate::html::escape;
use regex::{Captures, Regex};
use std::sync::LazyLock;

pub const APOD_HOME: &str = "https://science.nasa.gov/apod/";

pub const AUTHORS: [&str; 4] = [
    "Jerry Bonnell",
    "Cecilia Chirenti",
    "Robert Nemiroff",
    "Keighley Rockcliffe",
];
pub const SERVICE_LINE: &str = "ASD at NASA / GSFC, NASA Science Activation & Michigan Tech. U.";

const NAME: &str = "Astronomy Picture of the Day";

const DISPLAY_WIDTH: u32 = 1024;
const EMBED_TAGS: [&str; 4] = ["iframe", "video", "embed", "object"];
const SITE: &str = "APOD Archive";

const PAGE_PREFIXES: &[&str] = &[
    "https://apod.nasa.gov/apod/",
    "http://apod.nasa.gov/apod/",
    "https://www.apod.nasa.gov/apod/",
    "http://www.apod.nasa.gov/apod/",
    "https://antwrp.gsfc.nasa.gov/apod/",
    "http://antwrp.gsfc.nasa.gov/apod/",
];

static SCRIPT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<script\b[^>]*>.*?</script\s*>|<script\b[^>]*/>").expect("valid")
});
static URL_ATTR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)\b(href|src)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#).expect("valid")
});
static TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<title\b[^>]*>.*?</title\s*>").expect("valid"));
static HEAD_CLOSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)</head\s*>").expect("valid"));
static BODY_OPEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<body\b[^>]*>").expect("valid"));
static EMBED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<(?:iframe|video|embed|object)\b([^>]*)>").expect("valid"));
static SIZE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)\b(width|height)\s*=\s*(?:"(\d+)"|'(\d+)'|(\d+))"#).expect("valid")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    Archived,
    Reconstruction,
}

pub struct Original<'a> {
    pub entry: &'a ApodEntry,
    pub public_url: &'a str,
    pub fetched_at: Option<i64>,
}

impl Original<'_> {
    pub fn archived(&self, bytes: &[u8]) -> String {
        let (decoded, _) = decode_html(bytes);

        let stripped = SCRIPT.replace_all(&decoded, "");
        let rewritten = self.rewrite_urls(&stripped);
        let titled = TITLE.replace(&rewritten, "");

        let mut head = self.head(Form::Archived);
        head.push_str(&embed_sizes(&titled));

        let with_head = match HEAD_CLOSE.find(&titled) {
            Some(at) => splice(&titled, at.start(), at.start(), &head),
            None => format!("<head>{head}</head>\n{titled}"),
        };

        let frame = self.frame(Form::Archived);
        let at = BODY_OPEN
            .find(&with_head)
            .or_else(|| HEAD_CLOSE.find(&with_head))
            .map(|found| found.end());

        match at {
            Some(at) => splice(&with_head, at, at, &frame),
            None => format!("{frame}{with_head}"),
        }
    }

    pub fn reconstructed(&self) -> String {
        let entry = self.entry;
        let authors = match entry.authors.is_empty() {
            true => AUTHORS.join(", "),
            false => entry.authors.join(", "),
        };

        let credits = entry
            .credits
            .iter()
            .map(|credit| {
                format!(
                    "<p class=\"apod-credit\"><b>{}:</b> {}</p>",
                    escape(&credit.role),
                    self.rewrite_urls(&credit.html)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let teaser = entry
            .tomorrow_teaser
            .as_deref()
            .map(|teaser| {
                format!(
                    "<p class=\"apod-teaser\"><b>Tomorrow's picture:</b> {}</p>",
                    escape(teaser)
                )
            })
            .unwrap_or_default();

        format!(
            "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n{head}\
             </head>\n<body class=\"apod-recon\">\n{frame}\n\
             <main class=\"apod-page\">\n\
             <h1 class=\"apod-masthead\">{NAME}</h1>\n\
             <p class=\"apod-when\">{when}</p>\n\
             {media}\n\
             <h2 class=\"apod-title\">{title}</h2>\n\
             {credits}\n\
             <div class=\"apod-explanation\"><b>Explanation:</b> {explanation}</div>\n\
             {teaser}\n\
             <hr>\n\
             <footer class=\"apod-foot\">\n\
             <p><b>Authors &amp; editors:</b> {authors}</p>\n\
             <p><b>A service of:</b> {service}</p>\n\
             </footer>\n\
             </main>\n</body>\n</html>\n",
            head = self.head(Form::Reconstruction),
            frame = self.frame(Form::Reconstruction),
            when = escape(&entry.date.format("%Y %B %-d")),
            media = self.media_block(),
            title = escape(&entry.title),
            explanation = self.rewrite_urls(&entry.explanation_html),
            service = escape(SERVICE_LINE),
        )
    }

    fn rewrite_urls(&self, html: &str) -> String {
        URL_ATTR
            .replace_all(html, |caps: &Captures<'_>| {
                let name = &caps[1];
                let Some(raw) = caps
                    .get(2)
                    .or_else(|| caps.get(3))
                    .or_else(|| caps.get(4))
                    .map(|m| m.as_str())
                else {
                    return caps[0].to_owned();
                };

                match self.target(raw) {
                    Some(url) => format!("{name}=\"{url}\""),
                    None => caps[0].to_owned(),
                }
            })
            .into_owned()
    }

    fn target(&self, raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }
        // A scheme we do not serve, `mailto:` and `javascript:` among them. The colon has to come
        // before any path separator, or `page.html?t=1:2` reads as one.
        if let Some(at) = trimmed.find(':')
            && !trimmed[..at].contains(['/', '?', '#'])
            && !trimmed[..at].eq_ignore_ascii_case("http")
            && !trimmed[..at].eq_ignore_ascii_case("https")
        {
            return None;
        }

        let absolute = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            trimmed.to_owned()
        } else if let Some(rest) = trimmed.strip_prefix("//") {
            format!("https://{rest}")
        } else if let Some(rest) = trimmed.strip_prefix('/') {
            format!("https://apod.nasa.gov/{rest}")
        } else {
            format!("{}{trimmed}", crate::APOD_BASE_URL)
        };

        if let Some(date) = legacy_entry(&absolute) {
            return Some(format!("/{date}/original"));
        }

        if self.entry.legacy_media_url.as_deref() == Some(absolute.as_str())
            && let Some(origin) = self.entry.media.url.as_deref()
        {
            return Some(escape(origin));
        }

        (absolute != trimmed).then(|| escape_attr(&absolute))
    }

    fn title(&self, form: Form) -> String {
        match form {
            Form::Archived => format!("Archived: {} (APOD {})", self.entry.title, self.entry.date),
            Form::Reconstruction => {
                format!(
                    "Reconstruction: {} (APOD {})",
                    self.entry.title, self.entry.date
                )
            }
        }
    }

    fn description(&self, form: Form) -> String {
        match form {
            Form::Archived => format!(
                "The page NASA's {NAME} served on {}, as it was archived, with its links resolved.",
                self.entry.date
            ),
            Form::Reconstruction => format!(
                "A reconstruction of NASA's {NAME} for {}, generated from NASA's own record \
                 because no original page was archived for this date.",
                self.entry.date
            ),
        }
    }

    fn share_image(&self) -> Option<String> {
        let media = &self.entry.media;
        let usable = media.kind.renders_inline()
            && media
                .url
                .as_deref()
                .is_some_and(|url| !crate::is_decommissioned(url));

        match usable {
            true => media.url.clone(),
            false => media
                .thumb_url
                .as_deref()
                .map(|path| format!("{}{path}", self.public_url)),
        }
    }

    fn head(&self, form: Form) -> String {
        let mut tags = format!(
            "<title>{title}</title>\n\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
             <meta name=\"robots\" content=\"noindex, follow\">\n\
             <link rel=\"canonical\" href=\"{canonical}\">\n\
             <meta name=\"description\" content=\"{description}\">\n\
             <meta property=\"og:type\" content=\"article\">\n\
             <meta property=\"og:title\" content=\"{title}\">\n\
             <meta property=\"og:description\" content=\"{description}\">\n\
             <meta property=\"og:url\" content=\"{url}\">\n\
             <meta property=\"og:site_name\" content=\"{SITE}\">\n",
            canonical = escape(&format!("{}/{}", self.public_url, self.entry.date)),
            url = escape(&format!("{}/{}/original", self.public_url, self.entry.date)),
            title = escape(&self.title(form)),
            description = escape(&self.description(form)),
        );

        match self.share_image() {
            Some(image) => tags.push_str(&format!(
                "<meta name=\"twitter:card\" content=\"summary_large_image\">\n\
                 <meta property=\"og:image\" content=\"{image}\">\n\
                 <meta name=\"twitter:image\" content=\"{image}\">\n",
                image = escape(&image)
            )),
            None => tags.push_str("<meta name=\"twitter:card\" content=\"summary\">\n"),
        }

        tags.push_str(STYLE);
        tags.push_str(&self.display_cap());
        tags
    }

    fn display_cap(&self) -> String {
        let Some(origin) = self
            .entry
            .media
            .url
            .as_deref()
            .filter(|_| self.entry.legacy_media_url.is_some())
        else {
            return String::new();
        };

        format!(
            "<style>img[src=\"{}\"]{{max-width:min(100%,{DISPLAY_WIDTH}px)!important}}</style>\n",
            escape(origin)
        )
    }

    fn frame(&self, form: Form) -> String {
        let date = escape(&self.entry.date.format("%-d %B %Y"));
        let (official, label) = match self.entry.official_url() {
            Some(url) => (url, "This entry on APOD"),
            None => (APOD_HOME, "APOD's home"),
        };

        let links = format!(
            "<a href=\"/{date_id}\">This entry on the archive</a>\
             <a href=\"{official}\" rel=\"noopener\" target=\"_blank\">{label}</a>",
            date_id = self.entry.date,
            official = escape(official),
        );

        let lead = match form {
            Form::Archived => format!(
                "Archived copy of {date}{fetched}",
                fetched = match self.fetched_at.and_then(stamp) {
                    Some(when) => format!(", fetched {}", escape(&when)),
                    None => String::new(),
                }
            ),
            Form::Reconstruction => {
                format!("Reconstruction of {date}, no original page was archived")
            }
        };

        format!(
            "<div class=\"apod-frame-slot\">\
             <input type=\"checkbox\" id=\"apod-frame-off\" class=\"apod-frame-x\" \
             title=\"Dismiss this notice\" aria-label=\"Dismiss this notice\">\
             <div class=\"apod-frame\" role=\"note\">\
             <span class=\"apod-frame-lead\">{lead}</span>\
             <span class=\"apod-frame-links\">{links}</span>\
             </div>\
             </div>\n"
        )
    }

    fn media_block(&self) -> String {
        let media = &self.entry.media;
        let alt = escape(self.entry.alt.as_deref().unwrap_or(&self.entry.title));

        let Some(url) = media.url.as_deref() else {
            return String::new();
        };

        if media.kind.renders_inline() {
            return format!(
                "<p class=\"apod-shot\"><a href=\"{full}\"><img src=\"{url}\" alt=\"{alt}\"></a></p>",
                full = escape(media.best_url().unwrap_or(url)),
                url = escape(url),
            );
        }

        let poster = media
            .thumb_url
            .as_deref()
            .map(|thumb| format!("<img src=\"{}\" alt=\"{alt}\">", escape(thumb)))
            .unwrap_or_default();

        let note = match media.kind {
            crate::MediaKind::ImageTiff => {
                "NASA's copy of this picture is a TIFF, which browsers do not display. \
                 The thumbnail above was made from it."
            }
            kind if kind.is_video() => {
                "This entry is a video. It plays on the platform hosting it."
            }
            _ => "This entry is an interactive piece rather than a picture.",
        };

        format!(
            "<p class=\"apod-shot\">{poster}</p>\n\
             <p class=\"apod-note\">{note} <a href=\"{url}\" rel=\"noopener\">Open the original</a>.</p>",
            url = escape(url)
        )
    }
}

fn embed_sizes(html: &str) -> String {
    let mut sizes: Vec<(u32, u32)> = Vec::new();

    for element in EMBED.captures_iter(html) {
        let (mut width, mut height) = (None, None);

        for attribute in SIZE.captures_iter(&element[1]) {
            let Some(value) = attribute
                .get(2)
                .or_else(|| attribute.get(3))
                .or_else(|| attribute.get(4))
                .and_then(|found| found.as_str().parse::<u32>().ok())
            else {
                continue;
            };

            match attribute[1].eq_ignore_ascii_case("width") {
                true => width = Some(value),
                false => height = Some(value),
            }
        }

        let (Some(width), Some(height)) = (width, height) else {
            continue;
        };
        if width > 0 && height > 0 && !sizes.contains(&(width, height)) {
            sizes.push((width, height));
        }
    }

    if sizes.is_empty() {
        return String::new();
    }

    let rules = sizes
        .iter()
        .map(|(width, height)| {
            let selector = EMBED_TAGS
                .iter()
                .map(|tag| format!("{tag}[width=\"{width}\"][height=\"{height}\"]"))
                .collect::<Vec<_>>()
                .join(",");
            format!("{selector}{{height:auto;aspect-ratio:{width}/{height}}}")
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("<style>\n{rules}\n</style>\n")
}

fn legacy_entry(absolute: &str) -> Option<ApodDate> {
    let rest = PAGE_PREFIXES
        .iter()
        .find_map(|prefix| absolute.strip_prefix(prefix))?;

    let name = rest.split(['?', '#']).next()?;
    if name.contains('/') {
        return None;
    }

    ApodDate::from_legacy_filename(name).filter(|date| date.days() >= 0)
}

fn stamp(seconds: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(seconds, 0).map(|when| when.format("%-d %B %Y").to_string())
}

fn escape_attr(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    let mut rest = url;

    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        match rest[1..].split_once(';') {
            Some((name, _))
                if !name.is_empty()
                    && name.len() <= 8
                    && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'#') =>
            {
                out.push('&');
            }
            _ => out.push_str("&amp;"),
        }
        rest = &rest[1..];
    }

    out.push_str(rest);
    out.replace('"', "&quot;")
}

fn splice(haystack: &str, from: usize, to: usize, with: &str) -> String {
    let mut out = String::with_capacity(haystack.len() + with.len());
    out.push_str(&haystack[..from]);
    out.push_str(with);
    out.push_str(&haystack[to..]);
    out
}

const STYLE: &str = r#"<style>
html{color-scheme:light}
body:not([bgcolor]):not(.apod-recon){background:#fff;color:#000}
.apod-frame-slot{position:relative}
.apod-frame{font-family:system-ui,-apple-system,"Segoe UI",sans-serif;line-height:1.45;
margin:0 0 1rem;text-align:left;display:flex;flex-wrap:wrap;align-items:baseline;gap:.15rem 1rem;
font-size:.78rem;padding:.35rem 2.2rem .35rem .6rem;border:1px solid #dcdfe8;border-radius:.35rem;
background:#fff;color:#5f647a}
.apod-frame a{color:#3a53c4}
.apod-frame-links{display:flex;flex-wrap:wrap;gap:.15rem 1rem}
.apod-frame-links a{white-space:nowrap}
.apod-frame-x{position:absolute;top:.2rem;right:.25rem;appearance:none;-webkit-appearance:none;
margin:0;padding:0;width:1.7rem;height:1.7rem;border:0;border-radius:.3rem;background:none;
font:inherit;font-size:.95rem;line-height:1.7rem;text-align:center;color:#8a8fa3;cursor:pointer}
.apod-frame-x::before{content:"\00d7"}
.apod-frame-x:hover{color:#1b1d26;background:#eceef4}
.apod-frame-x:focus-visible{outline:2px solid #3a53c4;outline-offset:1px}
.apod-frame-x:checked{display:none}
.apod-frame-x:checked+.apod-frame{display:none}
img{max-width:100%;height:auto}
iframe,video,embed,object{max-width:100%}
body{overflow-wrap:break-word}
@media(max-width:40rem){table,pre{display:block;max-width:100%;overflow-x:auto}}
.apod-recon{background:#f4f4ff;color:#000;font-family:Georgia,"Times New Roman",serif;
margin:0;padding:1rem}
.apod-page{max-width:52rem;margin:0 auto;text-align:center}
.apod-masthead{font-size:1.6rem;font-weight:400;margin:.5rem 0}
.apod-when{margin:.5rem 0}
.apod-title{font-size:1.15rem;margin:1rem 0 .5rem}
.apod-shot{margin:1rem 0}
.apod-note{font-size:.9rem;color:#333}
.apod-explanation{text-align:left;margin:1.5rem 0;line-height:1.5}
.apod-foot{font-size:.85rem}
.apod-recon a{color:#00f}
.apod-recon a:visited{color:#7f0f9f}
</style>
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{Credit, Provenance};
    use crate::media::{Media, MediaKind};

    const PAGE: &str = r##"<!doctype html>
<html>
<head>
<title> APOD: 2025 January 31 - The Variable Nebula NGC 2261
</title>
<script id="_fed_an_ua_tag" src="//dap.digitalgov.gov/x.js">
</script>
</head>
<body BGCOLOR="#F4F4FF" text="#000000">
<center>
<h1> Astronomy Picture of the Day </h1>
<a href="archivepix.html">Discover the cosmos!</a>
<a href="image/2501/big.jpg">
<IMG SRC="image/2501/small.jpg"
alt="See Explanation." style="max-width:100%"></a>
</center>
<a href="https://apod.nasa.gov/apod/reflection_nebulae.html">reflection nebula</a>
<a href="ap250201.html">light-weekend</a>
<a href="/apod.rss">RSS</a>
<a href="http://asterisk.apod.com/discuss_apod.php?date=250131&amp;x=1">Discuss</a>
<a href="mailto:someone@example.com">mail</a>
<script src="//cdn.parsely.com/p.js" async="" defer=""></script>
</body>
</html>
"##;

    fn entry() -> ApodEntry {
        let date: ApodDate = "2025-01-31".parse().unwrap();
        ApodEntry {
            date,
            title: "The Variable Nebula NGC 2261".into(),
            title_raw: None,
            explanation_html: "The interstellar <b>cloud</b>.".into(),
            explanation_text: "The interstellar cloud.".into(),
            credits: vec![Credit {
                role: "Image Credit & Copyright".into(),
                html: "<a href=\"https://example.com\">Tommy Lease</a>".into(),
                text: "Tommy Lease".into(),
            }],
            has_copyright: true,
            license_url: None,
            tomorrow_teaser: Some("light-weekend".into()),
            keywords: Vec::new(),
            media: Media::new(
                MediaKind::ImageJpg,
                Some("https://assets.science.nasa.gov/content/dam/small.jpg".into()),
                None,
            ),
            extra_media: Vec::new(),
            legacy_media_url: Some("https://apod.nasa.gov/apod/image/2501/small.jpg".into()),
            first_stored_at: None,
            alt: None,
            authors: vec!["Jerry Bonnell".into()],
            provenance: Provenance::Both,
            source_url: "https://science.nasa.gov/image-article/apod/apod-x/".into(),
            picture: None,
        }
    }

    fn view(entry: &ApodEntry) -> Original<'_> {
        Original {
            entry,
            public_url: "https://apod.lemon.industries",
            fetched_at: Some(1_754_998_907),
        }
    }

    fn archived() -> String {
        let entry = entry();
        view(&entry).archived(PAGE.as_bytes())
    }

    #[test]
    fn no_script_survives_the_rewrite() {
        let page = archived();
        assert!(!page.contains("<script"), "{page}");
        assert!(!page.contains("dap.digitalgov.gov"));
        assert!(!page.contains("cdn.parsely.com"));
    }

    #[test]
    fn every_relative_url_resolves_against_the_legacy_base() {
        let page = archived();
        assert!(
            page.contains(r#"href="https://apod.nasa.gov/apod/archivepix.html""#),
            "{page}"
        );
        assert!(page.contains(r#"href="https://apod.nasa.gov/apod/image/2501/big.jpg""#));
        assert!(
            page.contains(r#"href="https://apod.nasa.gov/apod.rss""#),
            "a root-relative path hangs off the host, not the apod directory"
        );
    }

    #[test]
    fn the_attribute_name_may_be_shouted() {
        let page = archived();
        assert!(
            !page.contains("IMG SRC=\"image/"),
            "the corpus writes IMG SRC in capitals and it still has to be rewritten: {page}"
        );
    }

    #[test]
    fn the_entrys_own_picture_moves_to_the_host_that_survives() {
        let page = archived();
        assert!(
            page.contains(r#"SRC="https://assets.science.nasa.gov/content/dam/small.jpg""#),
            "{page}"
        );
        assert!(
            page.contains(
                r#"img[src="https://assets.science.nasa.gov/content/dam/small.jpg"]{max-width:min(100%,1024px)!important}"#
            ),
            "the origin file is the full-resolution master, and the page laid out for a display \
             copy, so it has to be capped back to about that size: {page}"
        );
        assert!(
            page.contains(r#"href="https://apod.nasa.gov/apod/image/2501/big.jpg""#),
            "only the displayed copy has a known origin URL; the linked one is left where it was"
        );
    }

    #[test]
    fn a_page_whose_picture_was_not_substituted_caps_nothing() {
        let mut entry = entry();
        entry.legacy_media_url = None;

        let page = view(&entry).archived(PAGE.as_bytes());
        assert!(
            !page.contains("max-width:min(100%"),
            "a legacy image that is still the legacy image keeps whatever size it was: {page}"
        );
    }

    #[test]
    fn a_video_embed_keeps_its_shape_while_it_shrinks_to_the_screen() {
        let entry = entry();
        let page = view(&entry).archived(
            br#"<html><head></head><body>
               <iframe width="960" height="540" src="https://www.youtube.com/embed/x?rel=0"></iframe>
               <object width="900" height="600"><embed width="900" height="600"></object>
               </body></html>"#,
        );

        assert!(
            page.contains("iframe,video,embed,object{max-width:100%}"),
            "nothing the page embeds may be wider than the screen it is read on: {page}"
        );
        for tag in ["iframe", "video", "embed", "object"] {
            assert!(
                page.contains(&format!(r#"{tag}[width="960"][height="540"]"#)),
                "the size APOD authored is the ratio to hold it to: {page}"
            );
        }
        assert!(
            page.contains("{height:auto;aspect-ratio:960/540}"),
            "capping the width without releasing the authored height squashes the frame: {page}"
        );
        assert!(
            page.contains("{height:auto;aspect-ratio:900/600}"),
            "a page carrying two differently shaped embeds needs a rule for each: {page}"
        );
    }

    #[test]
    fn an_embed_without_a_shape_of_its_own_is_left_alone() {
        let entry = entry();
        let page = view(&entry).archived(
            br#"<html><head></head><body>
               <iframe width="0" height="0" src="https://tracker.example/beacon"></iframe>
               <iframe width="100%" src="https://example.com/thing"></iframe>
               </body></html>"#,
        );

        assert!(
            !page.contains("aspect-ratio:"),
            "a zero-sized beacon and a frame already sized in percent have no ratio to keep, and              inventing one for them would be a guess: {page}"
        );
    }

    #[test]
    fn a_page_with_no_embeds_carries_no_rules_for_them() {
        assert!(
            !archived().contains("aspect-ratio:"),
            "the rules are generated per page, so a page of pictures gets none"
        );
    }

    #[test]
    fn another_day_of_the_archive_points_at_our_own_copy_of_it() {
        let page = archived();
        assert!(page.contains(r#"href="/2025-02-01/original""#), "{page}");
        assert!(
            page.contains(r#"href="https://apod.nasa.gov/apod/reflection_nebulae.html""#),
            "a library page is not an entry page"
        );
    }

    #[test]
    fn a_truncated_filename_is_not_a_date() {
        assert!(legacy_entry("https://apod.nasa.gov/apod/ap07071.html").is_none());
        assert!(legacy_entry("https://apod.nasa.gov/apod/apnotadate.html").is_none());
        assert!(legacy_entry("https://apod.nasa.gov/apod/calendar/allyears.html").is_none());
        assert_eq!(
            legacy_entry("https://apod.nasa.gov/apod/ap070713.html").map(|d| d.to_string()),
            Some("2007-07-13".to_owned())
        );
    }

    #[test]
    fn somebody_elses_url_is_left_exactly_as_nasa_wrote_it() {
        let page = archived();
        assert!(
            page.contains(
                r#"href="http://asterisk.apod.com/discuss_apod.php?date=250131&amp;x=1""#
            ),
            "an absolute URL is not ours to touch, entities included: {page}"
        );
        assert!(page.contains(r#"href="mailto:someone@example.com""#));
    }

    #[test]
    fn both_forms_of_the_banner_are_the_same_quiet_line() {
        let entry = entry();
        let recon = view(&entry).reconstructed();
        let archived = archived();

        for page in [&recon, &archived] {
            assert!(
                page.contains("<div class=\"apod-frame\" role=\"note\">"),
                "one banner, one shape: {page}"
            );
        }
        assert!(
            !recon.contains("apod-frame-foot"),
            "the reconstruction says what it is once, at the top, and not again at the bottom"
        );
    }

    #[test]
    fn the_banner_folds_away_for_this_reading_of_the_page_and_is_back_on_the_next() {
        let entry = entry();

        for page in [&view(&entry).reconstructed(), &archived()] {
            assert!(
                page.contains(r#"<input type="checkbox" id="apod-frame-off" class="apod-frame-x""#),
                "a page served under script-src 'none' can only dismiss with a checkbox: {page}"
            );
            assert!(
                page.contains(".apod-frame-x:checked+.apod-frame{display:none}"),
                "and checking it has to actually take the banner away: {page}"
            );
            let rule = page
                .split_once(".apod-frame-x{")
                .expect("the control is styled")
                .1;
            assert!(
                rule.starts_with("position:absolute") && rule[..60].contains("right:"),
                "the control is pinned to the far right of the banner, not left in the flow: \
                 {rule}"
            );
            assert!(
                !page.contains("<script") && !page.contains("localStorage"),
                "nothing remembers the dismissal, it lasts as long as this look at the page: \
                 {page}"
            );
        }
    }

    #[test]
    fn the_frame_names_apod_and_says_when_the_copy_was_taken() {
        let page = archived();
        let body = page.split_once("<body").unwrap().1;
        assert!(
            body.contains("Astronomy Picture of the Day"),
            "the name has to be near the picture, not only in the head"
        );
        assert!(body.contains("Archived copy of"));
        assert!(body.contains("fetched 12 August 2025"), "{body}");
        assert!(body.contains(r#"href="/2025-01-31""#));
    }

    #[test]
    fn the_head_says_what_this_is_and_keeps_it_out_of_the_index() {
        let page = archived();
        let head = page.split_once("</head>").unwrap().0;
        assert!(head.contains("<title>Archived: The Variable Nebula NGC 2261 (APOD 2025-01-31)"));
        assert!(
            !head.contains("<title> APOD: 2025 January 31"),
            "NASA's own title is replaced"
        );
        assert!(head.contains(r#"content="noindex, follow""#));
        assert!(
            head.contains(
                r#"<link rel="canonical" href="https://apod.lemon.industries/2025-01-31">"#
            )
        );
        assert!(
            head.contains(r#"content="https://assets.science.nasa.gov/content/dam/small.jpg""#)
        );
    }

    #[test]
    fn the_oldest_pages_carry_no_body_tag_and_still_get_the_frame_above_the_page() {
        let entry = entry();
        let bare = b"<title> APOD: June 16, 1995 </title>\n<h1> Astronomy Picture of the Day </h1>";
        let page = view(&entry).archived(bare);

        let frame = page.find("apod-frame").expect("the frame is there");
        let content = page.find("<h1>").expect("the page is there");
        assert!(
            frame < content,
            "a banner below the page it labels is the one place it cannot be: {page}"
        );
        assert!(page.contains("og:title"));
    }

    #[test]
    fn a_reconstruction_cannot_be_mistaken_for_an_archived_page() {
        let entry = entry();
        let page = view(&entry).reconstructed();

        assert!(
            page.contains("Reconstruction of 31 January 2025, no original page was archived"),
            "{page}"
        );
        assert!(page.contains("<title>Reconstruction: The Variable Nebula NGC 2261"));
        assert!(page.contains("Astronomy Picture of the Day"));
        assert!(page.contains("The interstellar <b>cloud</b>."));
        assert!(page.contains("<b>Image Credit &amp; Copyright:</b>"));
        assert!(page.contains("Tomorrow's picture:"));
    }

    #[test]
    fn a_reconstruction_states_the_credit_the_record_carries_and_the_service_line() {
        let entry = entry();
        let page = view(&entry).reconstructed();
        assert!(page.contains("<b>Authors &amp; editors:</b> Jerry Bonnell"));
        assert!(page.contains("NASA Science Activation &amp; Michigan Tech. U."));

        let mut anonymous = entry;
        anonymous.authors.clear();
        let page = view(&anonymous).reconstructed();
        assert!(
            page.contains("Jerry Bonnell, Cecilia Chirenti, Robert Nemiroff, Keighley Rockcliffe"),
            "a record with no authors falls back to the credit APOD carries today: {page}"
        );
    }

    #[test]
    fn a_tiff_shows_its_thumbnail_and_says_why() {
        let mut entry = entry();
        entry.media = Media::new(
            MediaKind::ImageTiff,
            Some("https://assets.science.nasa.gov/content/dam/saturn.tif".into()),
            None,
        );
        entry.media.thumb_url = Some("/thumbs/2025/11/2025-11-22.webp".into());

        let page = view(&entry).reconstructed();
        assert!(
            page.contains(r#"<img src="/thumbs/2025/11/2025-11-22.webp""#),
            "{page}"
        );
        assert!(page.contains("is a TIFF, which browsers do not display"));
        assert!(
            page.contains(
                r#"content="https://apod.lemon.industries/thumbs/2025/11/2025-11-22.webp""#
            ),
            "a share card cannot be a TIFF either"
        );
    }

    #[test]
    fn an_entry_nasa_never_carried_across_links_apods_home_rather_than_a_page_that_is_not_there() {
        let mut entry = entry();
        entry.provenance = Provenance::LegacyOnly;
        entry.source_url = entry.date.source_url();

        let page = view(&entry).archived(PAGE.as_bytes());
        let frame = page.split_once("apod-frame-links").unwrap().1;
        assert!(frame.contains(APOD_HOME), "{frame}");
    }
}
