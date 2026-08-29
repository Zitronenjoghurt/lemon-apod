use chrono::{Datelike, NaiveDate, TimeDelta, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

const fn days_from_civil(y: i32, m: u32, d: u32) -> i32 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i32 - 719468
}

const START_EPOCH_DAY: i32 = days_from_civil(1995, 6, 16);

const fn from_ymd_const(y: i32, m: u32, d: u32) -> ApodDate {
    ApodDate(days_from_civil(y, m, d) - START_EPOCH_DAY)
}

#[derive(Debug, thiserror::Error)]
#[error("'{0}' is not a valid YYYY-MM-DD date")]
pub struct DateParseError(String);

/// Days since 1995-06-16, APOD's first entry.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApodDate(i32);

impl ApodDate {
    pub const START: Self = Self(0);

    pub const KNOWN_MISSING: [Self; 4] = [
        from_ymd_const(1995, 6, 17),
        from_ymd_const(1995, 6, 18),
        from_ymd_const(1995, 6, 19),
        from_ymd_const(2020, 6, 10),
    ];

    pub const fn from_days(days: i32) -> Self {
        Self(days)
    }

    pub const fn days(self) -> i32 {
        self.0
    }

    pub fn from_ymd(year: i32, month: u32, day: u32) -> Option<Self> {
        NaiveDate::from_ymd_opt(year, month, day).map(Self::from)
    }

    pub fn today_utc() -> Self {
        Self::from(Utc::now().date_naive())
    }

    pub fn naive(self) -> NaiveDate {
        NaiveDate::from_ymd_opt(1995, 6, 16)
            .and_then(|start| start.checked_add_signed(TimeDelta::days(self.0 as i64)))
            .expect("ApodDate is always within NaiveDate's range")
    }

    pub fn format(self, fmt: &str) -> String {
        self.naive().format(fmt).to_string()
    }

    pub fn is_known_missing(self) -> bool {
        Self::KNOWN_MISSING.contains(&self)
    }

    pub fn is_in_range(self, today: Self) -> bool {
        self.0 >= 0 && self <= today && !self.is_known_missing()
    }

    pub fn from_legacy_filename(name: &str) -> Option<Self> {
        let name = name.rsplit('/').next()?;
        let digits = name.strip_prefix("ap")?.strip_suffix(".html")?;
        if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        NaiveDate::parse_from_str(digits, "%y%m%d")
            .ok()
            .map(Self::from)
    }

    pub fn source_url(self) -> String {
        format!(
            "https://apod.nasa.gov/apod/ap{}.html",
            self.format("%y%m%d")
        )
    }

    pub fn html_path(self) -> String {
        format!("{}/{}.html", self.format("%Y/%m"), self)
    }

    pub fn thumb_path(self) -> String {
        format!("{}/{}.webp", self.format("%Y/%m"), self)
    }

    pub fn json_path(self) -> String {
        format!("{}/{}.json", self.format("%Y/%m"), self)
    }

    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    pub const fn prev(self) -> Self {
        Self(self.0 - 1)
    }

    pub fn iter_desc(self) -> impl Iterator<Item = Self> {
        (0..=self.0.max(0))
            .rev()
            .map(Self)
            .filter(|d| !d.is_known_missing())
    }

    pub fn iter_asc(self) -> impl Iterator<Item = Self> {
        (0..=self.0.max(0))
            .map(Self)
            .filter(|d| !d.is_known_missing())
    }
}

impl From<NaiveDate> for ApodDate {
    fn from(date: NaiveDate) -> Self {
        Self(days_from_civil(date.year(), date.month(), date.day()) - START_EPOCH_DAY)
    }
}

impl From<ApodDate> for NaiveDate {
    fn from(date: ApodDate) -> Self {
        date.naive()
    }
}

impl fmt::Display for ApodDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format("%Y-%m-%d"))
    }
}

impl FromStr for ApodDate {
    type Err = DateParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
            .map(Self::from)
            .map_err(|_| DateParseError(s.to_owned()))
    }
}

impl Serialize for ApodDate {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ApodDate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_is_zero() {
        assert_eq!(ApodDate::from_ymd(1995, 6, 16).unwrap(), ApodDate::START);
        assert_eq!(ApodDate::START.to_string(), "1995-06-16");
    }

    #[test]
    fn roundtrips_through_string() {
        let date: ApodDate = "2024-03-05".parse().unwrap();
        assert_eq!(date.to_string(), "2024-03-05");
        assert_eq!(date.naive(), NaiveDate::from_ymd_opt(2024, 3, 5).unwrap());
    }

    #[test]
    fn builds_source_and_storage_paths() {
        let date: ApodDate = "2024-03-05".parse().unwrap();
        assert_eq!(
            date.source_url(),
            "https://apod.nasa.gov/apod/ap240305.html"
        );
        assert_eq!(date.html_path(), "2024/03/2024-03-05.html");
        assert_eq!(date.thumb_path(), "2024/03/2024-03-05.webp");
        assert_eq!(date.json_path(), "2024/03/2024-03-05.json");
    }

    #[test]
    fn skips_known_gaps_when_iterating() {
        let dates: Vec<_> = ApodDate::from_ymd(1995, 6, 20)
            .unwrap()
            .iter_asc()
            .map(|d| d.to_string())
            .collect();
        assert_eq!(dates, vec!["1995-06-16", "1995-06-20"]);
    }

    #[test]
    fn descending_iteration_starts_at_the_newest() {
        let today = ApodDate::from_ymd(1995, 6, 21).unwrap();
        let first = today.iter_desc().next().unwrap();
        assert_eq!(first, today);
    }
}
