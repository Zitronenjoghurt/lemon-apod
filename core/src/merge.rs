use crate::entry::{ApodEntry, Provenance};
use crate::media::MediaKind;

#[derive(Debug, Clone)]
pub struct Merged {
    pub entry: ApodEntry,
    pub divergences: Vec<Divergence>,
}

impl From<ApodEntry> for Merged {
    fn from(entry: ApodEntry) -> Self {
        Self {
            entry,
            divergences: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    pub field: &'static str,
    pub legacy: String,
    pub modern: String,
}

pub(crate) fn fold(value: &str) -> String {
    let mut out = String::with_capacity(value.len());

    for c in value.chars() {
        let c = match c {
            c if c.is_whitespace() => continue,
            '\'' | '\u{2018}' | '\u{2019}' => continue,
            '\u{2013}' | '\u{2014}' => '-',
            other => other,
        };
        if c == '-' && out.ends_with('-') {
            continue;
        }
        out.push(c);
    }

    out
}

fn differs(legacy: &str, modern: &str) -> bool {
    !legacy.is_empty() && !modern.is_empty() && fold(legacy) != fold(modern)
}

fn note(out: &mut Vec<Divergence>, field: &'static str, legacy: &str, modern: &str) {
    if differs(legacy, modern) {
        out.push(Divergence {
            field,
            legacy: legacy.to_owned(),
            modern: modern.to_owned(),
        });
    }
}

pub fn merge(legacy: Option<ApodEntry>, modern: Option<ApodEntry>) -> Option<Merged> {
    let provenance = Provenance::of(legacy.is_some(), modern.is_some())?;

    let (legacy, modern) = match (legacy, modern) {
        (Some(legacy), None) => return Some(sole(legacy, provenance).into()),
        (None, Some(modern)) => return Some(sole(modern, provenance).into()),
        (Some(legacy), Some(modern)) => (legacy, modern),
        (None, None) => unreachable!("Provenance::of already rejected two absent sides"),
    };

    let mut entry = legacy;
    let mut divergences = Vec::new();

    note(&mut divergences, "title", &entry.title, &modern.title);
    note(
        &mut divergences,
        "explanation_text",
        &entry.explanation_text,
        &modern.explanation_text,
    );

    if !entry.credits.is_empty() && !modern.credits.is_empty() {
        note(
            &mut divergences,
            "credit_text",
            entry.credit_text().as_deref().unwrap_or_default(),
            modern.credit_text().as_deref().unwrap_or_default(),
        );
        if entry.has_copyright != modern.has_copyright {
            divergences.push(Divergence {
                field: "has_copyright",
                legacy: entry.has_copyright.to_string(),
                modern: modern.has_copyright.to_string(),
            });
        }
    }

    note(
        &mut divergences,
        "license_url",
        entry.license_url.as_deref().unwrap_or_default(),
        modern.license_url.as_deref().unwrap_or_default(),
    );
    note(
        &mut divergences,
        "tomorrow_teaser",
        entry.tomorrow_teaser.as_deref().unwrap_or_default(),
        modern.tomorrow_teaser.as_deref().unwrap_or_default(),
    );

    if modern.media.kind != MediaKind::None {
        if entry.media.kind != MediaKind::None {
            if entry.media.kind != modern.media.kind {
                divergences.push(Divergence {
                    field: "media_kind",
                    legacy: entry.media.kind.to_string(),
                    modern: modern.media.kind.to_string(),
                });
            }
            note(
                &mut divergences,
                "media_url",
                entry.media.url.as_deref().unwrap_or_default(),
                modern.media.url.as_deref().unwrap_or_default(),
            );
        }

        entry.legacy_media_url = entry.media.url.clone();
        entry.media = modern.media;
        entry.extra_media = modern.extra_media;
    }

    if !modern.source_url.is_empty() {
        entry.source_url = modern.source_url;
    }

    entry.alt = modern.alt;
    entry.authors = modern.authors;
    entry.provenance = provenance;

    Some(Merged { entry, divergences })
}

fn sole(mut entry: ApodEntry, provenance: Provenance) -> ApodEntry {
    entry.provenance = provenance;
    entry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ApodDate;
    use crate::entry::Credit;
    use crate::media::Media;

    fn legacy() -> ApodEntry {
        ApodEntry {
            date: ApodDate::START,
            title: "Rubin's COSMOS field".into(),
            title_raw: Some("APOD: 2026 August 7 - Rubin's COSMOS field".into()),
            explanation_html: "The <b>survey</b> looked deep.".into(),
            explanation_text: "The survey looked deep.".into(),
            credits: vec![Credit {
                role: "Image Credit & Copyright".into(),
                html: "Jane Doe".into(),
                text: "Jane Doe".into(),
            }],
            has_copyright: true,
            license_url: None,
            tomorrow_teaser: Some("lion head space".into()),
            keywords: vec!["rubin".into()],
            media: Media::new(
                MediaKind::ImageJpg,
                Some("https://apod.nasa.gov/apod/image/2608/noirlab_1024.jpg".into()),
                Some("https://apod.nasa.gov/apod/image/2608/noirlab.jpg".into()),
            ),
            extra_media: Vec::new(),
            legacy_media_url: None,
            alt: None,
            authors: Vec::new(),
            provenance: Provenance::LegacyOnly,
            picture: None,
            source_url: ApodDate::START.source_url(),
        }
    }

    fn modern() -> ApodEntry {
        ApodEntry {
            title: "Rubin\u{2019}s Cosmos Field".into(),
            title_raw: None,
            media: Media::new(
                MediaKind::ImageJpg,
                Some("https://assets.science.nasa.gov/content/dam/noirlab.jpg".into()),
                Some("https://assets.science.nasa.gov/content/dam/noirlab.jpg".into()),
            ),
            alt: Some("A deep field of faint galaxies.".into()),
            authors: vec!["Robert Nemiroff".into(), "Jerry Bonnell".into()],
            keywords: Vec::new(),
            tomorrow_teaser: None,
            source_url: "https://science.nasa.gov/image-article/apod-2026-august-7/".into(),
            provenance: Provenance::ModernOnly,
            ..legacy()
        }
    }

    fn fields(divergences: &[Divergence]) -> Vec<&str> {
        divergences.iter().map(|d| d.field).collect()
    }

    #[test]
    fn two_absent_sides_are_not_an_entry() {
        assert!(merge(None, None).is_none());
    }

    #[test]
    fn one_side_is_the_entry_and_disagrees_with_nobody() {
        let Merged { entry, divergences } = merge(Some(legacy()), None).unwrap();
        assert_eq!(entry.provenance, Provenance::LegacyOnly);
        assert_eq!(entry.title, "Rubin's COSMOS field");
        assert!(divergences.is_empty());

        let Merged { entry, divergences } = merge(None, Some(modern())).unwrap();
        assert_eq!(entry.provenance, Provenance::ModernOnly);
        assert_eq!(entry.title, "Rubin\u{2019}s Cosmos Field");
        assert!(
            divergences.is_empty(),
            "a single source cannot disagree with anything"
        );
    }

    #[test]
    fn legacy_wins_every_content_field() {
        let Merged { entry, .. } = merge(Some(legacy()), Some(modern())).unwrap();

        assert_eq!(entry.title, "Rubin's COSMOS field");
        assert_eq!(entry.explanation_text, "The survey looked deep.");
        assert_eq!(entry.tomorrow_teaser.as_deref(), Some("lion head space"));
        assert_eq!(entry.keywords, ["rubin"]);
        assert_eq!(entry.provenance, Provenance::Both);
    }

    #[test]
    fn the_official_page_link_moves_to_the_host_that_survives() {
        let Merged { entry, divergences } = merge(Some(legacy()), Some(modern())).unwrap();

        assert_eq!(
            entry.source_url,
            "https://science.nasa.gov/image-article/apod-2026-august-7/"
        );
        assert!(
            !fields(&divergences).contains(&"source_url"),
            "a page that moved is not content the migration altered"
        );

        let mut blank = modern();
        blank.source_url = String::new();
        let Merged { entry, .. } = merge(Some(legacy()), Some(blank)).unwrap();
        assert_eq!(
            entry.source_url,
            ApodDate::START.source_url(),
            "a record with no link of its own leaves the legacy URL in place"
        );
    }

    #[test]
    fn media_comes_from_the_source_that_survives_and_the_old_url_is_kept() {
        let Merged { entry, divergences } = merge(Some(legacy()), Some(modern())).unwrap();

        assert_eq!(
            entry.media.url.as_deref(),
            Some("https://assets.science.nasa.gov/content/dam/noirlab.jpg")
        );
        assert_eq!(
            entry.legacy_media_url.as_deref(),
            Some("https://apod.nasa.gov/apod/image/2608/noirlab_1024.jpg"),
            "the dying URL is provenance, and nothing else records it"
        );
        assert!(fields(&divergences).contains(&"media_url"));
    }

    #[test]
    fn a_modern_record_with_no_media_never_blanks_the_entry() {
        let mut empty = modern();
        empty.media = Media::new(MediaKind::None, None, None);
        empty.credits.clear();
        empty.explanation_text.clear();
        empty.explanation_html.clear();

        let Merged { entry, divergences } = merge(Some(legacy()), Some(empty)).unwrap();
        assert_eq!(
            entry.media.url.as_deref(),
            Some("https://apod.nasa.gov/apod/image/2608/noirlab_1024.jpg")
        );
        assert_eq!(entry.legacy_media_url, None);
        assert_eq!(
            fields(&divergences),
            ["title"],
            "an empty record disagrees about nothing; it just has nothing"
        );
    }

    #[test]
    fn alt_text_and_authors_come_from_the_only_source_that_has_them() {
        let Merged { entry, divergences } = merge(Some(legacy()), Some(modern())).unwrap();

        assert_eq!(
            entry.alt.as_deref(),
            Some("A deep field of faint galaxies.")
        );
        assert_eq!(entry.authors, ["Robert Nemiroff", "Jerry Bonnell"]);
        assert!(
            !fields(&divergences).contains(&"alt"),
            "a field only one side has is absence, not disagreement"
        );
    }

    #[test]
    fn a_re_cased_title_is_a_divergence_and_keeps_both_spellings() {
        let Merged { divergences, .. } = merge(Some(legacy()), Some(modern())).unwrap();

        let title = divergences.iter().find(|d| d.field == "title").unwrap();
        assert_eq!(title.legacy, "Rubin's COSMOS field");
        assert_eq!(title.modern, "Rubin\u{2019}s Cosmos Field");
    }

    #[test]
    fn every_way_the_migration_mangles_an_apostrophe_folds_to_the_same_title() {
        for mangled in [
            "Kemble\u{2019}s Cascade",
            "Kembles Cascade",
            "Kemble s Cascade",
            "Kemble's Cascade",
        ] {
            let mut left = legacy();
            left.title = "Kemble's Cascade".into();
            let mut right = modern();
            right.title = mangled.into();

            let Merged { divergences, .. } = merge(Some(left), Some(right)).unwrap();
            assert!(
                !fields(&divergences).contains(&"title"),
                "{mangled} is the same title spelled the way NASA spelled it"
            );
        }
    }

    #[test]
    fn a_separator_normalised_to_an_en_dash_is_not_a_divergence() {
        let mut left = legacy();
        left.title = "M102: Edge-on Disk".into();
        let mut right = modern();
        right.title = "M102: Edge\u{2013}on Disk".into();

        let Merged { divergences, .. } = merge(Some(left), Some(right)).unwrap();
        assert!(!fields(&divergences).contains(&"title"));
    }

    #[test]
    fn punctuation_dropped_from_a_name_is_a_divergence() {
        let mut left = legacy();
        left.title = "Comet Tsuchinshan-ATLAS Approaches".into();
        let mut right = modern();
        right.title = "Comet Tsuchinshan ATLAS Approaches".into();

        let Merged { divergences, .. } = merge(Some(left), Some(right)).unwrap();
        assert!(
            fields(&divergences).contains(&"title"),
            "a hyphen dropped out of a comet's name changes the name"
        );
    }

    #[test]
    fn a_copyright_claim_the_modern_table_lost_is_recorded() {
        let mut right = modern();
        right.has_copyright = false;
        right.credits = vec![Credit {
            role: "Credit".into(),
            html: "Jane Doe".into(),
            text: "Jane Doe".into(),
        }];

        let Merged { entry, divergences } = merge(Some(legacy()), Some(right)).unwrap();
        assert!(entry.has_copyright, "legacy wins the claim itself");

        let claim = divergences
            .iter()
            .find(|d| d.field == "has_copyright")
            .unwrap();
        assert_eq!(
            (claim.legacy.as_str(), claim.modern.as_str()),
            ("true", "false")
        );
    }

    #[test]
    fn a_video_embed_that_lost_its_start_offset_is_recorded() {
        let mut left = legacy();
        left.media = Media::new(
            MediaKind::YouTube,
            Some("https://www.youtube.com/embed/UgxWkOXcdZU?si=GOlnR&t=23".into()),
            None,
        );
        let mut right = modern();
        right.media = Media::new(
            MediaKind::YouTube,
            Some("https://www.youtube.com/embed/UgxWkOXcdZU?feature=oembed".into()),
            None,
        );

        let Merged { entry, divergences } = merge(Some(left), Some(right)).unwrap();
        assert_eq!(
            entry.media.url.as_deref(),
            Some("https://www.youtube.com/embed/UgxWkOXcdZU?feature=oembed"),
            "the precedence table inverts media without exception"
        );
        assert!(
            entry
                .legacy_media_url
                .as_deref()
                .is_some_and(|url| url.contains("t=23")),
            "the deliberate start offset has to survive somewhere"
        );
        assert!(fields(&divergences).contains(&"media_url"));
    }

    #[test]
    fn keywords_the_modern_source_never_carried_are_not_a_disagreement() {
        let Merged { entry, divergences } = merge(Some(legacy()), Some(modern())).unwrap();
        assert_eq!(entry.keywords, ["rubin"]);
        assert!(!fields(&divergences).contains(&"keywords"));
        assert!(!fields(&divergences).contains(&"tomorrow_teaser"));
    }
}
