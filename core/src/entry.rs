use crate::date::ApodDate;
use crate::media::{Media, ThumbSource};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

const DECOMMISSIONED: [&str; 3] = ["apod.nasa.gov", "www.apod.nasa.gov", "antwrp.gsfc.nasa.gov"];

pub fn is_decommissioned(url: &str) -> bool {
    let host = url.split_once("//").map_or(url, |(_, rest)| rest);
    DECOMMISSIONED.iter().any(|dead| host.starts_with(dead))
}

pub fn decommissioned_sql(column: &str) -> String {
    DECOMMISSIONED
        .iter()
        .flat_map(|dead| {
            ["http", "https"].map(|scheme| format!("{column} LIKE '{scheme}://{dead}/%'"))
        })
        .collect::<Vec<_>>()
        .join(" OR ")
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    #[default]
    LegacyOnly,
    ModernOnly,
    Both,
}

impl Provenance {
    pub fn of(legacy: bool, modern: bool) -> Option<Self> {
        match (legacy, modern) {
            (true, true) => Some(Self::Both),
            (true, false) => Some(Self::LegacyOnly),
            (false, true) => Some(Self::ModernOnly),
            (false, false) => None,
        }
    }

    pub fn has_legacy(self) -> bool {
        matches!(self, Self::LegacyOnly | Self::Both)
    }

    pub fn has_modern(self) -> bool {
        matches!(self, Self::ModernOnly | Self::Both)
    }
}

impl fmt::Display for Provenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LegacyOnly => "legacy_only",
            Self::ModernOnly => "modern_only",
            Self::Both => "both",
        })
    }
}

impl FromStr for Provenance {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "legacy_only" => Self::LegacyOnly,
            "modern_only" => Self::ModernOnly,
            "both" => Self::Both,
            _ => return Err(()),
        })
    }
}

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
    /// Where the legacy page pointed its own media, kept once the modern origin took over
    /// `media`. Provenance only: the host it names is decommissioned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_media_url: Option<String>,
    /// Descriptive alt text, which only the modern source has, and only where it is not the
    /// boilerplate every older record carries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    /// The authors and editors credited for the entry itself, as distinct from the image.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    /// Which archived files this entry was built from.
    #[serde(default)]
    pub provenance: Provenance,
    /// The page this was archived from.
    pub source_url: String,
    /// Set when this entry's picture ran on more than one date. The value is the date it first
    /// ran, which is also how the picture is addressed. Filled in by readers, not by the parser:
    /// whether a picture is a rerun is a fact about the archive, not about the page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<ApodDate>,
    /// When this date first landed in the index, as opposed to when it was last parsed. What
    /// [`ApodEntry::settling`] measures its patience against. `None` on a row written before the
    /// archive started recording it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_stored_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unfinished {
    Merge,
    Thumbnail,
}

impl Unfinished {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Merge => "the record on science.nasa.gov",
            Self::Thumbnail => "the thumbnail",
        }
    }
}

impl fmt::Display for Unfinished {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub const SETTLE: Duration = Duration::from_secs(30 * 60);

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

    pub fn unfinished(&self) -> Option<Unfinished> {
        if !self.provenance.has_modern() {
            return Some(Unfinished::Merge);
        }

        let wants_thumbnail = !matches!(self.media.thumb_source(), ThumbSource::None);
        (wants_thumbnail && self.media.thumb_path.is_none()).then_some(Unfinished::Thumbnail)
    }

    pub fn settling(&self, grace: Duration, now: DateTime<Utc>) -> Option<Unfinished> {
        let unfinished = self.unfinished()?;
        let stored = self.first_stored_at?;
        let grace = TimeDelta::from_std(grace).ok()?;

        (now - stored < grace).then_some(unfinished)
    }

    pub fn official_url(&self) -> Option<&str> {
        let host = self
            .source_url
            .split_once("//")
            .map_or(self.source_url.as_str(), |(_, rest)| rest);

        DECOMMISSIONED
            .iter()
            .all(|dead| !host.starts_with(dead))
            .then_some(self.source_url.as_str())
    }

