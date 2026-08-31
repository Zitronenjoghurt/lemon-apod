use crate::config::Config;
use crate::store::Explanation;
use apod_core::{is_decommissioned, ApodEntry, MediaKind};
use poise::serenity_prelude::{
    Colour, CreateAttachment, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter,
};

pub const NAME: &str = "Astronomy Picture of the Day";
const APOD_HOME: &str = "https://science.nasa.gov/apod/";
const NASA_ICON: &str = "https://api.nasa.gov/assets/img/favicons/favicon-192.png";
const NASA_BLUE: Colour = Colour::new(0x0B_3D_91);
const EMBED_TOTAL: usize = 6000;
const DESCRIPTION_MAX: usize = 4096;
const FIELD_VALUE_MAX: usize = 1024;
const TITLE_MAX: usize = 256;
const CREDITS_SHOWN: usize = 3;
const TEASER_CHARS: usize = 320;
const MARGIN: usize = 96;

pub async fn thumbnail(cfg: &Config, entry: &ApodEntry) -> Option<CreateAttachment> {
    let path = entry.media.thumb_path.as_deref()?;
    let file = cfg.thumb_file(path);

    let bytes = match tokio::fs::read(&file).await {
        Ok(bytes) if !bytes.is_empty() => bytes,
        Ok(_) => {
            tracing::warn!(path = %file.display(), "the thumbnail on disk is empty");
            return None;
        }
        Err(error) => {
            tracing::warn!(path = %file.display(), "no thumbnail to attach: {error}");
            return None;
        }
    };

    let name = path.rsplit('/').next().unwrap_or("apod.webp").to_owned();
    let alt = entry.alt.as_deref().unwrap_or(&entry.title);

    Some(CreateAttachment::bytes(bytes, name).description(clip(alt, 1024)))
}

pub fn embed(
    cfg: &Config,
    entry: &ApodEntry,
    explanation: Explanation,
    attachment: Option<&CreateAttachment>,
) -> CreateEmbed {
    let title = clip(&entry.title, TITLE_MAX);
    let footer = footer(entry);
    let credits = credits(entry);

    let spent = NAME.chars().count()
        + title.chars().count()
        + footer.chars().count()
        + credits
            .iter()
            .map(|(name, value)| name.chars().count() + value.chars().count())
            .sum::<usize>();

    let mut embed = CreateEmbed::new()
        .colour(NASA_BLUE)
        .author(
            CreateEmbedAuthor::new(NAME)
                .url(APOD_HOME)
                .icon_url(NASA_ICON),
        )
        .title(title)
        .url(cfg.entry_url(entry.date))
        .footer(CreateEmbedFooter::new(footer));

    if let Some(description) = description(entry, explanation, EMBED_TOTAL.saturating_sub(spent)) {
        embed = embed.description(description);
    }

    for (name, value) in credits {
        embed = embed.field(name, value, false);
    }

    if let Some(attachment) = attachment {
        embed = embed.image(format!("attachment://{}", attachment.filename));
    }

    embed
}

fn description(entry: &ApodEntry, explanation: Explanation, room: usize) -> Option<String> {
    let tail = tail(entry);
    let room = room
        .saturating_sub(tail.chars().count() + MARGIN)
        .min(DESCRIPTION_MAX);

    let body = match explanation {
        Explanation::None => String::new(),
        Explanation::Teaser => entry.summary_text(TEASER_CHARS.min(room)),
        Explanation::Full => entry.summary_text(room),
    };

    let joined = match (body.trim().is_empty(), tail.is_empty()) {
        (true, true) => return None,
        (true, false) => tail,
        (false, true) => body,
        (false, false) => format!("{body}\n\n{tail}"),
    };

    Some(joined)
}

