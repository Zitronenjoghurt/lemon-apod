use crate::date::ApodDate;
use crate::entry::{ApodEntry, Credit, Provenance};
use crate::html::{self, Options, collapse};
use crate::media::Media;
use crate::merge::fold;
use crate::quality::{QualityIssue, QualityWarning, issue};
use scraper::{ElementRef, Html, Selector};
use serde::Deserialize;
use std::sync::LazyLock;
use url::Url;

use super::{ParseError, credit, explanation, media, page_base, title};

const ORIGIN: &str = "assets.science.nasa.gov/content/dam/";
const RENDERER: &str = "assets.science.nasa.gov/dynamicimage/assets/";
const PLACEHOLDER: &str = "news-thumbnail.png";
const ALT_BOILERPLATE: &str = "see explanation.";
const ELLIPSIS: &str = "[\u{2026}]";

static HERO: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".wp-block-nasa-blocks-media-detail-hero").unwrap());
static DESCRIPTION: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".media-detail-hero__description").unwrap());
static META_ROW: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("table.media-detail-hero__meta-table tr").unwrap());
static TH: LazyLock<Selector> = LazyLock::new(|| Selector::parse("th").unwrap());
static TD: LazyLock<Selector> = LazyLock::new(|| Selector::parse("td").unwrap());
static IMG: LazyLock<Selector> = LazyLock::new(|| Selector::parse("img").unwrap());

#[derive(Debug, Default, Deserialize)]
struct Rendered {
    #[serde(default)]
    rendered: String,
}

