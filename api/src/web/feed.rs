use crate::api::error::ApiResult;
use crate::api::response;
use crate::config::Publish;
use crate::schedule;
use crate::state::ServerState;
use apod_core::original::APOD_HOME;
use apod_core::{ApodEntry, Filters, Order};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use chrono::{DateTime, NaiveTime, SecondsFormat, Utc};

const ATOM_HTTP_TYPE: &str = "application/atom+xml; charset=utf-8";
const RSS_HTTP_TYPE: &str = "application/rss+xml; charset=utf-8";
const ATOM_MEDIA_TYPE: &str = "application/atom+xml";
const RSS_MEDIA_TYPE: &str = "application/rss+xml";

const TITLE: &str = "APOD Archive";
const DESCRIPTION: &str =
    "A picture of our universe every day, from NASA's Astronomy Picture of the Day.";
const SUMMARY_CHARS: usize = 400;

pub async fn get_atom(State(state): State<ServerState>, headers: HeaderMap) -> ApiResult<Response> {
    let life = until_next_entry(&state);
    let xml = state
        .atom
        .get_or_build_capped(life, || build_atom(&state))
        .await?;
    Ok(response::revalidated(&headers, &xml, ATOM_HTTP_TYPE))
}

pub async fn get_rss(State(state): State<ServerState>, headers: HeaderMap) -> ApiResult<Response> {
    let life = until_next_entry(&state);
    let xml = state
        .rss
        .get_or_build_capped(life, || build_rss(&state))
        .await?;
    Ok(response::revalidated(&headers, &xml, RSS_HTTP_TYPE))
}

fn until_next_entry(state: &ServerState) -> std::time::Duration {
    schedule::until_next(&state.config.publish, Utc::now())
}

async fn latest(state: &ServerState) -> ApiResult<Vec<ApodEntry>> {
    let page = state
        .store
        .list(
            &Filters::default(),
            None,
            state.config.feed_limit,
            Order::Desc,
        )
        .await?;

    let mut entries = Vec::with_capacity(page.items.len());
    for summary in &page.items {
        if let Some(entry) = state.store.entry(summary.date).await? {
            entries.push(entry);
        }
    }

    Ok(entries)
}

fn published_at(publish: &Publish, entry: &ApodEntry) -> DateTime<Utc> {
    schedule::instant_on(publish, entry.date.naive())
        .unwrap_or_else(|| entry.date.naive().and_time(NaiveTime::MIN).and_utc())
}

fn rfc3339(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Secs, true)
}

async fn build_atom(state: &ServerState) -> ApiResult<String> {
    let entries = latest(state).await?;
    let base = &state.config.public_url;
    let publish = &state.config.publish;
    let updated = entries
        .first()
        .map(|entry| published_at(publish, entry))
        .unwrap_or_else(Utc::now);

    let mut xml = String::with_capacity(entries.len() * 4096);
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<feed xmlns=\"http://www.w3.org/2005/Atom\">\n");
    push(&mut xml, 1, "title", TITLE);
    push(&mut xml, 1, "subtitle", DESCRIPTION);
    push(&mut xml, 1, "id", &format!("{base}/atom.xml"));
    push(&mut xml, 1, "updated", &rfc3339(updated));
    push_link(
        &mut xml,
        1,
        "self",
        ATOM_MEDIA_TYPE,
        &format!("{base}/atom.xml"),
    );
    push_link(&mut xml, 1, "alternate", "text/html", &format!("{base}/"));
    xml.push_str("  <author>\n    <name>NASA APOD</name>\n  </author>\n");
    push(&mut xml, 1, "generator", "lemon-apod");

    for entry in &entries {
        let url = format!("{base}/{}", entry.date);
        let at = rfc3339(published_at(publish, entry));

        xml.push_str("  <entry>\n");
        push(&mut xml, 2, "title", &entry.title);
        push(&mut xml, 2, "id", &url);
        push_link(&mut xml, 2, "alternate", "text/html", &url);
        push(&mut xml, 2, "published", &at);
        push(&mut xml, 2, "updated", &at);
        push_typed(
            &mut xml,
            2,
            "summary",
            "text",
            &entry.summary_text(SUMMARY_CHARS),
        );
        push_typed(&mut xml, 2, "content", "html", &content_html(base, entry));
        xml.push_str("  </entry>\n");
    }

    xml.push_str("</feed>\n");
    Ok(xml)
}

