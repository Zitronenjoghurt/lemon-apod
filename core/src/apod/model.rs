use crate::date::ApodDate;
use crate::media::MediaKind;
use serde::Serialize;

#[derive(Debug, Clone, Default)]
pub struct Filters {
    pub from: Option<ApodDate>,
    pub to: Option<ApodDate>,
    pub kind: Option<MediaKind>,
    pub copyright: Option<bool>,
}

impl Filters {
    pub fn is_empty(&self) -> bool {
        self.from.is_none() && self.to.is_none() && self.kind.is_none() && self.copyright.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Asc,
    Desc,
}

#[derive(Debug, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<ApodDate>,
}

#[derive(Debug, Serialize)]
pub struct SearchResults {
    pub items: Vec<crate::entry::SearchHit>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct Stats {
    pub entries: i64,
    pub thumbnails: i64,
    pub first: Option<ApodDate>,
    pub latest: Option<ApodDate>,
    pub by_media_kind: Vec<KindCount>,
}

#[derive(Debug, Serialize)]
pub struct KindCount {
    pub kind: String,
    pub count: i64,
}