#[derive(Debug, Deserialize)]
struct Record {
    #[serde(default)]
    title: Rendered,
    #[serde(default)]
    content: Rendered,
    #[serde(default)]
    link: String,
    #[serde(default)]
    featured_image: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct Modern {
    pub entry: ApodEntry,
    pub issues: Vec<QualityIssue>,
}

pub fn parse_json_bytes(date: ApodDate, bytes: &[u8]) -> Result<Modern, ParseError> {
    let record: Record = serde_json::from_slice(bytes)?;
    let base = page_base(date);
    let doc = Html::parse_document(&record.content.rendered);
    let hero = doc.select(&HERO).next().ok_or(ParseError::NoArticleBody)?;

    let named = title::modern(&record.title.rendered);
    if named.title.is_empty() {
        return Err(ParseError::TitleNotFound);
    }

    let body = hero
        .select(&DESCRIPTION)
        .next()
        .and_then(|el| explanation::from_element(el, &base))
        .ok_or(ParseError::ExplanationNotFound)?;

    let credits = credits(hero, &base);
    let (media, extra_media) = origin_media(hero, &base);

    let mut issues = Vec::new();
    if named.date.is_some_and(|titled| titled != date) {
        issues.push(issue(QualityWarning::TitleDateMismatch, "title"));
    }
    if let Some(caption) = caption(&record.featured_image)
        && !says_the_same(&caption, &body.text)
    {
        issues.push(issue(QualityWarning::ExplanationMismatch, "explanation"));
    }

    let entry = ApodEntry {
        date,
        title: named.title,
        title_raw: (!record.title.rendered.is_empty()).then(|| decode(&record.title.rendered)),
        explanation_html: body.html,
        explanation_text: body.text,
        has_copyright: credits.as_ref().is_some_and(|c| c.has_copyright),
        license_url: credits.as_ref().and_then(|c| c.license_url.clone()),
        credits: credits.map(|c| c.segments).unwrap_or_default(),
        tomorrow_teaser: None,
        keywords: Vec::new(),
        media,
        extra_media,
        legacy_media_url: None,
        first_stored_at: None,
        alt: alt(hero),
        authors: authors(hero),
        provenance: Provenance::ModernOnly,
        source_url: match record.link.is_empty() {
            true => date.source_url(),
            false => record.link,
        },
        picture: None,
    };

    Ok(Modern { entry, issues })
}

fn origin_url(url: &str) -> Option<String> {
    let url = match url.contains(RENDERER) || url.contains(ORIGIN) {
        true => {
            let rewritten = url.replacen(RENDERER, ORIGIN, 1);
            rewritten
                .split(['?', '#'])
                .next()
                .unwrap_or(&rewritten)
                .to_owned()
        }
        false => url.to_owned(),
    };

    let path = url.split(['?', '#']).next().unwrap_or(&url);
    (!path.ends_with(PLACEHOLDER)).then_some(url)
}

fn origin_media(hero: ElementRef<'_>, base: &Url) -> (Media, Vec<Media>) {
    let (first, rest) = media::parse_in(hero, base);

    let single = |found: Media| {
        origin_url(found.url.as_deref()?).map(|url| Media::new(found.kind, Some(url), None))
    };

    let extra: Vec<Media> = rest.into_iter().filter_map(single).collect();
    match single(first) {
        Some(media) => (media, extra),
        None => match extra.split_first() {
            Some((first, rest)) => (first.clone(), rest.to_vec()),
            None => (Media::default(), Vec::new()),
        },
    }
}

fn credits(hero: ElementRef<'_>, base: &Url) -> Option<credit::Credits> {
    let mut segments: Vec<Credit> = Vec::new();
    let mut has_copyright = false;
    let mut license_url = None;

    for row in hero.select(&META_ROW) {
        let (Some(header), Some(cell)) = (row.select(&TH).next(), row.select(&TD).next()) else {
            continue;
        };
        let label = collapse(&header.text().collect::<String>());
        if !is_credit_label(&label) {
            continue;
        }

        match credit::from_element(cell, base) {
            Some(found) => {
                has_copyright |= found.has_copyright;
                license_url = license_url.or(found.license_url);
                segments.extend(found.segments);
            }
            None => {
                let role = label.trim_end_matches(':').trim().to_owned();
                let Some(value) = html::sanitize(cell, base, &Options::default())
                    .filter(|fragment| !fragment.is_empty())
                else {
                    continue;
                };
                has_copyright |= role.to_ascii_lowercase().contains("copyright");
                segments.push(Credit {
                    role,
                    html: value.html,
                    text: value.text,
                });
            }
        }
    }

    (!segments.is_empty()).then_some(credit::Credits {
        segments,
        has_copyright,
        license_url,
    })
}

fn is_credit_label(label: &str) -> bool {
    let label = label.to_ascii_lowercase();
    ["credit", "copyright", "courtesy", "hat tip"]
        .iter()
        .any(|word| label.contains(word))
}

fn authors(hero: ElementRef<'_>) -> Vec<String> {
    hero.select(&META_ROW)
        .filter_map(|row| {
            let header = row.select(&TH).next()?;
            let label = collapse(&header.text().collect::<String>()).to_ascii_lowercase();
            label
                .starts_with("authors")
                .then(|| row.select(&TD).next())?
        })
        .flat_map(|cell| {
            collapse(&cell.text().collect::<String>())
                .split(['&', ','])
                .map(|name| collapse(name.split('(').next().unwrap_or(name)))
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn alt(hero: ElementRef<'_>) -> Option<String> {
    let raw = hero.select(&IMG).find_map(|img| img.value().attr("alt"))?;
    let text = collapse(&decode(raw));

    (!text.is_empty() && !text.to_ascii_lowercase().starts_with(ALT_BOILERPLATE)).then_some(text)
}

fn caption(featured_image: &serde_json::Value) -> Option<String> {
    let raw = featured_image.get("caption")?.as_str()?;
    let text = collapse(&decode(raw));
    (!text.is_empty()).then_some(text)
}

fn says_the_same(caption: &str, explanation: &str) -> bool {
    let caption = explanation::strip_label(caption);
    let truncated = caption.contains(ELLIPSIS);
    let caption = fold(caption.split(ELLIPSIS).next().unwrap_or(caption));
    let explanation = fold(explanation);

    match truncated {
        true => explanation.starts_with(&caption),
        false => explanation == caption,
    }
}

fn decode(raw: &str) -> String {
    Html::parse_fragment(raw)
        .root_element()
        .text()
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::MediaKind;

    fn date() -> ApodDate {
        ApodDate::from_ymd(2026, 8, 25).unwrap()
    }

    const RENDERED: &str = "https://assets.science.nasa.gov/dynamicimage/assets/science/cds/apod/apod/2026/august/EarthShadow_Martin_4000.jpg";
    const ORIGINAL: &str = "https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2026/august/EarthShadow_Martin_4000.jpg";

    fn record(hero: &str) -> String {
        captioned(hero, "")
    }

    fn captioned(hero: &str, caption: &str) -> String {
        let content = format!(
            r#"<div class="hds-module alignfull wp-block-nasa-blocks-secondary-navigation">
               <a href="https://science.nasa.gov/apod/">Today&#8217;s APOD</a></div>
               <div class="hds-media-detail-hero hds-module alignwide
                wp-block-nasa-blocks-media-detail-hero">{hero}</div>
               <div class="hds-module alignwide wp-block-nasa-blocks-related-link">
               <a href="https://science.nasa.gov/apod/random-apod/">Random APOD Generator</a></div>"#
        );
        serde_json::json!({
            "title": {"rendered": "APOD: 2026 August 25 &#8211; Earth&#8217;s Shadow Visualized"},
            "content": {"rendered": content},
            "link": "https://science.nasa.gov/image-article/apod-2026-august-25-earths-shadow/",
            "featured_image": {"caption": caption}
        })
        .to_string()
    }

    fn hero() -> String {
        format!(
            r#"
        <div class="media-detail-hero__media">
          <figure><a href="{RENDERED}?w=4000&#038;h=5000&#038;fit=clip">
            <img width="4000" src="{RENDERED}?w=4000&amp;h=5000&amp;fit=clip"
            alt="Many images of Earth&#039;s Moon are shown each partly eclipsed."></a></figure>
        </div>
        <h1 class="display-48">Earth&#039;s Shadow Visualized</h1>
        <p class="p-md margin-0 media-detail-hero__description">
          <strong>Explanation:</strong> What&#8217;s creating this giant hole in space? It&#8217;s
          <a href="ap080820.html" rel="noopener">Earth&#8217;s shadow</a>.<br><br>
          <strong>APOD&#8217;s main NASA site is
          <a href="https://asterisk.apod.com/viewtopic.php?t=45023">moving</a> </strong>:
          From <a href="https://apod.nasa.gov/">apod.nasa.gov</a> to
          <a href="https://science.nasa.gov/apod">science.nasa.gov/apod</a><br>
          <strong>Tomorrow&#8217;s picture: </strong><a href="fap/ap260826.html">lion head</a>
        </p>
        <table class="media-detail-hero__meta-table width-full"><tbody>
          <tr class="media-detail-hero__meta-row"><th scope="row" class="label">Date</th>
            <td class="p-sm">August 25, 2026</td></tr>
          <tr class="media-detail-hero__meta-row"><th scope="row" class="label">Credit</th>
            <td class="p-sm">Image Credit &amp;
              <a href="https://apod.nasa.gov/apod/lib/about_apod.html#srapply">Copyright</a>:
              <a href="https://www.elon.edu/u/directory/profile/tmartin24/">Tim Martin</a></td></tr>
          <tr class="media-detail-hero__meta-row"><th scope="row" class="label">Authors &amp;
            editors:</th>
            <td class="p-sm">Robert Nemiroff ( MTU ) &amp; Jerry Bonnell ( UMCP )</td></tr>
          <tr class="media-detail-hero__meta-row"><th scope="row" class="label">A service of:</th>
            <td class="p-sm"><a href="https://www.nasa.gov/">NASA</a> / GSFC</td></tr>
        </tbody></table>"#
        )
    }

    const CREDIT_CELL: &str = r#"<td class="p-sm">Image Credit &amp;
              <a href="https://apod.nasa.gov/apod/lib/about_apod.html#srapply">Copyright</a>:
              <a href="https://www.elon.edu/u/directory/profile/tmartin24/">Tim Martin</a></td>"#;

    fn parsed(hero: &str) -> Modern {
        parse_json_bytes(date(), record(hero).as_bytes()).expect("the fixture is a readable record")
    }

    fn warnings(modern: &Modern) -> Vec<QualityWarning> {
        modern.issues.iter().map(|issue| issue.warning).collect()
    }

    #[test]
    fn reads_a_whole_record() {
        let modern = parsed(&hero());
        let entry = &modern.entry;

        assert_eq!(entry.title, "Earth\u{2019}s Shadow Visualized");
        assert_eq!(entry.provenance, Provenance::ModernOnly);
        assert_eq!(
            entry.source_url,
            "https://science.nasa.gov/image-article/apod-2026-august-25-earths-shadow/"
        );
        assert_eq!(
            entry.alt.as_deref(),
            Some("Many images of Earth's Moon are shown each partly eclipsed.")
        );
        assert_eq!(entry.authors, ["Robert Nemiroff", "Jerry Bonnell"]);
        assert!(warnings(&modern).is_empty());
    }

    #[test]
    fn the_explanation_stops_before_the_move_notice_and_the_teaser() {
        let entry = parsed(&hero()).entry;

        assert_eq!(
            entry.explanation_text,
            "What\u{2019}s creating this giant hole in space? It\u{2019}s Earth\u{2019}s shadow."
        );
        assert!(
            !entry.explanation_html.contains("moving"),
            "the migration notice runs below the prose, not inside it: {}",
            entry.explanation_html
        );
        assert!(!entry.explanation_html.contains("lion head"));
    }

    #[test]
    fn a_relative_link_resolves_against_the_apod_base_for_both_sources() {
        assert!(
            parsed(&hero())
                .entry
                .explanation_html
                .contains(r#"<a href="https://apod.nasa.gov/apod/ap080820.html">"#),
            "decision 9: hrefs resolve the same way whichever file they came out of"
        );
    }

    #[test]
    fn a_paragraph_that_never_got_its_label_is_still_the_explanation() {
        let hero = hero().replace("<strong>Explanation:</strong> ", "");
        let entry = parsed(&hero).entry;

        assert!(
            entry.explanation_text.starts_with("What\u{2019}s creating"),
            "one record in the corpus is missing the label and its prose is not lost: {}",
            entry.explanation_text
        );
    }

    #[test]
    fn the_renderer_url_is_rewritten_to_the_origin_and_loses_its_sizing() {
        let entry = parsed(&hero()).entry;

        assert_eq!(entry.media.kind, MediaKind::ImageJpg);
        assert_eq!(entry.media.url.as_deref(), Some(ORIGINAL));
        assert_eq!(
            entry.media.hd_url, None,
            "the modern record names one file per image and it is already the original"
        );
    }

    #[test]
    fn the_generic_placeholder_is_never_filed_as_an_apod_picture() {
        let hero = hero().replace("EarthShadow_Martin_4000.jpg", "misc/news-thumbnail.png");
        let entry = parsed(&hero).entry;

        assert_eq!(entry.media.kind, MediaKind::None);
        assert_eq!(entry.media.url, None);
    }

    #[test]
    fn the_copyright_claim_is_in_the_cell_rather_than_the_header() {
        let entry = parsed(&hero()).entry;

        assert_eq!(entry.credits.len(), 1);
        assert_eq!(entry.credits[0].role, "Image Credit & Copyright");
        assert_eq!(entry.credits[0].text, "Tim Martin");
        assert!(
            entry.has_copyright,
            "the header says only `Credit`, and believing it loses the claim on most records"
        );
    }

    #[test]
    fn a_cell_holding_a_bare_name_falls_back_to_the_header_for_its_role() {
        let hero = hero().replace(CREDIT_CELL, r#"<td class="p-sm">Monica Mesa</td>"#);
        let entry = parsed(&hero).entry;

        assert_eq!(entry.credits.len(), 1);
        assert_eq!(entry.credits[0].role, "Credit");
        assert_eq!(entry.credits[0].text, "Monica Mesa");
        assert!(!entry.has_copyright);
    }

    #[test]
    fn a_header_that_does_claim_copyright_is_believed_when_the_cell_says_nothing() {
        let hero = hero()
            .replace(
                r#"<th scope="row" class="label">Credit</th>"#,
                r#"<th scope="row" class="label">Credit &amp; Copyright:</th>"#,
            )
            .replace(
                CREDIT_CELL,
                r#"<td class="p-sm">Juan Pablo Casta&#241;eda</td>"#,
            );
        let entry = parsed(&hero).entry;

        assert_eq!(entry.credits[0].role, "Credit & Copyright");
        assert_eq!(entry.credits[0].text, "Juan Pablo Casta\u{f1}eda");
        assert!(entry.has_copyright);
    }

    #[test]
    fn the_service_and_author_rows_are_not_attribution_for_the_picture() {
        let entry = parsed(&hero()).entry;
        assert!(
            entry.credits.iter().all(|c| c.text != "NASA / GSFC"),
            "the service line credits the site, not the image"
        );
    }

    #[test]
    fn boilerplate_alt_text_describes_the_site_and_is_not_stored() {
        for boilerplate in [
            "See Explanation. Clicking on the picture will download the highest resolution \
             version available.",
            "See Explanation. Clicking on the picture will download and animated gif.",
        ] {
            let hero = hero().replace(
                "Many images of Earth&#039;s Moon are shown each partly eclipsed.",
                boilerplate,
            );
            assert_eq!(parsed(&hero).entry.alt, None, "{boilerplate}");
        }
    }

    #[test]
    fn a_record_with_no_article_body_is_a_failure_rather_than_an_empty_entry() {
        let bare = serde_json::json!({
            "title": {"rendered": "APOD: 2024 October 23 &#8211; Caught"},
            "content": {"rendered": "<div class=\"wp-block-nasa-blocks-related-link\">x</div>"},
            "featured_image": {"file": "news-thumbnail.png", "caption": ""}
        })
        .to_string();

        assert!(matches!(
            parse_json_bytes(date(), bare.as_bytes()),
            Err(ParseError::NoArticleBody)
        ));
    }

    #[test]
    fn something_that_is_not_a_record_is_not_silently_an_empty_entry() {
        assert!(matches!(
            parse_json_bytes(date(), b"not json at all"),
            Err(ParseError::NotJson(_))
        ));
    }

    #[test]
    fn a_video_entry_keeps_the_embed_the_page_points_at() {
        let hero = hero().replace(
            &format!(
                r#"<figure><a href="{RENDERED}?w=4000&#038;h=5000&#038;fit=clip">
            <img width="4000" src="{RENDERED}?w=4000&amp;h=5000&amp;fit=clip"
            alt="Many images of Earth&#039;s Moon are shown each partly eclipsed."></a></figure>"#
            ),
            r#"<iframe src="https://www.youtube.com/embed/UgxWkOXcdZU?feature=oembed"></iframe>"#,
        );
        let entry = parsed(&hero).entry;

        assert_eq!(entry.media.kind, MediaKind::YouTube);
        assert_eq!(
            entry.media.url.as_deref(),
            Some("https://www.youtube.com/embed/UgxWkOXcdZU?feature=oembed"),
            "a query string off the asset host is the page's, not ours to edit"
        );
        assert_eq!(entry.alt, None);
    }

    #[test]
    fn a_title_naming_a_date_the_key_contradicts_is_warned_about_and_never_overrides_it() {
        let json =
            record(&hero()).replace("APOD: 2026 August 25 &#8211;", "APOD: 2007 July 16 &#8211;");
        let modern = parse_json_bytes(date(), json.as_bytes()).unwrap();

        assert_eq!(modern.entry.date, date());
        assert_eq!(warnings(&modern), [QualityWarning::TitleDateMismatch]);
    }

    #[test]
    fn a_title_that_never_carried_a_date_is_not_a_disagreement_about_one() {
        let json = record(&hero()).replace(
            "APOD: 2026 August 25 &#8211; Earth&#8217;s Shadow Visualized",
            "Albert Einstein: 1879 - 1955",
        );
        let modern = parse_json_bytes(date(), json.as_bytes()).unwrap();

        assert_eq!(modern.entry.title, "Albert Einstein: 1879 - 1955");
        assert!(
            warnings(&modern).is_empty(),
            "the pre-prefix era names no date, and absence is not disagreement"
        );
    }

    #[test]
    fn a_caption_that_contradicts_the_prose_is_worth_knowing_about() {
        let check = |caption: &str| {
            let json = captioned(&hero(), caption);
            warnings(&parse_json_bytes(date(), json.as_bytes()).unwrap())
        };

        assert!(
            check("What\u{2019}s creating this giant hole in space? It\u{2019}s Earth\u{2019}s shadow.")
                .is_empty(),
            "the caption is the same prose with its links stripped"
        );
        assert!(
            check("What\u{2019}s creating this giant hole in space? [\u{2026}]").is_empty(),
            "a caption WordPress cut short can only say how the prose starts"
        );
        assert_eq!(
            check("An entirely different picture."),
            [QualityWarning::ExplanationMismatch]
        );
    }
}
