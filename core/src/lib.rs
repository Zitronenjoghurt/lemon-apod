#[cfg(feature = "data-read")]
pub mod apod;
pub mod date;
#[cfg(feature = "data")]
pub mod db;
pub mod decode;
pub mod entry;
pub mod html;
pub mod media;
pub mod merge;
#[cfg(feature = "notify-data")]
pub mod notify;
pub mod parse;
pub mod quality;
#[cfg(feature = "rating")]
pub mod rating;
pub mod resource;
#[cfg(feature = "sky")]
pub mod sky;
pub mod text;

#[cfg(feature = "data-write")]
pub use apod::ApodWriter;
#[cfg(feature = "data-read")]
pub use apod::{
    AnchorCount, ApodError, ApodReader, ApodResult, Appearance, Changed, Cloze, ClozePiece, Deal,
    Filters, Fingerprint, GameEntry, HostCount, Listing, Order, Page, Picture, PictureAppearances,
    PictureFilters, PictureGroup, PictureOrder, PictureSummary, Resource, ResourceFilters,
    ResourceOrder, ResourceRef, ResourceRefs, SearchResults, Snippet, Stats, TextSummary, Timeline,
    Word, WordFilters, WordOrder, WordUse,
};
pub use date::ApodDate;
#[cfg(feature = "data")]
pub use db::{Access, Db, DbConfig, DbError, DbResult};
pub use entry::{ApodEntry, ApodSummary, Credit, Matched, Provenance, SearchHit};
pub use media::{KindFilter, Media, MediaKind, Thumb, ThumbSource};
pub use merge::{Divergence, Merged, merge};
pub use parse::{ParseError, parse_page};
pub use quality::{QualityWarning, quality_control};

pub const APOD_BASE_URL: &str = "https://apod.nasa.gov/apod/";

pub const PARSER_VERSION: u32 = 6;
