use crate::colour::NASA_BLUE;
use crate::config::Config;
use crate::store::Explanation;
use apod_core::{ApodDate, ApodEntry, MediaKind, is_decommissioned};
use poise::serenity_prelude::{
    ButtonStyle, CreateActionRow, CreateAttachment, CreateButton, CreateEmbed, CreateEmbedAuthor,
    CreateEmbedFooter,
};

pub const NAME: &str = "Astronomy Picture of the Day";
const APOD_HOME: &str = "https://science.nasa.gov/apod/";
const NASA_ICON: &str = "https://api.nasa.gov/assets/img/favicons/favicon-192.png";
const EMBED_TOTAL: usize = 6000;
const DESCRIPTION_MAX: usize = 4096;
const FIELD_VALUE_MAX: usize = 1024;
const TITLE_MAX: usize = 256;
const CREDITS_SHOWN: usize = 3;
const TEASER_CHARS: usize = 320;
const MARGIN: usize = 96;

const FAVORITE_ID_PREFIX: &str = "apod:favorite:";
const STAR: char = '⭐';

pub fn favorite_id(date: ApodDate) -> String {
    format!("{FAVORITE_ID_PREFIX}{date}")
}

pub fn favorited_date(custom_id: &str) -> Option<ApodDate> {
    custom_id.strip_prefix(FAVORITE_ID_PREFIX)?.parse().ok()
}

pub fn buttons(cfg: &Config, entry: &ApodEntry, favorites: i64) -> Vec<CreateActionRow> {
    let mut row = vec![
        CreateButton::new(favorite_id(entry.date))
            .label(favorite_label(favorites))
            .emoji(STAR)
            .style(ButtonStyle::Secondary),
    ];

    let official = entry.official_url();
    let media = media_link(entry).filter(|(url, _)| Some(*url) != official);

    let links = [
        media.map(|(url, label)| (url.to_owned(), label)),
        Some((cfg.entry_url(entry.date), "Open")),
        official.map(|url| (url.to_owned(), "On APOD")),
    ];

    for (url, label) in links.into_iter().flatten() {
        if url.starts_with("http") {
            row.push(CreateButton::new_link(url).label(label));
        }
    }

    vec![CreateActionRow::Buttons(row)]
}

fn media_link(entry: &ApodEntry) -> Option<(&str, &'static str)> {
    if renders_here(entry) {
        return full_size(entry).map(|url| (url, "Full resolution"));
    }

    let url = entry
        .media
        .url
        .as_deref()
        .filter(|url| !is_decommissioned(url))?;

    Some((url, offscreen(entry.media.kind).1))
}

fn favorite_label(favorites: i64) -> String {
    match favorites {
        ..=0 => "Favorite".to_owned(),
        count => format!("Favorite · {count}"),
    }
}

pub async fn thumbnail(cfg: &Config, entry: &ApodEntry) -> Option<CreateAttachment> {
    let Some(path) = entry.media.thumb_path.as_deref() else {
        tracing::debug!(date = %entry.date, "no thumbnail recorded, the card carries no picture");
        return None;
    };
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
    if renders_here(entry) || entry.media.url.is_none() {
        return String::new();
    }

    offscreen(entry.media.kind).0.to_owned()
}

