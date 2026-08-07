use crate::date::ApodDate;
use crate::media::KindFilter;
use serde::Serialize;

#[derive(Debug, Clone, Default)]
pub struct Filters {
    pub from: Option<ApodDate>,
    pub to: Option<ApodDate>,
    pub kind: Option<KindFilter>,
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
pub struct Listing<T> {
    pub items: Vec<T>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct Stats {
    pub entries: i64,
    pub thumbnails: i64,
    pub first: Option<ApodDate>,
    pub latest: Option<ApodDate>,
    pub by_media_kind: Vec<KindCount>,
    pub copyright: i64,
    pub text: TextSummary,
    pub resources: ResourceSummary,
}

#[derive(Debug, Serialize)]
pub struct KindCount {
    pub kind: String,
    pub count: i64,
}

#[derive(Debug, Default, Serialize)]
pub struct TextSummary {
    pub measured: i64,
    pub total_words: i64,
    pub distinct_words: i64,
    pub avg_words: f64,
    pub median_words: i64,
    pub min_words: i64,
    pub max_words: i64,
    pub avg_unique_words: f64,
    pub avg_chars: f64,
    pub avg_sentences: f64,
    pub avg_words_per_sentence: f64,
    pub avg_links: f64,
    pub used_once: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortest: Option<EntryLength>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longest: Option<EntryLength>,
}

#[derive(Debug, Serialize)]
pub struct EntryLength {
    pub date: ApodDate,
    pub title: String,
    pub word_count: i64,
}

#[derive(Debug, Default, Serialize)]
pub struct ResourceSummary {
    pub resources: i64,
    pub hosts: i64,
    pub references: i64,
    pub referenced_once: i64,
}

#[derive(Debug, Serialize)]
pub struct Timeline {
    pub years: Vec<YearStats>,
}

#[derive(Debug, Serialize)]
pub struct YearStats {
    pub year: i32,
    pub entries: i64,
    pub measured: i64,
    pub total_words: i64,
    pub distinct_words: i64,
    pub new_words: i64,
    pub avg_words: f64,
    pub min_words: i64,
    pub max_words: i64,
    pub avg_sentences: f64,
    pub avg_words_per_sentence: f64,
    pub avg_links: f64,
    pub copyright: i64,
    pub images: i64,
    pub videos: i64,
}

/// One catalogued link target.
#[derive(Debug, Serialize)]
pub struct Resource {
    pub id: i64,
    pub url: String,
    pub key: String,
    pub host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub refs: i64,
    pub entries: i64,
    pub credited: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<ApodDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<ApodDate>,
}

#[derive(Debug, Clone, Default)]
pub struct ResourceFilters {
    pub query: Option<String>,
    pub host: Option<String>,
    pub min_refs: Option<i64>,
    pub credited: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResourceOrder {
    #[default]
    Refs,
    Entries,
    First,
    Last,
    Label,
    Address,
}

#[derive(Debug, Serialize)]
pub struct ResourceRefs {
    pub resource: Resource,
    pub items: Vec<ResourceRef>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct ResourceRef {
    #[serde(flatten)]
    pub entry: crate::entry::ApodSummary,
    pub anchor: String,
    pub in_credit: bool,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct HostCount {
    pub host: String,
    pub resources: i64,
    pub refs: i64,
}

#[derive(Debug, Serialize)]
pub struct Word {
    pub word: String,
    pub total: i64,
    pub entries: i64,
}

#[derive(Debug, Clone, Default)]
pub struct WordFilters {
    pub query: Option<String>,
    pub min_total: Option<i64>,
    pub max_total: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WordOrder {
    #[default]
    Total,
    Entries,
    Alphabetical,
}

#[derive(Debug, Serialize)]
pub struct WordUse {
    #[serde(flatten)]
    pub word: Word,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<ApodDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<ApodDate>,
    pub by_year: Vec<YearCount>,
    pub top_entries: Vec<WordEntry>,
}

#[derive(Debug, Serialize)]
pub struct YearCount {
    pub year: i32,
    pub total: i64,
    pub entries: i64,
}

#[derive(Debug, Serialize)]
pub struct WordEntry {
    pub date: ApodDate,
    pub title: String,
    pub count: i64,
}