    pub fn to_summary(&self) -> ApodSummary {
        ApodSummary {
            date: self.date,
            title: self.title.clone(),
            media: self.media.clone(),
            has_copyright: self.has_copyright,
            picture: self.picture,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApodSummary {
    pub date: ApodDate,
    pub title: String,
    pub media: Media,
    pub has_copyright: bool,
    /// See [`ApodEntry::picture`]. What lets a listing mark a rerun without asking per card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<ApodDate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    #[serde(flatten)]
    pub entry: ApodSummary,
    pub snippet: String,
    pub matched: Matched,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Matched {
    pub title: bool,
    pub explanation: bool,
    pub credit: bool,
    pub keywords: bool,
}

impl Matched {
    pub fn only_beyond_the_prose(&self) -> bool {
        !self.explanation && !self.title && (self.credit || self.keywords)
    }
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

    fn at(source_url: &str) -> ApodEntry {
        ApodEntry {
            date: ApodDate::START,
            title: String::new(),
            title_raw: None,
            explanation_html: String::new(),
            explanation_text: String::new(),
            credits: Vec::new(),
            has_copyright: false,
            license_url: None,
            tomorrow_teaser: None,
            keywords: Vec::new(),
            media: Media::default(),
            extra_media: Vec::new(),
            legacy_media_url: None,
            first_stored_at: None,
            alt: None,
            authors: Vec::new(),
            provenance: Provenance::Both,
            source_url: source_url.to_owned(),
            picture: None,
        }
    }

    fn arrived(minutes_ago: i64) -> (ApodEntry, chrono::DateTime<Utc>) {
        let now = chrono::Utc::now();
        let mut entry = at("https://science.nasa.gov/image-article/apod/apod-x/");
        entry.media = Media::new(
            crate::media::MediaKind::ImageJpg,
            Some("https://assets.science.nasa.gov/a.jpg".into()),
            None,
        );
        entry.first_stored_at = Some(now - TimeDelta::minutes(minutes_ago));
        (entry, now)
    }

    #[test]
    fn an_entry_only_the_legacy_page_has_yet_is_waiting_on_the_merge() {
        let (mut half, _) = arrived(1);
        half.provenance = Provenance::LegacyOnly;
        half.media.thumb_path = Some("1995/06/1995-06-16.webp".into());

        assert_eq!(half.unfinished(), Some(Unfinished::Merge));

        half.provenance = Provenance::Both;
        assert_eq!(
            half.unfinished(),
            None,
            "both records in and the picture made leaves nothing outstanding"
        );
    }

    #[test]
    fn an_entry_whose_thumbnail_has_not_been_made_yet_is_waiting_on_it() {
        let (coming, _) = arrived(1);
        assert_eq!(coming.unfinished(), Some(Unfinished::Thumbnail));

        let mut made = coming.clone();
        made.media.thumb_path = Some("1995/06/1995-06-16.webp".into());
        assert_eq!(made.unfinished(), None);
    }

    #[test]
    fn an_entry_that_can_never_have_a_thumbnail_is_never_waiting_on_one() {
        let (mut nothing, _) = arrived(1);
        nothing.media = Media::default();
        assert_eq!(
            nothing.unfinished(),
            None,
            "no picture is coming, so waiting would only mean publishing it late"
        );

        let (mut headless, _) = arrived(1);
        headless.media = Media::new(crate::media::MediaKind::VideoMp4, None, None);
        assert_eq!(
            headless.unfinished(),
            None,
            "a video with no url has no frame to take, and the archive cannot invent one"
        );
    }

    #[test]
    fn patience_runs_out_rather_than_holding_an_entry_back_all_day() {
        let (fresh, now) = arrived(2);
        assert_eq!(fresh.settling(SETTLE, now), Some(Unfinished::Thumbnail));

        let (stale, now) = arrived(31);
        assert_eq!(
            stale.settling(SETTLE, now),
            None,
            "past the grace it goes out as it stands, picture or no picture"
        );
    }

    #[test]
    fn an_entry_with_no_recorded_arrival_is_published_rather_than_held_for_ever() {
        let (mut old, now) = arrived(1);
        old.first_stored_at = None;

        assert_eq!(old.unfinished(), Some(Unfinished::Thumbnail));
        assert_eq!(
            old.settling(SETTLE, now),
            None,
            "a row from before the archive recorded arrivals has no clock to wait against, and a \
             missing clock must not become an indefinite silence"
        );
    }

    #[test]
    fn only_a_page_on_a_host_that_survives_is_offered_as_the_official_one() {
        assert_eq!(
            at("https://science.nasa.gov/image-article/apod/apod-x/").official_url(),
            Some("https://science.nasa.gov/image-article/apod/apod-x/")
        );

        for dead in [
            "https://apod.nasa.gov/apod/ap950616.html",
            "http://apod.nasa.gov/apod/ap950616.html",
            "https://www.apod.nasa.gov/apod/ap950616.html",
            "https://antwrp.gsfc.nasa.gov/apod/ap950616.html",
        ] {
            assert_eq!(at(dead).official_url(), None, "{dead}");
        }

        assert_eq!(
            at("https://science.nasa.gov/apod.nasa.gov/x").official_url(),
            Some("https://science.nasa.gov/apod.nasa.gov/x"),
            "the dead host has to be the host, not a substring of the path"
        );
    }

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
