use crate::date::ApodDate;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pause {
    pub start: ApodDate,
    pub end: Option<ApodDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl Pause {
    pub fn new(start: ApodDate, end: Option<ApodDate>, reason: Option<String>) -> Self {
        Self { start, end, reason }
    }

    pub fn covers(&self, date: ApodDate) -> bool {
        date >= self.start && self.end.is_none_or(|end| date <= end)
    }

    pub fn running_on(&self, today: ApodDate) -> bool {
        self.covers(today)
    }

    pub fn well_formed(&self) -> bool {
        self.end.is_none_or(|end| end >= self.start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> ApodDate {
        text.parse().unwrap()
    }

    fn window(start: &str, end: Option<&str>) -> Pause {
        Pause::new(date(start), end.map(date), None)
    }

    #[test]
    fn a_closed_window_covers_both_ends_and_nothing_outside() {
        let pause = window("2018-12-22", Some("2019-01-25"));

        assert!(pause.covers(date("2018-12-22")));
        assert!(pause.covers(date("2019-01-01")));
        assert!(pause.covers(date("2019-01-25")));
        assert!(!pause.covers(date("2018-12-21")));
        assert!(!pause.covers(date("2019-01-26")));
    }

    #[test]
    fn an_open_window_covers_everything_from_its_start() {
        let pause = window("2026-10-01", None);

        assert!(!pause.covers(date("2026-09-30")));
        assert!(pause.covers(date("2026-10-01")));
        assert!(pause.covers(date("2030-01-01")));
    }

    #[test]
    fn a_window_starting_later_is_not_running_yet() {
        let pause = window("2026-12-01", None);

        assert!(!pause.running_on(date("2026-11-30")));
        assert!(pause.running_on(date("2026-12-01")));
    }

    #[test]
    fn an_end_before_the_start_is_rejected() {
        assert!(!window("2026-10-01", Some("2026-09-01")).well_formed());
        assert!(window("2026-10-01", Some("2026-10-01")).well_formed());
        assert!(window("2026-10-01", None).well_formed());
    }
}
