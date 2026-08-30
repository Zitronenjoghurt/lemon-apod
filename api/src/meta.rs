use crate::config::Config;
use crate::web::escape;
use anyhow::{Context, Result};
use apod_core::{ApodDate, ApodEntry, PictureAppearances, Resource, is_decommissioned};

const MARKER: &str = "<!--APOD_META-->";
const DESCRIPTION_CHARS: usize = 200;
const NAME: &str = "Astronomy Picture of the Day";
const SITE: &str = "APOD Archive";
const SITE_DESCRIPTION: &str = "An archive of every NASA Astronomy Picture of the Day since 1995.";

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

struct Fixed {
    path: &'static str,
    title: &'static str,
    description: &'static str,
}

const FIXED: &[Fixed] = &[
    Fixed {
        path: "/feed",
        title: "Feed",
        description: "An endlessly scrolling feed of every NASA Astronomy Picture of the Day since 1995.",
    },
    Fixed {
        path: "/search",
        title: "Search",
        description: "Search through thirty years of NASAs Astronomy Picture of the Day.",
    },
    Fixed {
        path: "/favorites",
        title: "Favorites",
        description: "Your favorite entries of NASAs Astronomy Picture of the Day.",
    },
    Fixed {
        path: "/random",
        title: "Random APOD",
        description: "A random entry from the full archive of NASAs Astronomy Picture of the Day.",
    },
    Fixed {
        path: "/archive",
        title: "Archive",
        description: "Browse every NASA Astronomy Picture of the Day by year and month, as a grid of thumbnails or as a calendar.",
    },
    Fixed {
        path: "/stats",
        title: "Statistics",
        description: "Detailed statistics about the full archive of NASAs Astronomy Picture of the Day.",
    },
    Fixed {
        path: "/space-weather",
        title: "Space weather",
        description: "Solar activity and its influence on Earth, as measured and forecast by NOAA's Space Weather Prediction Center.",
    },
    Fixed {
        path: "/pictures",
        title: "Encores",
        description: "The NASA Astronomy Picture of the Day that have been published more than once, and what changed between their appearances",
    },
    Fixed {
        path: "/resources",
        title: "Resources",
        description: "A comprehensive catalogue of every webpage NASAs Astronomy Picture of the Day has ever referenced.",
    },
    Fixed {
        path: "/modernization",
        title: "Modernization",
        description: "NASA moved the Astronomy Picture of the Day from apod.nasa.gov to science.nasa.gov. These are statistics about the modernization itself.",
    },
    Fixed {
        path: "/modernization/changes",
        title: "What changed",
        description: "NASA moved the Astronomy Picture of the Day from apod.nasa.gov to science.nasa.gov. This is a list of the changes that have been made to individual entries.",
    },
    Fixed {
        path: "/notifications",
        title: "Notifications",
        description: "Follow the archive by Atom, by RSS, or through ntfy push notifications.",
    },
    Fixed {
        path: "/contact",
        title: "Contact",
        description: "Contact the creator of this NASA Astronomy Picture of the Day archive.",
    },
    Fixed {
        path: "/games",
        title: "Games",
        description: "A minigame mashup of NASAs Astronomy Picture of the Day.",
    },
    Fixed {
        path: "/games/date",
        title: "Guess the Date",
        description: "A minigame where you guess the date of a NASA Astronomy Picture of the Day.",
    },
    Fixed {
        path: "/games/order",
        title: "Older or Newer",
        description: "A minigame where you guess whether a NASA Astronomy Picture of the Day was published before or after another.",
    },
    Fixed {
        path: "/games/match",
        title: "Match the Picture",
        description: "A minigame where you guess which NASA Astronomy Picture of the Day a given description originates from.",
    },
    Fixed {
        path: "/games/words",
        title: "Fill the Words",
        description: "A minigame where you have to fill in the words of a NASA Astronomy Picture of the Day to eventually uncover its title.",
    },
    Fixed {
        path: "/rating",
        title: "Reader ratings",
        description: "Which NASA Astronomy Picture of the Day users think the best!",
    },
    Fixed {
        path: "/rating/vote",
        title: "Vote on a pair",
        description: "Vote which NASA Astronomy Picture of the Day you like the most!",
    },
];

pub enum Target {
    Entry(ApodDate),
    Picture(ApodDate),
    Resource(i64),
    Fixed,
}

struct Meta {
    title: String,
    description: String,
    image: Option<String>,
    article: bool,
}

