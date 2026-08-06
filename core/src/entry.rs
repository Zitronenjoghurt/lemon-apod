use crate::date::ApodDate;
use crate::media::Media;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApodEntry {
    pub date: ApodDate,
    pub title: String,
    /// The `<title>` tag verbatim. Stored purely so parser regressions are greppable, since a title
    /// that stops matching its own page title is the cheapest possible regression signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_raw: Option<String>,
    /// Sanitized inline HTML, links absolute. This is what the frontend renders.
    pub explanation_html: String,
    /// The same content as plain text. This is what search indexes.
    pub explanation_text: String,
    /// The attribution block, one entry per labelled role, in page order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub credits: Vec<Credit>,
    /// Whether a credit label claims copyright. Drives the per-entry attribution notice.
    pub has_copyright: bool,
    /// Where a credit label linked its licence, on the entries released under one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tomorrow_teaser: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    pub media: Media,
    /// Additional media on multi-image entries, in page order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_media: Vec<Media>,
    /// The page on apod.nasa.gov this was archived from.
    pub source_url: String,
}

/// One labelled line of an entry's attribution block, such as `Image Credit & Copyright` or
/// `Text`. APOD writes these as a single run of prose; keeping them apart is what lets the
/// frontend label them and lets `has_copyright` mean something precise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credit {
    /// The label as APOD wrote it, minus its colon: `Image Credit & Copyright`, `Music`.
    pub role: String,
    /// Sanitized inline HTML, links absolute.
    pub html: String,
    /// The same content as plain text.
    pub text: String,
}

impl ApodEntry {
    pub fn summary_text(&self, max_chars: usize) -> String {
        truncate_on_word_boundary(&self.explanation_text, max_chars)
    }

    /// Every credited name as one string. Not part of the API: this is what search indexes,
    /// where the role labels themselves are noise.
    pub fn credit_text(&self) -> Option<String> {
        (!self.credits.is_empty()).then(|| {
            self.credits
                .iter()
                .map(|credit| credit.text.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        })
    }

    pub fn to_summary(&self) -> ApodSummary {
        ApodSummary {
            date: self.date,
            title: self.title.clone(),
            media: self.media.clone(),
            has_copyright: self.has_copyright,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApodSummary {
    pub date: ApodDate,
    pub title: String,
    pub media: Media,
    pub has_copyright: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    #[serde(flatten)]
    pub entry: ApodSummary,
    pub snippet: String,
}

fn truncate_on_word_boundary(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }

    let cut: String = text.chars().take(max_chars).collect();
    let trimmed = match cut.rsplit_once(char::is_whitespace) {
        Some((head, _)) if !head.trim().is_empty() => head,
        _ => cut.as_str(),
    };

    format!("{}…", trimmed.trim_end_matches([',', '.', ';', ':', ' ']))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_without_splitting_words() {
        assert_eq!(
            truncate_on_word_boundary("one two three", 100),
            "one two three"
        );
        assert_eq!(truncate_on_word_boundary("one two three", 8), "one two…");
    }

    #[test]
    fn truncating_handles_multibyte_text() {
        let text = "Messier 31 — the Andromeda Galaxy";
        assert_eq!(truncate_on_word_boundary(text, 12), "Messier 31…");
    }
}
