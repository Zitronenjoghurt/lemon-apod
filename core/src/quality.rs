use crate::entry::ApodEntry;
use crate::media::MediaKind;
use regex::Regex;
use std::fmt;
use std::sync::LazyLock;

const MIN_EXPLANATION_CHARS: usize = 80;
const MAX_TITLE_CHARS: usize = 200;
const MAX_ROLE_WORDS: usize = 4;

static LOOKS_LIKE_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<\s*/?[a-zA-Z]").unwrap());
static HREF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"href="([^"]*)""#).unwrap());
static TEASER_LABEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^tomorrow['\u{2019}]s\s+picture\b").unwrap());

fn has_relative_link(html: &str) -> bool {
    HREF.captures_iter(html).any(|caps| {
        let href = &caps[1];
        !["http://", "https://", "mailto:"]
            .iter()
            .any(|scheme| href.starts_with(scheme))
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QualityWarning {
    ContainsHtml,
    CreditMissing,
    CreditRoleSuspicious,
    EmptyField,
    ExplanationSuspiciouslyShort,
    LeadingWhitespace,
    MultiWhitespace,
    NoMedia,
    NonAbsoluteLink,
    TitleIsTeaserLabel,
    TitleMultiline,
    TitleSuspiciouslyLong,
    TrailingWhitespace,
    UnknownMediaKind,
}

impl fmt::Display for QualityWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::ContainsHtml => "contains_html",
            Self::CreditMissing => "credit_missing",
            Self::CreditRoleSuspicious => "credit_role_suspicious",
            Self::EmptyField => "empty_field",
            Self::ExplanationSuspiciouslyShort => "explanation_suspiciously_short",
            Self::LeadingWhitespace => "leading_whitespace",
            Self::MultiWhitespace => "multi_whitespace",
            Self::NoMedia => "no_media",
            Self::NonAbsoluteLink => "non_absolute_link",
            Self::TitleIsTeaserLabel => "title_is_teaser_label",
            Self::TitleMultiline => "title_multiline",
            Self::TitleSuspiciouslyLong => "title_suspiciously_long",
            Self::TrailingWhitespace => "trailing_whitespace",
            Self::UnknownMediaKind => "unknown_media_kind",
        };
        f.write_str(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualityIssue {
    pub warning: QualityWarning,
    pub field: &'static str,
}

impl fmt::Display for QualityIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.warning)
    }
}

pub fn quality_control(entry: &ApodEntry, attributed: Option<bool>) -> Vec<QualityIssue> {
    let mut issues = Vec::new();

    check_string(&mut issues, "title", &entry.title);
    if TEASER_LABEL.is_match(&entry.title) {
        push(&mut issues, QualityWarning::TitleIsTeaserLabel, "title");
    }
    if entry.title.lines().count() > 1 {
        push(&mut issues, QualityWarning::TitleMultiline, "title");
    }
    if entry.title.chars().count() > MAX_TITLE_CHARS {
        push(&mut issues, QualityWarning::TitleSuspiciouslyLong, "title");
    }

    check_string(&mut issues, "explanation", &entry.explanation_text);
    if !entry.explanation_text.is_empty()
        && entry.explanation_text.chars().count() < MIN_EXPLANATION_CHARS
    {
        push(
            &mut issues,
            QualityWarning::ExplanationSuspiciouslyShort,
            "explanation",
        );
    }
    if has_relative_link(&entry.explanation_html) {
        push(&mut issues, QualityWarning::NonAbsoluteLink, "explanation");
    }

    if entry.credits.is_empty() && attributed.unwrap_or(true) {
        push(&mut issues, QualityWarning::CreditMissing, "credit");
    }
    for credit in &entry.credits {
        check_string(&mut issues, "credit", &credit.text);
        if has_relative_link(&credit.html) {
            push(&mut issues, QualityWarning::NonAbsoluteLink, "credit");
        }
        // A role that ran on past its colon means the label vocabulary missed a word.
        if role_words(&credit.role) > MAX_ROLE_WORDS {
            push(&mut issues, QualityWarning::CreditRoleSuspicious, "credit");
        }
    }

    match entry.media.kind {
        MediaKind::None => push(&mut issues, QualityWarning::NoMedia, "media"),
        MediaKind::Other => push(&mut issues, QualityWarning::UnknownMediaKind, "media"),
        _ => {}
    }

    issues
}

fn check_string(issues: &mut Vec<QualityIssue>, field: &'static str, value: &str) {
    if value.is_empty() {
        push(issues, QualityWarning::EmptyField, field);
        return;
    }
    if value.starts_with(char::is_whitespace) {
        push(issues, QualityWarning::LeadingWhitespace, field);
    }
    if value.ends_with(char::is_whitespace) {
        push(issues, QualityWarning::TrailingWhitespace, field);
    }
    if has_double_whitespace(value) {
        push(issues, QualityWarning::MultiWhitespace, field);
    }
    if LOOKS_LIKE_TAG.is_match(value) {
        push(issues, QualityWarning::ContainsHtml, field);
    }
}

fn role_words(role: &str) -> usize {
    role.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty() && !word.eq_ignore_ascii_case("and"))
        .count()
}

fn has_double_whitespace(value: &str) -> bool {
    value
        .chars()
        .zip(value.chars().skip(1))
        .any(|(a, b)| a.is_whitespace() && b.is_whitespace())
}

fn push(issues: &mut Vec<QualityIssue>, warning: QualityWarning, field: &'static str) {
    issues.push(QualityIssue { warning, field });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ApodDate;
    use crate::entry::Credit;
    use crate::media::{Media, MediaKind};

    fn entry() -> ApodEntry {
        ApodEntry {
            date: ApodDate::START,
            title: "A Fine Title".into(),
            title_raw: None,
            explanation_html: "Some prose that is comfortably long enough to look like a real \
                               APOD explanation."
                .into(),
            explanation_text: "Some prose that is comfortably long enough to look like a real \
                               APOD explanation."
                .into(),
            credits: vec![Credit {
                role: "Image Credit".into(),
                html: "Someone".into(),
                text: "Someone".into(),
            }],
            has_copyright: false,
            license_url: None,
            tomorrow_teaser: None,
            keywords: Vec::new(),
            media: Media::new(MediaKind::ImageJpg, Some("https://x/y.jpg".into()), None),
            extra_media: Vec::new(),
            source_url: ApodDate::START.source_url(),
            picture: None,
        }
    }

    fn warnings(entry: &ApodEntry) -> Vec<QualityWarning> {
        quality_control(entry, None)
            .into_iter()
            .map(|i| i.warning)
            .collect()
    }

    #[test]
    fn a_clean_entry_has_no_warnings() {
        assert!(quality_control(&entry(), None).is_empty());
    }

    #[test]
    fn a_page_that_credits_nobody_is_not_a_missing_credit() {
        let mut entry = entry();
        entry.credits.clear();

        assert!(
            warnings(&entry).contains(&QualityWarning::CreditMissing),
            "with no page to consult, an absent credit is still worth reporting"
        );
        assert!(
            quality_control(&entry, Some(true))
                .iter()
                .any(|issue| issue.warning == QualityWarning::CreditMissing),
            "the page attributes somebody, so the parser missed it"
        );
        assert!(
            !quality_control(&entry, Some(false))
                .iter()
                .any(|issue| issue.warning == QualityWarning::CreditMissing),
            "the page attributes nobody, so there is nothing to find"
        );
    }

    #[test]
    fn flags_a_truncated_explanation() {
        let mut entry = entry();
        entry.explanation_text = "Too short.".into();
        assert!(warnings(&entry).contains(&QualityWarning::ExplanationSuspiciouslyShort));
    }

    #[test]
    fn flags_a_title_that_is_only_the_next_days_teaser_label() {
        let mut entry = entry();
        entry.title = "Tomorrow's picture:".into();
        assert!(warnings(&entry).contains(&QualityWarning::TitleIsTeaserLabel));

        entry.title = "Tomorrow\u{2019}s Picture".into();
        assert!(warnings(&entry).contains(&QualityWarning::TitleIsTeaserLabel));

        entry.title = "Tomorrow at the Observatory".into();
        assert!(
            !warnings(&entry).contains(&QualityWarning::TitleIsTeaserLabel),
            "a title that merely starts with the word is still a title"
        );
    }

    #[test]
    fn flags_surviving_markup_but_not_legitimate_angle_brackets() {
        let mut entry = entry();
        entry.title = "5 < 7 and 8 > 2".into();
        assert!(!warnings(&entry).contains(&QualityWarning::ContainsHtml));

        entry.title = "Broken <br> title".into();
        assert!(warnings(&entry).contains(&QualityWarning::ContainsHtml));
    }

    #[test]
    fn flags_a_relative_link_that_escaped_absolutisation() {
        let mut entry = entry();
        entry.explanation_html = r#"See <a href="ap240304.html">this</a>."#.into();
        assert!(warnings(&entry).contains(&QualityWarning::NonAbsoluteLink));
    }

    #[test]
    fn flags_missing_credit_and_media() {
        let mut entry = entry();
        entry.credits.clear();
        entry.media = Media::new(MediaKind::None, None, None);

        let found = warnings(&entry);
        assert!(found.contains(&QualityWarning::CreditMissing));
        assert!(found.contains(&QualityWarning::NoMedia));
    }

    #[test]
    fn flags_a_role_that_swallowed_the_name_after_it() {
        let mut entry = entry();
        entry.credits[0].role = "Image Credit and Processing and Text and Music and More".into();
        assert!(warnings(&entry).contains(&QualityWarning::CreditRoleSuspicious));
    }

    #[test]
    fn a_long_label_apod_really_writes_is_not_a_runaway() {
        for role in [
            "Image and Video Credit & Copyright",
            "Simulation Video & Text Credit",
            "Sound Image Credit & Copyright",
            "Digital Illustration Credit & Copyright",
        ] {
            let mut entry = entry();
            entry.credits[0].role = role.into();
            assert!(
                !warnings(&entry).contains(&QualityWarning::CreditRoleSuspicious),
                "{role} is a label APOD writes, not a parse that ran on"
            );
        }
    }

    #[test]
    fn an_embed_is_a_classification_not_a_gap() {
        let mut entry = entry();
        entry.media = Media::new(MediaKind::Embed, Some("https://x/panorama".into()), None);

        let found = warnings(&entry);
        assert!(!found.contains(&QualityWarning::UnknownMediaKind));
        assert!(!found.contains(&QualityWarning::NoMedia));
    }
}
