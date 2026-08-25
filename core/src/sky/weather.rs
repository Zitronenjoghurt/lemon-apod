use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

const ALERT_LIFE_HOURS: i64 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Band {
    G,
    S,
    R,
}

impl Band {
    pub const ALL: [Self; 3] = [Self::G, Self::S, Self::R];

    pub const fn letter(self) -> &'static str {
        match self {
            Self::G => "G",
            Self::S => "S",
            Self::R => "R",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::G => "Geomagnetic storms",
            Self::S => "Solar radiation",
            Self::R => "Radio blackouts",
        }
    }

    pub const fn about(self) -> &'static str {
        match self {
            Self::G => {
                "The magnetic field being shaken by the solar wind, which influences power grids and brings auroras closer to the equator."
            }
            Self::S => {
                "Energetic protons streaming past Earth, which reaches satellites and polar flights."
            }
            Self::R => {
                "Flares soaking the sunlit side of Earth in x-rays, which drowns out high frequency radio."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chance {
    pub minor: Option<u8>,
    pub major: Option<u8>,
}

impl Chance {
    pub fn is_empty(&self) -> bool {
        self.minor.is_none() && self.major.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level {
    pub band: Band,
    pub scale: Option<u8>,
    pub text: Option<String>,
    #[serde(default)]
    pub chance: Chance,
}

impl Level {
    pub fn quiet(&self) -> bool {
        self.scale.unwrap_or(0) == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleDay {
    pub date: String,
    #[serde(default)]
    pub observed_at: Option<DateTime<Utc>>,
    pub levels: Vec<Level>,
}

impl ScaleDay {
    pub fn quiet(&self) -> bool {
        self.levels.iter().all(Level::quiet)
    }

    pub fn worst(&self) -> Option<&Level> {
        self.levels
            .iter()
            .max_by_key(|level| level.scale.unwrap_or(0))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct KpPoint {
    pub at: DateTime<Utc>,
    pub kp: f64,
    pub ahead: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FluxPoint {
    pub at: DateTime<Utc>,
    pub flux: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DstPoint {
    pub at: DateTime<Utc>,
    pub dst: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Notice {
    /// Something is happening now.
    Alert,
    /// Something is expected within hours.
    Warning,
    /// Something is expected within days.
    Watch,
    /// Something has finished, written up after the fact.
    Summary,
}

impl Notice {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Alert => "Alert",
            Self::Warning => "Warning",
            Self::Watch => "Watch",
            Self::Summary => "Summary",
        }
    }

    pub const fn pressing(self) -> bool {
        matches!(self, Self::Alert | Self::Warning)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub notice: Notice,
    pub headline: String,
    pub scale: Option<String>,
    pub issued_at: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
    pub message: String,
}

impl Alert {
    pub fn current(&self, now: DateTime<Utc>) -> bool {
        self.notice.pressing() && self.in_force(now)
    }

    pub fn in_force(&self, now: DateTime<Utc>) -> bool {
        match self.valid_until {
            Some(until) => until >= now,
            None => now - self.issued_at <= TimeDelta::hours(ALERT_LIFE_HOURS),
        }
    }

    pub fn band(&self) -> Option<Band> {
        let lettered = self
            .scale
            .as_deref()
            .and_then(|scale| scale.trim().chars().next())
            .and_then(|letter| match letter {
                'G' => Some(Band::G),
                'S' => Some(Band::S),
                'R' => Some(Band::R),
                _ => None,
            });

        lettered.or_else(|| self.headline.contains("Geomagnetic").then_some(Band::G))
    }

    pub fn is_geomagnetic(&self) -> bool {
        self.band() == Some(Band::G)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherReport {
    pub kp: f64,
    pub observed_at: DateTime<Utc>,
    pub scales: Option<ScaleDay>,
    pub outlook: Vec<ScaleDay>,
    pub kp_series: Vec<KpPoint>,
    pub flux: Vec<FluxPoint>,
    pub dst: Vec<DstPoint>,
    pub alerts: Vec<Alert>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeatherSummary {
    pub kp: f64,
    pub observed_at: DateTime<Utc>,
    pub scales: Option<ScaleDay>,
    pub alert: Option<Alert>,
    pub active: usize,
}

impl WeatherReport {
    pub fn summary(&self, now: DateTime<Utc>) -> WeatherSummary {
        let current: Vec<&Alert> = self
            .alerts
            .iter()
            .filter(|alert| alert.current(now))
            .collect();

        WeatherSummary {
            kp: self.kp,
            observed_at: self.observed_at,
            scales: self.scales.clone(),
            alert: current.first().map(|alert| (*alert).clone()),
            active: current.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hours: i64) -> DateTime<Utc> {
        DateTime::UNIX_EPOCH + TimeDelta::hours(hours)
    }

    fn alert(notice: Notice, issued: i64, until: Option<i64>) -> Alert {
        Alert {
            id: format!("{issued}"),
            notice,
            headline: "Geomagnetic K-index of 6".to_owned(),
            scale: Some("G2 - Moderate".to_owned()),
            issued_at: at(issued),
            valid_until: until.map(at),
            message: String::new(),
        }
    }

    fn banded(headline: &str, scale: Option<&str>) -> Alert {
        Alert {
            headline: headline.to_owned(),
            scale: scale.map(str::to_owned),
            ..alert(Notice::Alert, 100, None)
        }
    }

    #[test]
    fn the_noaa_scale_field_names_the_band() {
        assert_eq!(banded("x", Some("G2 - Moderate")).band(), Some(Band::G));
        assert_eq!(banded("x", Some("S1 - Minor")).band(), Some(Band::S));
        assert_eq!(banded("x", Some("R3 - Strong")).band(), Some(Band::R));
    }

    #[test]
    fn a_geomagnetic_alert_below_the_scale_is_still_geomagnetic() {
        let quiet = banded("Geomagnetic K-index of 4", None);

        assert_eq!(quiet.band(), Some(Band::G));
        assert!(quiet.is_geomagnetic());
    }

    #[test]
    fn an_alert_on_no_scale_at_all_is_not_claimed_for_a_band() {
        let electron = banded("Electron 2MeV Integral Flux exceeded 1,000pfu", None);

        assert_eq!(electron.band(), None);
        assert!(!electron.is_geomagnetic());
    }

    #[test]
    fn a_proton_event_is_not_mistaken_for_an_aurora() {
        let proton = banded(
            "Proton Event 10MeV Integral Flux exceeded 10pfu",
            Some("S1 - Minor"),
        );

        assert!(!proton.is_geomagnetic());
    }

    #[test]
    fn every_alert_shape_noaa_actually_issues_lands_on_the_right_side() {
        let cases: [(&str, Option<&str>, bool); 15] = [
            ("Geomagnetic K-index of 4", None, true),
            ("Geomagnetic K-index of 4 expected", None, true),
            ("Geomagnetic K-index of 5", Some("G1 - Minor"), true),
            (
                "Geomagnetic K-index of 5 expected",
                Some("G1 - Minor"),
                true,
            ),
            ("Geomagnetic K-index of 6", Some("G2 - Moderate"), true),
            (
                "Geomagnetic K-index of 6 expected",
                Some("G2 - Moderate"),
                true,
            ),
            ("Geomagnetic Storm Category G1 Predicted", None, true),
            ("Geomagnetic Storm Category G2 Predicted", None, true),
            ("Geomagnetic Sudden Impulse", None, true),
            ("Geomagnetic Sudden Impulse expected", None, true),
            ("Electron 2MeV Integral Flux exceeded 1,000pfu", None, false),
            (
                "Proton Event 100MeV Integral Flux exceeded 1pfu",
                None,
                false,
            ),
            (
                "Proton 10MeV Integral Flux above 10pfu expected",
                Some("S1 - Minor"),
                false,
            ),
            (
                "Proton Event 10MeV Integral Flux exceeded 10pfu",
                Some("S1 - Minor"),
                false,
            ),
            (
                "Proton Event 10meV Integral Flux exceeded 10pfu",
                Some("S1 - Minor"),
                false,
            ),
        ];

        for (headline, scale, geomagnetic) in cases {
            assert_eq!(
                banded(headline, scale).is_geomagnetic(),
                geomagnetic,
                "{headline:?} with scale {scale:?}"
            );
        }
    }

    #[test]
    fn a_watch_is_in_force_even_though_it_is_not_pressing() {
        let watch = alert(Notice::Watch, 100, Some(150));

        assert!(watch.in_force(at(120)), "still inside its validity");
        assert!(
            !watch.current(at(120)),
            "but not urgent enough for `current`"
        );
        assert!(!watch.in_force(at(151)), "and it does expire");
    }

    #[test]
    fn a_notice_with_an_expiry_is_current_right_up_to_it() {
        let warning = alert(Notice::Warning, 100, Some(110));

        assert!(warning.current(at(105)));
        assert!(warning.current(at(110)));
        assert!(!warning.current(at(111)));
    }

    #[test]
    fn a_notice_with_no_expiry_gets_a_few_hours_and_no_more() {
        let raised = alert(Notice::Alert, 100, None);

        assert!(raised.current(at(102)));
        assert!(!raised.current(at(104)));
    }

    #[test]
    fn a_watch_or_a_write_up_never_counts_as_something_going_on() {
        assert!(!alert(Notice::Watch, 100, Some(200)).current(at(150)));
        assert!(!alert(Notice::Summary, 100, None).current(at(100)));
    }

    #[test]
    fn the_summary_carries_the_first_notice_still_in_force() {
        let report = WeatherReport {
            kp: 6.0,
            observed_at: at(100),
            scales: None,
            outlook: Vec::new(),
            kp_series: Vec::new(),
            flux: Vec::new(),
            dst: Vec::new(),
            alerts: vec![
                alert(Notice::Summary, 99, None),
                alert(Notice::Alert, 100, Some(120)),
                alert(Notice::Warning, 98, Some(120)),
            ],
        };

        let summary = report.summary(at(101));
        assert_eq!(summary.active, 2, "the write-up is not something going on");
        assert_eq!(summary.alert.map(|alert| alert.id), Some("100".to_owned()));
    }

    #[test]
    fn a_day_reports_the_worst_of_its_three_scales() {
        let day = ScaleDay {
            date: "2026-08-09".to_owned(),
            observed_at: None,
            levels: vec![
                Level {
                    band: Band::R,
                    scale: Some(1),
                    text: Some("minor".to_owned()),
                    chance: Chance::default(),
                },
                Level {
                    band: Band::S,
                    scale: Some(0),
                    text: Some("none".to_owned()),
                    chance: Chance::default(),
                },
                Level {
                    band: Band::G,
                    scale: Some(3),
                    text: Some("strong".to_owned()),
                    chance: Chance::default(),
                },
            ],
        };

        assert!(!day.quiet());
        assert_eq!(day.worst().map(|level| level.band), Some(Band::G));
    }

    #[test]
    fn a_day_with_nothing_on_it_is_quiet() {
        let day = ScaleDay {
            date: "2026-08-09".to_owned(),
            observed_at: None,
            levels: Band::ALL
                .into_iter()
                .map(|band| Level {
                    band,
                    scale: Some(0),
                    text: Some("none".to_owned()),
                    chance: Chance::default(),
                })
                .collect(),
        };

        assert!(day.quiet());
    }
}