fn tail(entry: &ApodEntry) -> String {
    let mut lines = Vec::new();
    let mut links = Vec::new();

    if !renders_here(entry) {
        if let Some(url) = entry.media.url.as_deref() {
            let (says, opens) = match entry.media.kind {
                MediaKind::ImageTiff => (
                    "NASA's copy of this one is a TIFF, which Discord cannot show.",
                    "Open the original",
                ),
                kind if kind.is_video() => ("This entry is a video.", "Watch it"),
                _ => (
                    "This entry is interactive rather than a picture.",
                    "Open it",
                ),
            };

            lines.push(match is_decommissioned(url) {
                true => says.to_owned(),
                false => format!("{says} [{opens}]({url})."),
            });
        }
    } else if let Some(url) = full_size(entry) {
        links.push(format!("[Full resolution]({url})"));
    }

    if let Some(official) = entry.official_url() {
        links.push(format!("[This entry on APOD]({official})"));
    }

    if !links.is_empty() {
        lines.push(links.join(" · "));
    }

    lines.join("\n")
}

fn full_size(entry: &ApodEntry) -> Option<&str> {
    entry.media.best_url().filter(|url| !is_decommissioned(url))
}

fn renders_here(entry: &ApodEntry) -> bool {
    entry.media.kind.renders_inline()
}

fn footer(entry: &ApodEntry) -> String {
    let date = entry.date.format("%-d %B %Y");
    match entry.has_copyright {
        true => format!("{date} · Copyrighted"),
        false => date,
    }
}

fn credits(entry: &ApodEntry) -> Vec<(String, String)> {
    entry
        .credits
        .iter()
        .filter(|credit| !credit.text.trim().is_empty())
        .take(CREDITS_SHOWN)
        .map(|credit| {
            (
                clip(&credit.role, TITLE_MAX),
                clip(&credit.text, FIELD_VALUE_MAX),
            )
        })
        .collect()
}