fn offscreen(kind: MediaKind) -> (&'static str, &'static str) {
    match kind {
        MediaKind::ImageTiff => (
            "This entry is a TIFF which Discord cannot display.",
            "Open TIFF",
        ),
        kind if kind.is_video() => ("This entry is a video.", "Watch it"),
        _ => (
            "This entry is interactive rather than a picture.",
            "Open site",
        ),
    }
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
    fn the_title_links_to_the_archive_and_apods_own_page_is_a_button() {
        let entry = entry();
        let json = rendered(&embed(&cfg(), &entry, Explanation::Full, None));

        assert!(json.contains("https://apod.example/2025-01-31"), "{json}");
        assert!(
            !json.contains("science.nasa.gov/image-article"),
            "the way to APOD's own page is a button now, not a link buried in prose: {json}"
        );

        let row = row(&buttons(&cfg(), &entry, 0));
        assert_eq!(row[3]["label"], "On APOD");
        assert_eq!(
            row[3]["url"],
            "https://science.nasa.gov/image-article/apod/apod-x/"
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

        assert!(teaser.chars().count() < full.chars().count(), "{teaser}");
        assert!(teaser.contains('…'), "a cut teaser says it was cut");
        assert!(
            description(&entry, Explanation::None, EMBED_TOTAL).is_none(),
            "none means none: every way out of the card is a button now"
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

        let row = row(&buttons(&cfg(), &entry, 0));
        assert_eq!(row[1]["label"], "Full resolution");
        assert_eq!(row[1]["url"], "https://assets.science.nasa.gov/big.jpg");

        let body = description(&entry, Explanation::Full, EMBED_TOTAL).unwrap();
        assert!(
            !body.contains("Full resolution"),
            "the post carries a 480px thumbnail and the real file is one press away, so the \
             prose does not say it twice: {body}"
        );
    }

    #[test]
    fn the_display_copy_stands_in_where_the_archive_knows_no_larger_one() {
        let entry = entry();
        assert_eq!(entry.media.hd_url, None);

        let row = row(&buttons(&cfg(), &entry, 0));
        assert_eq!(
            row[1]["url"], "https://assets.science.nasa.gov/small.jpg",
            "on the modern host the displayed file is the master: {row}"
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

        let row = row(&buttons(&cfg(), &entry, 0));
        assert_eq!(
            row.as_array().map(Vec::len),
            Some(3),
            "apod.nasa.gov stopped answering, and a button to nowhere is worse than no button: \
             {row}"
        );
        assert_eq!(row[1]["label"], "Open");
        assert_eq!(row[2]["label"], "On APOD");
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
        assert!(
            !body.contains("youtube.com"),
            "the sentence says what it is and the button says where it is: {body}"
        );

        let row = row(&buttons(&cfg(), &entry, 0));
        assert_eq!(row[1]["label"], "Watch it");
        assert_eq!(row[1]["url"], "https://www.youtube.com/embed/abc");
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

        let row = row(&buttons(&cfg(), &entry, 0));
        assert_eq!(row[1]["label"], "Open TIFF");
        assert_eq!(row[1]["url"], "https://assets.science.nasa.gov/saturn.tif");
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

        let row = row(&buttons(&cfg(), &entry, 0));
        assert_eq!(
            row.as_array().map(Vec::len),
            Some(3),
            "the reader still needs somewhere to go: {row}"
        );
        assert_eq!(row[2]["label"], "On APOD");
    }

    #[test]
    fn an_entry_nasa_never_carried_across_does_not_link_a_page_that_is_not_there() {
        let mut entry = entry();
        entry.provenance = Provenance::LegacyOnly;
        entry.source_url = entry.date.source_url();

        let row = row(&buttons(&cfg(), &entry, 0));
        let dead = row.as_array().unwrap().iter().any(|button| {
            button["url"]
                .as_str()
                .unwrap_or("")
                .contains("apod.nasa.gov")
        });

        assert!(
            !dead,
            "apod.nasa.gov is decommissioned, so that button would go nowhere: {row}"
        );
    }

    fn row(rows: &[CreateActionRow]) -> serde_json::Value {
        serde_json::to_value(rows).unwrap()[0]["components"].clone()
    }

    #[test]
    fn a_press_carries_the_date_so_the_card_still_works_after_a_restart() {
        let date: ApodDate = "2025-01-31".parse().unwrap();
        let id = favorite_id(date);

        assert_eq!(
            favorited_date(&id),
            Some(date),
            "nothing about the press may depend on the command that posted the card"
        );
        assert_eq!(favorited_date("1234567890:forward"), None);
        assert_eq!(favorited_date("apod:favorite:not-a-date"), None);
        assert_eq!(favorited_date(""), None);
    }

    #[test]
    fn the_button_says_what_it_does_and_carries_how_many_have_done_it() {
        assert_eq!(
            favorite_label(0),
            "Favorite",
            "one message is read by everybody, so a button cannot show each of them their own \
             state and must name the action instead"
        );
        assert_eq!(favorite_label(1), "Favorite · 1");
        assert_eq!(favorite_label(42), "Favorite · 42");
    }

    #[test]
    fn the_row_offers_the_picture_the_archive_and_apods_own_page_beside_the_favorite() {
        let entry = entry();
        let row = row(&buttons(&cfg(), &entry, 3));

        assert_eq!(
            row.as_array().map(Vec::len),
            Some(4),
            "four is the worst case, and Discord takes five to a row: {row}"
        );
        assert_eq!(row[0]["custom_id"], "apod:favorite:2025-01-31");
        assert_eq!(row[0]["label"], "Favorite · 3");
        assert_eq!(row[1]["url"], "https://assets.science.nasa.gov/small.jpg");
        assert_eq!(row[2]["url"], "https://apod.example/2025-01-31");
        assert_eq!(
            row[3]["url"],
            "https://science.nasa.gov/image-article/apod/apod-x/"
        );
    }

    #[test]
    fn an_interactive_entry_offers_the_thing_itself_rather_than_a_still() {
        let mut entry = entry();
        entry.media = Media::new(
            MediaKind::Embed,
            Some("https://stefanom.org/spc/game.php".into()),
            None,
        );

        let body = description(&entry, Explanation::Full, EMBED_TOTAL).unwrap();
        assert!(body.contains("interactive rather than a picture"), "{body}");

        let row = row(&buttons(&cfg(), &entry, 0));
        assert_eq!(row[1]["label"], "Open site");
        assert_eq!(row[1]["url"], "https://stefanom.org/spc/game.php");
    }

    #[test]
    fn the_media_button_and_the_apod_button_are_never_the_same_press() {
        let mut entry = entry();
        let url = entry.source_url.clone();
        entry.media = Media::new(MediaKind::Embed, Some(url), None);

        let row = row(&buttons(&cfg(), &entry, 0));
        assert_eq!(
            row.as_array().map(Vec::len),
            Some(3),
            "two buttons onto one address is one button and a puzzle: {row}"
        );
        assert_eq!(row[2]["label"], "On APOD");
    }

    #[test]
    fn no_two_buttons_in_a_row_read_as_the_same_control() {
        let mut entry = entry();
        entry.media = Media::new(
            MediaKind::Embed,
            Some("https://stefanom.org/spc/game.php".into()),
            None,
        );

        let row = row(&buttons(&cfg(), &entry, 0));
        let labels: Vec<String> = row
            .as_array()
            .unwrap()
            .iter()
            .map(|button| button["label"].as_str().unwrap_or_default().to_owned())
            .collect();

        let mut distinct = labels.clone();
        distinct.sort();

        assert_eq!(
            distinct.len(),
            labels.len(),
            "two controls a press apart cannot read as the same control: {labels:?}"
        );
    }

    #[test]
    fn a_link_discord_would_refuse_drops_its_button_rather_than_the_whole_card() {
        let mut entry = entry();
        entry.provenance = Provenance::LegacyOnly;
        entry.source_url = entry.date.source_url();
        entry.media = Media::new(
            MediaKind::ImageJpg,
            Some("https://apod.nasa.gov/apod/image/2501/small.jpg".into()),
            None,
        );

        let without_apod = row(&buttons(&cfg(), &entry, 0));
        assert_eq!(
            without_apod.as_array().map(Vec::len),
            Some(2),
            "{without_apod}"
        );

        let mut cfg = cfg();
        cfg.public_url = String::new();
        let only_favorite = row(&buttons(&cfg, &entry, 0));
        assert_eq!(
            only_favorite.as_array().map(Vec::len),
            Some(1),
            "an unset public URL takes its own button and leaves the favorite: {only_favorite}"
        );
        assert_eq!(only_favorite[0]["custom_id"], "apod:favorite:2025-01-31");
    }
}
