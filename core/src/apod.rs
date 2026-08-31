pub mod catalogue;
pub mod games;
pub mod insight;
pub mod model;
pub mod pictures;
pub mod query;
pub mod read;
pub mod reuse;
#[cfg(feature = "data-write")]
pub mod write;

pub use games::{Deal, GameEntry};
pub use model::{
    AnchorCount, Appearance, Changed, Cloze, ClozePiece, Coverage, EntryLength, FieldDivergence,
    Filters, HostCount, KindCount, LengthBucket, Listing, MonthCount, Order, Page, Picture,
    PictureAppearances, PictureFilters, PictureOrder, PictureSummary, Resource, ResourceFilters,
    ResourceOrder, ResourceRef, ResourceRefs, ResourceSummary, SearchResults, Stats, TextSummary,
    Timeline, Word, WordEntry, WordFilters, WordOrder, WordUse, YearCount, YearStats,
};
pub use pictures::{Fingerprint, PictureGroup};
pub use read::{ApodError, ApodReader, ApodResult, Snippet};
#[cfg(feature = "data-write")]
pub use write::ApodWriter;

pub const SCHEMA_VERSION: i64 = 6;
pub const MIN_SCHEMA_VERSION: i64 = 6;

pub(crate) const ENTRY_COLUMNS: &str = "date_id, title, title_raw, explanation_html, \
                                        explanation_text, credits, has_copyright, license_url, \
                                        tomorrow_teaser, keywords, media_kind, media_url, \
                                        media_hd_url, thumb_path, thumb_width, thumb_height, \
                                        source_url, picture_group, legacy_media_url, alt, \
                                        authors, provenance, first_stored_at";

pub(crate) const SUMMARY_COLUMNS: &str = "date_id, title, has_copyright, media_kind, media_url, \
                                          media_hd_url, thumb_path, thumb_width, thumb_height, \
                                          picture_group";

#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error(
        "{path} has never been migrated. The archiver creates and migrates it; \
         start the archiver first"
    )]
    Unmigrated { path: String },
    #[error(
        "{path} is at schema version {found}, but this build needs \
         {MIN_SCHEMA_VERSION} to {SCHEMA_VERSION}. \
         {}",
        if *.found > SCHEMA_VERSION {
            "The database is newer than this binary; upgrade it."
        } else {
            "The database is older than this binary; run the archiver to migrate it."
        }
    )]
    Unsupported { path: String, found: i64 },
}