impl Meta {
    fn new(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            image: None,
            article: false,
        }
    }

    fn titled(name: &str, description: impl Into<String>) -> Self {
        Self::new(format!("{name} \u{b7} {SITE}"), description)
    }
}

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

    pub fn page(&self, path: &str) -> String {
        let path = trimmed(path);
        match fixed(path).or_else(|| archive(path)) {
            Some(meta) => self.render(&meta, path),
            None => self.default_page(),
        }
    }

    pub fn gap_page(&self, date: ApodDate) -> String {
        let Some((title, opening)) = crate::api::routes::gaps::describe(date) else {
            return self.default_page();
        };

        let meta = Meta {
            title: format!("{title} \u{b7} {SITE}"),
            description: format!("NASA's {NAME}, {date}. {opening}"),
            image: None,
            article: true,
        };

        self.render(&meta, &format!("/{date}"))
    }

    pub fn entry_page(&self, entry: &ApodEntry) -> String {
        let meta = Meta {
            title: format!("{} (APOD {})", entry.title, entry.date),
            description: format!(
                "From NASA's {NAME}. {}",
                entry.summary_text(DESCRIPTION_CHARS)
            ),
            image: self.entry_image(entry),
            article: true,
        };

        self.render(&meta, &format!("/{}", entry.date))
    }

    pub fn picture_page(&self, path: &str, found: &PictureAppearances) -> String {
        let picture = &found.picture;
        let (first, last) = (picture.first.format("%Y"), picture.last.format("%Y"));

        let description = format!(
            "NASA's {NAME} has shown this picture {} times{}. Every date it appeared on, and what has changed.",
            picture.appearances,
            if first == last {
                format!(" in {first}")
            } else {
                format!(" between {first} and {last}")
            }
        );

        let meta = Meta {
            title: format!(
                "{} \u{b7} Shown {}\u{d7}",
                picture.title, picture.appearances
            ),
            description,
            image: picture
                .media
                .thumb_url
                .as_deref()
                .map(|thumb| format!("{}{thumb}", self.public_url)),
            article: false,
        };

        self.render(&meta, trimmed(path))
    }

    pub fn resource_page(&self, path: &str, resource: &Resource) -> String {
        let name = resource
            .label
            .as_deref()
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .unwrap_or(&resource.key);

        let meta = Meta::titled(
            name,
            format!(
                "APOD has linked to {name} {} times across {} {}.",
                resource.refs,
                resource.entries,
                if resource.entries == 1 {
                    "entry"
                } else {
                    "entries"
                }
            ),
        );

        self.render(&meta, trimmed(path))
    }

    fn entry_image(&self, entry: &ApodEntry) -> Option<String> {
        entry
            .media
            .url
            .as_deref()
            .filter(|url| entry.media.kind.renders_inline() && !is_decommissioned(url))
            .map(str::to_owned)
            .or_else(|| {
                entry
                    .media
                    .thumb_url
                    .as_deref()
                    .map(|path| format!("{}{path}", self.public_url))
            })
    }

    fn render(&self, meta: &Meta, path: &str) -> String {
        format!("{}{}{}", self.head, self.tags(meta, path), self.tail)
    }

    fn tags(&self, meta: &Meta, path: &str) -> String {
        let url = format!("{}{path}", self.public_url);

        let mut tags = format!(
            r#"<title>{title}</title>
<meta name="description" content="{description}">
<meta property="og:type" content="{kind}">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{description}">
<meta property="og:url" content="{url}">
<meta property="og:site_name" content="{SITE}">
<link rel="canonical" href="{url}">
"#,
            kind = if meta.article { "article" } else { "website" },
            title = escape(&meta.title),
            description = escape(&meta.description),
            url = escape(&url),
        );

        match &meta.image {
            Some(image) => tags.push_str(&format!(
                r#"<meta name="twitter:card" content="summary_large_image">
<meta property="og:image" content="{image}">
<meta name="twitter:image" content="{image}">
"#,
                image = escape(image)
            )),
            None => tags.push_str("<meta name=\"twitter:card\" content=\"summary\">\n"),
        }

        if noindex(path) {
            tags.push_str("<meta name=\"robots\" content=\"noindex, follow\">\n");
        }

        tags
    }
}

fn default_tags(public_url: &str) -> String {
    format!(
        r#"<title>{SITE}</title>
<meta name="description" content="{SITE_DESCRIPTION}">
<meta property="og:type" content="website">
<meta property="og:title" content="{SITE}">
<meta property="og:description" content="{SITE_DESCRIPTION}">
<meta property="og:url" content="{url}">
<meta property="og:site_name" content="{SITE}">
<meta name="twitter:card" content="summary">
"#,
        url = escape(public_url)
    )
}