fn clip(text: &str, max: usize) -> String {
    match text.chars().count() <= max {
        true => text.to_owned(),
        false => text.chars().take(max.saturating_sub(1)).collect::<String>() + "…",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Announce;
    use apod_core::entry::{Credit, Provenance};
    use apod_core::{ApodDate, Media};
    use std::time::Duration;

    fn cfg() -> Config {
        Config {
            index_db: "apod.db".into(),
            bot_db: "bot.db".into(),
            thumb_dir: "thumbs".into(),
            public_url: "https://apod.example".to_owned(),
            announce: Announce {
                enabled: true,
                poll: Duration::from_secs(60),
                max_age: Duration::from_secs(36 * 3600),
                settle: Duration::from_secs(30 * 60),
            },
            search_page: 5,
            page_life: Duration::from_secs(300),
            owner_ids: Default::default(),
        }
    }

    fn entry() -> ApodEntry {
        let date: ApodDate = "2025-01-31".parse().unwrap();
        ApodEntry {
            date,
            title: "The Variable Nebula NGC 2261".into(),
            title_raw: None,
            explanation_html: "The interstellar <b>cloud</b>.".into(),
            explanation_text: "The interstellar cloud drifts past a young star.".into(),
            credits: vec![Credit {
                role: "Image Credit & Copyright".into(),
                html: "<a href=\"https://example.com\">Tommy Lease</a>".into(),
                text: "Tommy Lease".into(),
            }],
            has_copyright: true,
            license_url: None,
            tomorrow_teaser: None,
            keywords: Vec::new(),
            media: Media::new(
                MediaKind::ImageJpg,
                Some("https://assets.science.nasa.gov/small.jpg".into()),
                None,
            ),
            extra_media: Vec::new(),
            legacy_media_url: None,
            first_stored_at: None,
            alt: None,
            authors: Vec::new(),
            provenance: Provenance::Both,
            source_url: "https://science.nasa.gov/image-article/apod/apod-x/".into(),
            picture: None,
        }
    }

    fn rendered(embed: &CreateEmbed) -> String {
        serde_json::to_string(embed).unwrap()
    }

    #[test]
    fn the_name_is_beside_the_picture_and_it_is_not_abbreviated() {
        let entry = entry();
        let json = rendered(&embed(&cfg(), &entry, Explanation::Full, None));

        assert!(
            json.contains("Astronomy Picture of the Day"),
            "the promise is the words themselves, not the acronym: {json}"
        );
    }

    #[test]
    fn the_title_links_to_the_archive_and_the_body_links_to_apods_own_page() {
        let entry = entry();
        let json = rendered(&embed(&cfg(), &entry, Explanation::Full, None));

        assert!(json.contains("https://apod.example/2025-01-31"), "{json}");
        assert!(
            json.contains("https://science.nasa.gov/image-article/apod/apod-x/"),
            "{json}"
        );
    }

    #[test]
    fn an_entry_credited_to_a_named_holder_says_so_where_the_picture_is() {
        let entry = entry();
        let json = rendered(&embed(&cfg(), &entry, Explanation::Full, None));

        assert!(json.contains("Image Credit & Copyright"), "{json}");
        assert!(json.contains("Tommy Lease"), "{json}");
        assert!(json.contains("31 January 2025 · Copyrighted"), "{json}");
    }

    #[test]
    fn a_public_domain_entry_is_not_labelled_as_somebody_elses() {
        let mut entry = entry();
        entry.has_copyright = false;
        entry.credits.clear();

        let json = rendered(&embed(&cfg(), &entry, Explanation::Full, None));
        assert!(!json.contains("Copyrighted"), "{json}");
        assert!(json.contains("31 January 2025"), "{json}");
    }

    #[test]
    fn a_teaser_is_shorter_than_the_full_explanation_and_full_is_the_whole_thing() {
        let mut entry = entry();
        entry.explanation_text = "word ".repeat(400);

        let full = description(&entry, Explanation::Full, EMBED_TOTAL).unwrap();
        let teaser = description(&entry, Explanation::Teaser, EMBED_TOTAL).unwrap();
        let none = description(&entry, Explanation::None, EMBED_TOTAL).unwrap();

        assert!(teaser.chars().count() < full.chars().count(), "{teaser}");
        assert!(teaser.contains('…'), "a cut teaser says it was cut");
        assert!(!none.contains("word"), "none means none: {none}");
        assert!(
            none.contains("This entry on APOD"),
            "but the link out still has to be there: {none}"
        );
    }

    #[test]
    fn the_longest_explanation_the_archive_holds_still_fits_what_discord_accepts() {
        let mut entry = entry();
        entry.explanation_text = "starlight ".repeat(3_000);
        entry.title = "t".repeat(400);
        entry.credits = vec![
            Credit {
                role: "r".repeat(400),
                html: String::new(),
                text: "c".repeat(2_000),
            };
            5
        ];

        let embed = embed(&cfg(), &entry, Explanation::Full, None);
        let json: serde_json::Value = serde_json::to_value(&embed).unwrap();

        let count = |value: Option<&serde_json::Value>| {
            value.and_then(|v| v.as_str()).unwrap_or("").chars().count()
        };

        let description = count(json.get("description"));
        let title = count(json.get("title"));
        let footer = count(json.get("footer").and_then(|f| f.get("text")));
        let author = count(json.get("author").and_then(|a| a.get("name")));
        let fields: usize = json
            .get("fields")
            .and_then(|f| f.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|row| count(row.get("name")) + count(row.get("value")))
                    .sum()
            })
            .unwrap_or(0);

        assert!(description <= DESCRIPTION_MAX, "{description}");
        assert!(title <= TITLE_MAX, "{title}");
        assert!(
            description + title + footer + author + fields <= EMBED_TOTAL,
            "an embed over the total is refused outright: \
             {description} + {title} + {footer} + {author} + {fields}"
        );
    }

    #[test]
    fn a_picture_carries_a_way_through_to_the_full_size_file() {
        let mut entry = entry();
        entry.media.hd_url = Some("https://assets.science.nasa.gov/big.jpg".into());

        let body = description(&entry, Explanation::Full, EMBED_TOTAL).unwrap();
        assert!(
            body.contains("[Full resolution](https://assets.science.nasa.gov/big.jpg)"),
            "the post carries a 480px thumbnail, so the real file has to be one click away: \
             {body}"
        );

        let bare = description(&entry, Explanation::None, EMBED_TOTAL).unwrap();
        assert!(
            bare.contains("Full resolution"),
            "dropping the explanation does not drop the picture: {bare}"
        );
    }

    #[test]
    fn the_display_copy_stands_in_where_the_archive_knows_no_larger_one() {
        let entry = entry();
        assert_eq!(entry.media.hd_url, None);

        let body = description(&entry, Explanation::Full, EMBED_TOTAL).unwrap();
        assert!(
            body.contains("[Full resolution](https://assets.science.nasa.gov/small.jpg)"),
            "on the modern host the displayed file is the master: {body}"
        );
    }

    #[test]
    fn a_picture_that_only_ever_lived_on_the_dead_host_is_not_linked_at_all() {
        let mut entry = entry();
        entry.media = Media::new(
            MediaKind::ImageJpg,
            Some("https://apod.nasa.gov/apod/image/2501/small.jpg".into()),
            Some("https://apod.nasa.gov/apod/image/2501/big.jpg".into()),
        );

        let body = description(&entry, Explanation::Full, EMBED_TOTAL);
        assert!(
            !body.unwrap_or_default().contains("Full resolution"),
            "apod.nasa.gov stopped answering, and a link to nowhere is worse than no link"
        );
    }

    #[test]
    fn a_video_says_so_rather_than_showing_an_empty_frame() {
        let mut entry = entry();
        entry.media = Media::new(
            MediaKind::YouTube,
            Some("https://www.youtube.com/embed/abc".into()),
            None,
        );

        let body = description(&entry, Explanation::Full, EMBED_TOTAL).unwrap();
        assert!(body.contains("is a video"), "{body}");
        assert!(body.contains("https://www.youtube.com/embed/abc"), "{body}");
        assert!(
            !body.contains("Full resolution"),
            "a video has no full size still to offer, and it is already linked once: {body}"
        );
    }

    #[test]
    fn a_tiff_explains_itself_because_no_client_will_render_it() {
        let mut entry = entry();
        entry.media = Media::new(
            MediaKind::ImageTiff,
            Some("https://assets.science.nasa.gov/saturn.tif".into()),
            None,
        );

        let body = description(&entry, Explanation::Full, EMBED_TOTAL).unwrap();
        assert!(body.contains("TIFF"), "{body}");
    }

    #[test]
    fn a_video_that_is_still_only_on_the_dead_host_says_so_without_a_link_to_nowhere() {
        let mut entry = entry();
        entry.media = Media::new(
            MediaKind::VideoMp4,
            Some("https://apod.nasa.gov/apod/image/2608/RomanLaunch_NASA.mp4".into()),
            None,
        );

        let body = description(&entry, Explanation::Full, EMBED_TOTAL).unwrap();
        assert!(body.contains("is a video"), "{body}");
        assert!(
            !body.contains("apod.nasa.gov"),
            "the legacy record lands first every morning, and its links are retired: {body}"
        );
        assert!(
            body.contains("This entry on APOD"),
            "the reader still needs somewhere to go: {body}"
        );
    }

    #[test]
    fn an_entry_nasa_never_carried_across_does_not_link_a_page_that_is_not_there() {
        let mut entry = entry();
        entry.provenance = Provenance::LegacyOnly;
        entry.source_url = entry.date.source_url();

        let body = description(&entry, Explanation::Full, EMBED_TOTAL).unwrap();
        assert!(
            !body.contains("This entry on APOD"),
            "apod.nasa.gov is decommissioned, so that link would go nowhere: {body}"
        );
    }
}
