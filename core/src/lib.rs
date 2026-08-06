pub mod date;
pub mod decode;
pub mod entry;
pub mod html;
pub mod media;
pub mod parse;
pub mod quality;

pub use date::ApodDate;
pub use entry::{ApodEntry, ApodSummary, SearchHit};
pub use media::{Media, MediaKind, ThumbSource};
pub use parse::{ParseError, parse_page};
pub use quality::{QualityWarning, quality_control};

pub const APOD_BASE_URL: &str = "https://apod.nasa.gov/apod/";

pub const PARSER_VERSION: u32 = 1;
