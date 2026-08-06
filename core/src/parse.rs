use crate::APOD_BASE_URL;
use crate::date::ApodDate;
use crate::decode;
use crate::entry::ApodEntry;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use std::sync::LazyLock;
use url::Url;

mod credit;
mod explanation;
mod media;
mod meta;
mod title;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("no title found")]
    TitleNotFound,
    #[error("no explanation found")]
    ExplanationNotFound,
}

pub fn parse_bytes(date: ApodDate, bytes: &[u8]) -> Result<ApodEntry, ParseError> {
    let (text, _) = decode::decode_html(bytes);
    parse_page(date, &text)
}

pub fn parse_page(date: ApodDate, raw: &str) -> Result<ApodEntry, ParseError> {
    let doc = Html::parse_document(raw);
    let base = page_base(date);

    let title = title::parse(&doc).ok_or(ParseError::TitleNotFound)?;
    let explanation = explanation::parse(&doc, &base).ok_or(ParseError::ExplanationNotFound)?;
    let credit = credit::parse(&doc, &base);
    let (media, extra_media) = media::parse(&doc, &base);

    Ok(ApodEntry {
        date,
        title,
        title_raw: title::raw_title(&doc),
        explanation_html: explanation.html,
        explanation_text: explanation.text,
        credit_html: credit.as_ref().map(|c| c.fragment.html.clone()),
        credit_text: credit.as_ref().map(|c| c.fragment.text.clone()),
        has_copyright: credit.as_ref().is_some_and(|c| c.has_copyright),
        tomorrow_teaser: meta::tomorrow_teaser(&doc),
        keywords: meta::keywords(&doc),
        media,
        extra_media,
        source_url: date.source_url(),
    })
}

fn page_base(date: ApodDate) -> Url {
    Url::parse(&date.source_url())
        .unwrap_or_else(|_| Url::parse(APOD_BASE_URL).expect("APOD_BASE_URL is a valid URL"))
}

static CONTAINERS: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("p, td, div, center, body").expect("static selector is valid")
});

fn find_container<'a>(doc: &'a Html, marker: &Regex) -> Option<ElementRef<'a>> {
    doc.select(&CONTAINERS)
        .filter_map(|el| {
            let text = el.text().collect::<String>();
            marker.is_match(&text).then_some((text.len(), el))
        })
        .min_by_key(|(len, _)| *len)
        .map(|(_, el)| el)
}