async fn build_rss(state: &ServerState) -> ApiResult<String> {
    let entries = latest(state).await?;
    let base = &state.config.public_url;
    let publish = &state.config.publish;
    let built = entries
        .first()
        .map(|entry| published_at(publish, entry))
        .unwrap_or_else(Utc::now);

    let mut xml = String::with_capacity(entries.len() * 4096);
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\">\n");
    xml.push_str("  <channel>\n");
    push(&mut xml, 2, "title", TITLE);
    push(&mut xml, 2, "link", &format!("{base}/"));
    push(&mut xml, 2, "description", DESCRIPTION);
    push(&mut xml, 2, "language", "en");
    push(&mut xml, 2, "lastBuildDate", &built.to_rfc2822());
    xml.push_str(&format!(
        "    <atom:link href=\"{}/feed.xml\" rel=\"self\" type=\"{RSS_MEDIA_TYPE}\"/>\n",
        super::escape(base)
    ));

    for entry in &entries {
        let url = format!("{base}/{}", entry.date);

        xml.push_str("    <item>\n");
        push(&mut xml, 3, "title", &entry.title);
        push(&mut xml, 3, "link", &url);
        xml.push_str("      <guid isPermaLink=\"true\">");
        xml.push_str(&super::escape(&url));
        xml.push_str("</guid>\n");
        push(
            &mut xml,
            3,
            "pubDate",
            &published_at(publish, entry).to_rfc2822(),
        );
        push(&mut xml, 3, "description", &content_html(base, entry));
        xml.push_str("    </item>\n");
    }

    xml.push_str("  </channel>\n</rss>\n");
    Ok(xml)
}

fn content_html(base: &str, entry: &ApodEntry) -> String {
    let mut html = String::with_capacity(entry.explanation_html.len() + 512);

    if let Some(thumb) = entry.media.thumb_url.as_deref() {
        html.push_str(&format!(
            "<p><a href=\"{base}/{date}\"><img src=\"{base}{thumb}\" alt=\"{alt}\"",
            base = super::escape(base),
            date = entry.date,
            thumb = super::escape(thumb),
            alt = super::escape(&entry.title),
        ));
        if let (Some(width), Some(height)) = (entry.media.thumb_width, entry.media.thumb_height) {
            html.push_str(&format!(" width=\"{width}\" height=\"{height}\""));
        }
        html.push_str("></a></p>");
    }

    html.push_str(&format!(
        "<p>From NASA's <a href=\"{source}\">Astronomy Picture of the Day</a></p>",
        source = super::escape(entry.official_url().unwrap_or(APOD_HOME)),
    ));

    html.push_str("<p>");
    html.push_str(&entry.explanation_html);
    html.push_str("</p>");

    for credit in &entry.credits {
        html.push_str(&format!(
            "<p><strong>{}:</strong> {}</p>",
            super::escape(&credit.role),
            credit.html
        ));
    }

    html
}

fn push(xml: &mut String, depth: usize, tag: &str, value: &str) {
    indent(xml, depth);
    xml.push_str(&format!("<{tag}>{}</{tag}>\n", super::escape(value)));
}

fn push_typed(xml: &mut String, depth: usize, tag: &str, kind: &str, value: &str) {
    indent(xml, depth);
    xml.push_str(&format!(
        "<{tag} type=\"{kind}\">{}</{tag}>\n",
        super::escape(value)
    ));
}

fn push_link(xml: &mut String, depth: usize, rel: &str, kind: &str, href: &str) {
    indent(xml, depth);
    xml.push_str(&format!(
        "<link rel=\"{rel}\" type=\"{kind}\" href=\"{}\"/>\n",
        super::escape(href)
    ));
}

