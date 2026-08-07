#[cfg(feature = "data-read")]
pub mod apod;
pub mod date;
#[cfg(feature = "data")]
pub mod db;
pub mod decode;
pub mod entry;
pub mod html;
pub mod media;
pub mod parse;
pub mod quality;
pub mod resource;
pub mod text;

#[cfg(feature = "data-write")]
pub use apod::ApodWriter;
#[cfg(feature = "data-read")]
pub use apod::{
    ApodError, ApodReader, ApodResult, Filters, HostCount, Listing, Order, Page, Resource,
    ResourceFilters, ResourceOrder, ResourceRef, ResourceRefs, SearchResults, Snippet, Stats,
    TextSummary, Timeline, Word, WordFilters, WordOrder, WordUse,
};
pub use date::ApodDate;
#[cfg(feature = "data")]
pub use db::{Access, Db, DbConfig, DbError, DbResult};
pub use entry::{ApodEntry, ApodSummary, Credit, SearchHit};
pub use media::{KindFilter, Media, MediaKind, Thumb, ThumbSource};
pub use parse::{ParseError, parse_page};
pub use quality::{QualityWarning, quality_control};

pub const APOD_BASE_URL: &str = "https://apod.nasa.gov/apod/";

pub const PARSER_VERSION: u32 = 4;
