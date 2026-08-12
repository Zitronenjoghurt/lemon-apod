use crate::config::Config;
use crate::web::escape;
use anyhow::{Context, Result};
use apod_core::ApodEntry;

const MARKER: &str = "<!--APOD_META-->";
const DESCRIPTION_CHARS: usize = 200;

pub struct Shell {
    head: String,
    tail: String,
    default_tags: String,
    public_url: String,
}

impl Shell {
    pub fn load(cfg: &Config) -> Result<Self> {
        let path = cfg.static_dir.join("index.html");
        let html = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;

        let (head, tail) = split(&html);
        Ok(Self {
            head,
            tail,
            default_tags: default_tags(&cfg.public_url),
            public_url: cfg.public_url.clone(),
        })
    }

    pub fn default_page(&self) -> String {
        format!("{}{}{}", self.head, self.default_tags, self.tail)
    }

    pub fn entry_page(&self, entry: &ApodEntry) -> String {
        format!("{}{}{}", self.head, self.entry_tags(entry), self.tail)
    }

    fn entry_tags(&self, entry: &ApodEntry) -> String {
        let title = format!("{} (APOD {})", entry.title, entry.date);
        let description = entry.summary_text(DESCRIPTION_CHARS);
        let url = format!("{}/{}", self.public_url, entry.date);

        let image = entry
            .media
            .url
            .as_deref()
            .filter(|_| entry.media.kind.is_image())
            .map(str::to_owned)
            .or_else(|| {
                entry
                    .media
                    .thumb_url
                    .as_deref()
                    .map(|path| format!("{}{path}", self.public_url))
            });

        let mut tags = format!(
            r#"<title>{title}</title>
<meta name="description" content="{description}">
<meta property="og:type" content="article">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{description}">
<meta property="og:url" content="{url}">
<meta property="og:site_name" content="APOD Archive">
<link rel="canonical" href="{url}">
"#,
            title = escape(&title),
            description = escape(&description),
            url = escape(&url),
        );

        match image {
            Some(image) => tags.push_str(&format!(
                r#"<meta name="twitter:card" content="summary_large_image">
<meta property="og:image" content="{image}">
<meta name="twitter:image" content="{image}">
"#,
                image = escape(&image)
            )),
            None => tags.push_str("<meta name=\"twitter:card\" content=\"summary\">\n"),
        }

        tags
    }
}

fn default_tags(public_url: &str) -> String {
    format!(
        r#"<title>APOD Archive</title>
<meta name="description" content="An archive of every NASA Astronomy Picture of the Day since 1995.">
<meta property="og:type" content="website">
<meta property="og:title" content="APOD Archive">
<meta property="og:description" content="An archive of every NASA Astronomy Picture of the Day since 1995.">
<meta property="og:url" content="{url}">
<meta property="og:site_name" content="APOD Archive">
<meta name="twitter:card" content="summary">
"#,
        url = escape(public_url)
    )
}

fn split(html: &str) -> (String, String) {
    if let Some(at) = html.find(MARKER) {
        return (html[..at].to_owned(), html[at + MARKER.len()..].to_owned());
    }

    if let Some(at) = html.find("</head>") {
        tracing::warn!("index.html has no {MARKER}; falling back to </head>");
        return (html[..at].to_owned(), html[at..].to_owned());
    }

    tracing::warn!("index.html has no {MARKER} and no </head>; link previews will be generic");
    (html.to_owned(), String::new())
}

pub fn entry_path(path: &str) -> Option<apod_core::ApodDate> {
    let candidate = path.trim_start_matches('/').trim_end_matches('/');
    if candidate.len() != 10 {
        return None;
    }
    candidate.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use apod_core::{ApodDate, Media, MediaKind};

    fn shell() -> Shell {
        Shell {
            head: "<html><head>".into(),
            tail: "</head><body><div id=app></div></body></html>".into(),
            default_tags: default_tags("https://apod.lemon.industries"),
            public_url: "https://apod.lemon.industries".into(),
        }
    }

    fn entry() -> ApodEntry {
        let date: ApodDate = "2024-03-05".parse().unwrap();
        ApodEntry {
            date,
            title: "NGC 2170".into(),
            title_raw: None,
            explanation_html: String::new(),
            explanation_text: "Is this a painting or a photograph? ".repeat(20),
            credits: Vec::new(),
            has_copyright: true,
            license_url: None,
            tomorrow_teaser: None,
            keywords: Vec::new(),
            media: Media::new(
                MediaKind::ImageJpg,
                Some("https://apod.nasa.gov/apod/image/2403/a.jpg".into()),
                None,
            ),
            extra_media: Vec::new(),
            source_url: date.source_url(),
        }
    }

    #[test]
    fn splits_on_the_marker() {
        let (head, tail) = split("<head>A<!--APOD_META-->B</head>");
        assert_eq!(head, "<head>A");
        assert_eq!(tail, "B</head>");
    }

    #[test]
    fn falls_back_to_the_head_close_tag() {
        let (head, tail) = split("<html><head><title>x</title></head><body></body></html>");
        assert!(head.ends_with("</title>"));
        assert!(tail.starts_with("</head>"));
    }

    #[test]
    fn builds_entry_tags_pointing_at_the_display_image() {
        let page = shell().entry_page(&entry());
        assert!(page.contains(
            r#"<meta property="og:url" content="https://apod.lemon.industries/2024-03-05">"#
        ));
        assert!(page.contains(r#"content="https://apod.nasa.gov/apod/image/2403/a.jpg""#));
        assert!(page.contains(r#"twitter:card" content="summary_large_image"#));
        assert!(page.contains("<div id=app>"), "the app shell must survive");
    }

    #[test]
    fn a_video_entry_falls_back_to_its_thumbnail() {
        let mut entry = entry();
        entry.media = Media::new(
            MediaKind::YouTube,
            Some("https://www.youtube.com/embed/x".into()),
            None,
        );
        entry.media.thumb_url = Some("/thumbs/2024/03/2024-03-05.webp".into());

        let page = shell().entry_page(&entry);
        assert!(
            page.contains(
                r#"content="https://apod.lemon.industries/thumbs/2024/03/2024-03-05.webp""#
            )
        );
    }

    #[test]
    fn escapes_titles_that_contain_markup_characters() {
        let mut entry = entry();
        entry.title = r#"A "quoted" <thing> & more"#.into();
        let page = shell().entry_page(&entry);

        assert!(page.contains("&quot;quoted&quot;"));
        assert!(!page.contains("<thing>"));
    }

    #[test]
    fn recognises_entry_paths() {
        assert!(entry_path("/2024-03-05").is_some());
        assert!(entry_path("/2024-03-05/").is_some());
        assert!(entry_path("/search").is_none());
        assert!(entry_path("/archive/2024").is_none());
        assert!(entry_path("/not-a-date").is_none());
    }
}
