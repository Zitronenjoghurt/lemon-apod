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
    pub licensed: i64,
    pub gaps: i64,
    pub gap_dates: Vec<ApodDate>,
    pub text: TextSummary,
    pub resources: ResourceSummary,
    pub pictures: PictureSummary,
}

#[derive(Debug, Default, Serialize)]
pub struct PictureSummary {
    pub hashed: i64,
    pub pictures: i64,
    pub entries: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub most_shown: Option<ApodDate>,
    pub most_shown_times: i64,
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
    pub p25_words: i64,
    pub p75_words: i64,
    pub min_words: i64,
    pub max_words: i64,
    pub avg_unique_words: f64,
    pub avg_chars: f64,
    pub avg_sentences: f64,
    pub avg_words_per_sentence: f64,
    pub avg_links: f64,
    pub used_once: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lengths: Vec<LengthBucket>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortest: Option<EntryLength>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longest: Option<EntryLength>,
}

#[derive(Debug, Serialize)]
pub struct LengthBucket {
    pub from: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<i64>,
    pub entries: i64,
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
pub struct Coverage {
    pub months: Vec<MonthCount>,
}

#[derive(Debug, Serialize)]
pub struct MonthCount {
    pub year: i32,
    pub month: u32,
    pub entries: i64,
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
    pub anchors: Vec<AnchorCount>,
}

#[derive(Debug, Serialize)]
pub struct AnchorCount {
    pub anchor: String,
    pub entries: i64,
}

#[derive(Debug, Serialize)]
pub struct Picture {
    pub id: ApodDate,
    pub title: String,
    pub media: crate::media::Media,
    pub appearances: i64,
    pub first: ApodDate,
    pub last: ApodDate,
    pub titles: i64,
    pub span_days: i64,
}

#[derive(Debug, Clone, Default)]
pub struct PictureFilters {
    pub query: Option<String>,
    pub min_appearances: Option<i64>,
    pub retitled: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PictureOrder {
    #[default]
    Appearances,
    First,
    Last,
    Span,
    Title,
}

#[derive(Debug, Serialize)]
pub struct PictureAppearances {
    pub picture: Picture,
    pub items: Vec<Appearance>,
}

#[derive(Debug, Serialize)]
pub struct Appearance {
    #[serde(flatten)]
    pub entry: crate::entry::ApodSummary,
    pub changed: Changed,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since_previous_days: Option<i64>,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Changed {
    pub title: bool,
    pub explanation: bool,
    pub credit: bool,
    pub file: bool,
}

impl Changed {
    pub fn any(&self) -> bool {
        self.title || self.explanation || self.credit || self.file
    }
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

#[derive(Debug, Serialize)]
pub struct Cloze {
    pub salt: String,
    pub title: Vec<ClozePiece>,
    pub text: Vec<ClozePiece>,
    pub hidden: usize,
    pub distinct: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum ClozePiece {
    Shown { s: String },
    Hidden { h: String, n: usize },
}