fn fixed(path: &str) -> Option<Meta> {
    FIXED
        .iter()
        .find(|page| page.path == path)
        .map(|page| Meta::titled(page.title, page.description))
}

fn archive(path: &str) -> Option<Meta> {
    let mut parts = path.strip_prefix("/archive/")?.split('/');

    let year: i32 = parts.next()?.parse().ok()?;
    if !(1995..=9999).contains(&year) {
        return None;
    }

    let month = match parts.next() {
        None => None,
        Some(raw) => {
            let value: usize = raw.parse().ok()?;
            if !(1..=12).contains(&value) {
                return None;
            }
            Some(MONTHS[value - 1])
        }
    };

    if parts.next().is_some() {
        return None;
    }

    Some(match month {
        Some(name) => Meta::titled(
            &format!("APOD in {name} {year}"),
            format!("Every Astronomy Picture of the Day published in {name} {year}."),
        ),
        None => Meta::titled(
            &format!("APOD in {year}"),
            format!("Every Astronomy Picture of the Day published in {year}, month by month."),
        ),
    })
}

fn noindex(path: &str) -> bool {
    matches!(path, "/random" | "/favorites" | "/rating/vote")
}

pub fn indexable_paths() -> impl Iterator<Item = &'static str> {
    FIXED
        .iter()
        .map(|page| page.path)
        .filter(|path| !noindex(path))
}