fn indent(xml: &mut String, depth: usize) {
    for _ in 0..depth {
        xml.push_str("  ");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apod_core::media::{Media, MediaKind};
    use apod_core::{ApodDate, Credit};
    use chrono_tz::Tz;

    const BASE: &str = "https://apod.example";

    fn entry() -> ApodEntry {
        ApodEntry {
            date: ApodDate::from_ymd(2026, 3, 5).unwrap(),
            title: "Rings & Shadows".to_owned(),
            title_raw: None,
            // Inline only, which is what the parser stores: no block element ever reaches here.
            explanation_html: "A ring, <a href=\"https://x/\">lit</a>.".to_owned(),
            explanation_text: "A ring, lit.".to_owned(),
            credits: vec![Credit {
                role: "Image Credit & Copyright".to_owned(),
                html: "<a href=\"https://x/\">A. Nother</a>".to_owned(),
                text: "A. Nother".to_owned(),
            }],
            has_copyright: true,
            license_url: None,
            tomorrow_teaser: None,
            keywords: Vec::new(),
            media: Media {
                kind: MediaKind::ImageJpg,
                url: Some("https://apod.nasa.gov/apod/image/2603/rings.jpg".to_owned()),
                hd_url: None,
                thumb_path: None,
                thumb_url: Some("/thumbs/2026/03/2026-03-05.webp".to_owned()),
                thumb_width: Some(480),
                thumb_height: Some(320),
            },
            extra_media: Vec::new(),
            legacy_media_url: None,
            first_stored_at: None,
            alt: None,
            authors: Vec::new(),
            provenance: apod_core::Provenance::LegacyOnly,
            source_url: "https://apod.nasa.gov/apod/ap260305.html".to_owned(),
            picture: None,
        }
    }

    #[test]
    fn a_title_with_an_ampersand_survives_as_an_entity() {
        let mut xml = String::new();
        push(&mut xml, 1, "title", "Rings & Shadows");
        assert_eq!(xml, "  <title>Rings &amp; Shadows</title>\n");
    }

    #[test]
    fn the_thumbnail_is_ours_and_carries_its_own_dimensions() {
        let html = content_html(BASE, &entry());

        assert!(
            html.contains("<img src=\"https://apod.example/thumbs/2026/03/2026-03-05.webp\""),
            "{html}"
        );
        assert!(html.contains("width=\"480\" height=\"320\""), "{html}");
        assert!(
            !html.contains("apod.nasa.gov/apod/image"),
            "the full frame is not hotlinked into the feed: {html}"
        );
    }

    #[test]
    fn the_attribution_travels_with_the_explanation() {
        let html = content_html(BASE, &entry());

        assert!(
            html.contains("A ring, <a href=\"https://x/\">lit</a>."),
            "{html}"
        );
        assert!(
            html.contains("<strong>Image Credit &amp; Copyright:</strong>"),
            "{html}"
        );
    }

    #[test]
    fn the_explanation_is_a_paragraph_of_its_own_not_loose_text() {
        let mut inline = entry();
        inline.explanation_html = "A ring, <i>lit</i>.".to_owned();

        let html = content_html(BASE, &inline);
        assert!(html.contains("<p>A ring, <i>lit</i>.</p>"), "{html}");
        assert!(
            !html.contains("</p>A ring"),
            "the explanation must not run straight out of the image paragraph: {html}"
        );
    }

    #[test]
    fn apod_is_named_next_to_the_picture_it_came_from() {
        let html = content_html(BASE, &entry());

        let image = html.find("<img").expect("the entry has a thumbnail");
        let name = html
            .find("Astronomy Picture of the Day")
            .expect("the name is in the item");

        assert!(name > image, "the name follows the picture: {html}");
        assert!(
            html.contains(&format!(
                "<a href=\"{APOD_HOME}\">Astronomy Picture of the Day</a>"
            )),
            "{html}"
        );
    }

    #[test]
    fn an_entry_without_a_thumbnail_still_has_content() {
        let mut bare = entry();
        bare.media.thumb_url = None;

        let html = content_html(BASE, &bare);
        assert!(!html.contains("<img"), "{html}");
        assert!(html.contains("A ring,"), "{html}");
    }

    #[test]
    fn an_entry_is_dated_at_the_publish_time_not_midnight_utc() {
        let publish = Publish {
            timezone: Tz::America__New_York,
            hour: 0,
            minute: 0,
        };

        assert_eq!(
            rfc3339(published_at(&publish, &entry())),
            "2026-03-05T05:00:00Z"
        );
    }
}
