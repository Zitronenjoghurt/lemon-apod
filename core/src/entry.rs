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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_text: Option<String>,
    /// Whether the credit line claims copyright. Drives the per-entry attribution notice.
    pub has_copyright: bool,
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

impl ApodEntry {
    pub fn summary_text(&self, max_chars: usize) -> String {
        truncate_on_word_boundary(&self.explanation_text, max_chars)
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