fn trimmed(path: &str) -> &str {
    match path.strip_suffix('/') {
        Some("") | None => path,
        Some(trimmed) => trimmed,
    }
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

pub fn target(path: &str) -> Target {
    let path = trimmed(path);

    if let Some(date) = as_date(path.trim_start_matches('/')) {
        return Target::Entry(date);
    }

    if let Some(date) = path.strip_prefix("/pictures/").and_then(as_date) {
        return Target::Picture(date);
    }

    if let Some(id) = path
        .strip_prefix("/resources/")
        .and_then(|raw| raw.parse::<i64>().ok())
    {
        return Target::Resource(id);
    }

    Target::Fixed
}

fn as_date(candidate: &str) -> Option<ApodDate> {
    if candidate.len() != 10 {
        return None;
    }
    candidate.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use apod_core::{Media, MediaKind, Picture};

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
                Some("https://assets.science.nasa.gov/content/dam/a.jpg".into()),
                None,
            ),
            extra_media: Vec::new(),
            legacy_media_url: None,
            alt: None,
            authors: Vec::new(),
            provenance: apod_core::Provenance::LegacyOnly,
            source_url: date.source_url(),
            picture: None,
        }
    }

    fn appearances() -> PictureAppearances {
        let mut media = Media::new(MediaKind::ImageJpg, None, None);
        media.thumb_url = Some("/thumbs/1997/02/1997-02-14.webp".into());

        PictureAppearances {
            picture: Picture {
                id: "1997-02-14".parse().unwrap(),
                title: "The Pleiades".into(),
                media,
                appearances: 4,
                first: "1997-02-14".parse().unwrap(),
                last: "2019-11-02".parse().unwrap(),
                titles: 2,
                span_days: 8296,
            },
            items: Vec::new(),
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
    fn every_page_that_carries_a_picture_spells_apod_out() {
        let entry = shell().entry_page(&entry());
        assert!(entry.contains("og:image"), "{entry}");
        assert!(
            entry.contains("From NASA&#39;s Astronomy Picture of the Day."),
            "{entry}"
        );

        let picture = shell().picture_page("/pictures/2024-03-05", &appearances());
        assert!(picture.contains("og:image"), "{picture}");
        assert!(
            picture.contains("NASA&#39;s Astronomy Picture of the Day has shown this picture"),
            "{picture}"
        );
    }

    #[test]
    fn builds_entry_tags_pointing_at_the_display_image() {
        let page = shell().entry_page(&entry());
        assert!(page.contains(
            r#"<meta property="og:url" content="https://apod.lemon.industries/2024-03-05">"#
        ));
        assert!(page.contains(r#"content="https://assets.science.nasa.gov/content/dam/a.jpg""#));
        assert!(page.contains(r#"twitter:card" content="summary_large_image"#));
        assert!(page.contains("<div id=app>"), "the app shell must survive");
    }

    #[test]
    fn a_picture_on_the_host_being_switched_off_is_not_offered_as_a_share_image() {
        let mut entry = entry();
        entry.media = Media::new(
            MediaKind::ImageJpg,
            Some("https://apod.nasa.gov/apod/image/2403/a.jpg".into()),
            None,
        );
        entry.media.thumb_url = Some("/thumbs/2024/03/2024-03-05.webp".into());

        let page = shell().entry_page(&entry);
        assert!(
            !page.contains("apod.nasa.gov"),
            "a card fetched weeks after it was posted would find nothing there: {page}"
        );
        assert!(
            page.contains(
                r#"content="https://apod.lemon.industries/thumbs/2024/03/2024-03-05.webp""#
            )
        );
    }

    #[test]
    fn an_entry_with_only_a_dying_picture_and_no_thumbnail_offers_no_image_at_all() {
        let mut entry = entry();
        entry.media = Media::new(
            MediaKind::ImageJpg,
            Some("https://apod.nasa.gov/apod/image/2403/a.jpg".into()),
            None,
        );

        let page = shell().entry_page(&entry);
        assert!(!page.contains("og:image"), "{page}");
        assert!(page.contains(r#"twitter:card" content="summary"#));
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
    fn recognises_what_each_path_needs_looked_up() {
        assert!(matches!(target("/2024-03-05"), Target::Entry(_)));
        assert!(matches!(target("/2024-03-05/"), Target::Entry(_)));
        assert!(matches!(target("/pictures/2024-03-05"), Target::Picture(_)));
        assert!(matches!(target("/resources/412"), Target::Resource(412)));

        for plain in ["/search", "/archive/2024", "/pictures", "/resources/x", "/"] {
            assert!(matches!(target(plain), Target::Fixed), "{plain}");
        }
    }

    #[test]
    fn every_fixed_page_carries_its_own_title_and_canonical() {
        let shell = shell();

        for page in FIXED {
            let html = shell.page(page.path);
            assert!(
                html.contains(&format!("<title>{} \u{b7} {SITE}</title>", page.title)),
                "{} kept the site-level title",
                page.path
            );
            assert!(
                html.contains(&format!(
                    r#"<link rel="canonical" href="https://apod.lemon.industries{}">"#,
                    page.path
                )),
                "{} has no canonical of its own",
                page.path
            );
            assert!(html.contains("<div id=app>"), "{}", page.path);
        }
    }

    #[test]
    fn a_trailing_slash_addresses_the_same_page() {
        assert_eq!(shell().page("/search/"), shell().page("/search"));
    }

    #[test]
    fn a_year_and_a_month_name_themselves() {
        let shell = shell();

        assert!(shell.page("/archive/2024").contains("<title>APOD in 2024"));
        assert!(
            shell
                .page("/archive/2024/03")
                .contains("<title>APOD in March 2024")
        );
    }

    #[test]
    fn an_archive_path_that_makes_no_sense_keeps_the_site_tags() {
        let shell = shell();

        for nonsense in [
            "/archive/1801",
            "/archive/2024/13",
            "/archive/2024/00",
            "/archive/2024/03/05",
            "/archive/nineteen",
        ] {
            assert_eq!(shell.page(nonsense), shell.default_page(), "{nonsense}");
        }
    }

    #[test]
    fn the_reader_s_own_pages_are_kept_out_of_the_index() {
        assert!(shell().page("/favorites").contains(r#"content="noindex"#));
        assert!(!shell().page("/feed").contains("noindex"));
    }

    #[test]
    fn a_picture_page_leads_with_the_picture_and_its_count() {
        let shell = shell();
        let html = shell.picture_page("/pictures/1997-02-14", &appearances());
        assert!(html.contains("<title>The Pleiades"));
        assert!(html.contains("shown this picture 4 times between 1997 and 2019"));
        assert!(
            html.contains(
                r#"content="https://apod.lemon.industries/thumbs/1997/02/1997-02-14.webp""#
            )
        );
    }

    #[test]
    fn a_resource_page_names_the_site_it_stands_for() {
        let shell = shell();
        let resource = Resource {
            id: 412,
            url: "https://en.wikipedia.org/wiki/Orion".into(),
            key: "en.wikipedia.org/wiki/Orion".into(),
            host: "en.wikipedia.org".into(),
            label: Some("Orion".into()),
            refs: 9,
            entries: 7,
            credited: 0,
            first: None,
            last: None,
        };

        let html = shell.resource_page("/resources/412", &resource);
        assert!(html.contains("<title>Orion \u{b7} APOD Archive</title>"));
        assert!(html.contains("linked to Orion 9 times across 7 entries"));
    }
}
