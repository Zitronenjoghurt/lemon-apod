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
}

pub fn g_level(kp: f64) -> Option<u8> {
    let level = kp.floor();
    (level >= 5.0).then(|| level.min(9.0) as u8 - 4)
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

    pub fn level(&self) -> Option<(Band, u8)> {
        let mut scale = self.scale.as_deref()?.trim().chars();

        let band = match scale.next()? {
            'G' => Band::G,
            'S' => Band::S,
            'R' => Band::R,
            _ => return None,
        };

        let level = scale.next()?.to_digit(10)? as u8;
        (1..=5).contains(&level).then_some((band, level))
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
            scales: self.measured_scales(now),
            alert: current.first().map(|alert| (*alert).clone()),
            active: current.len(),
        }
    }

    pub fn measured_scales(&self, now: DateTime<Utc>) -> Option<ScaleDay> {
        let mut day = self.scales.clone()?;
        raise(&mut day, Band::G, g_level(self.kp));

        for alert in &self.alerts {
            if alert.notice == Notice::Alert
                && alert.in_force(now)
                && let Some((band, level)) = alert.level()
            {
                raise(&mut day, band, Some(level));
            }
        }

        Some(day)
    }
}

fn raise(day: &mut ScaleDay, band: Band, level: Option<u8>) {
    let Some(level) = level else { return };
    let Some(found) = day.levels.iter_mut().find(|one| one.band == band) else {
        return;
    };

    if found.scale.unwrap_or(0) < level {
        found.scale = Some(level);
        found.text = None;
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
    fn the_g_scale_runs_from_five_to_nine_and_tops_out_at_g5() {
        assert_eq!(g_level(0.0), None);
        assert_eq!(g_level(4.67), None, "just under a storm is not one");
        assert_eq!(g_level(5.0), Some(1));
        assert_eq!(g_level(5.67), Some(1), "still G1 until Kp reaches 6");
        assert_eq!(g_level(6.0), Some(2));
        assert_eq!(g_level(7.0), Some(3));
        assert_eq!(g_level(8.0), Some(4));
        assert_eq!(g_level(9.0), Some(5));
        assert_eq!(g_level(9.5), Some(5), "the scale stops at G5");
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

    fn quiet_day() -> ScaleDay {
        ScaleDay {
            date: "2026-08-30".to_owned(),
            observed_at: Some(at(100)),
            levels: Band::ALL
                .into_iter()
                .map(|band| Level {
                    band,
                    scale: Some(0),
                    text: Some("none".to_owned()),
                    chance: Chance::default(),
                })
                .collect(),
        }
    }

    fn reported(kp: f64, alerts: Vec<Alert>) -> WeatherReport {
        WeatherReport {
            kp,
            observed_at: at(100),
            scales: Some(quiet_day()),
            outlook: Vec::new(),
            kp_series: Vec::new(),
            flux: Vec::new(),
            dst: Vec::new(),
            alerts,
        }
    }

    fn level_of(day: &ScaleDay, band: Band) -> Option<u8> {
        day.levels
            .iter()
            .find(|level| level.band == band)
            .and_then(|level| level.scale)
    }

    #[test]
    fn a_measured_kp_settles_the_g_band_whatever_the_daily_table_still_says() {
        let report = reported(5.0, Vec::new());
        let day = report.measured_scales(at(101)).unwrap();

        assert_eq!(level_of(&day, Band::G), Some(1));
        assert_eq!(
            day.levels
                .iter()
                .find(|level| level.band == Band::G)
                .and_then(|level| level.text.clone()),
            None,
            "NOAA's \"none\" cannot be left standing next to a G1"
        );
        assert_eq!(level_of(&day, Band::S), Some(0), "and nothing else moves");
        assert!(!day.quiet());
    }

    #[test]
    fn an_alert_in_force_raises_its_own_band_and_a_warning_raises_nothing() {
        let raised = Alert {
            scale: Some("R3 - Strong".to_owned()),
            ..alert(Notice::Alert, 100, Some(120))
        };
        let expected = Alert {
            scale: Some("S2 - Moderate".to_owned()),
            ..alert(Notice::Warning, 100, Some(120))
        };

        let day = reported(3.0, vec![raised.clone(), expected])
            .measured_scales(at(110))
            .unwrap();

        assert_eq!(level_of(&day, Band::R), Some(3));
        assert_eq!(
            level_of(&day, Band::S),
            Some(0),
            "a warning says a level is expected, which is not a level reached"
        );

        let expired = reported(3.0, vec![raised])
            .measured_scales(at(121))
            .unwrap();
        assert_eq!(level_of(&expired, Band::R), Some(0));
    }

    #[test]
    fn a_table_that_has_already_seen_worse_keeps_its_own_reading() {
        let mut day = quiet_day();
        for level in &mut day.levels {
            level.scale = Some(3);
            level.text = Some("strong".to_owned());
        }

        let report = WeatherReport {
            scales: Some(day),
            ..reported(5.0, Vec::new())
        };
        let measured = report.measured_scales(at(101)).unwrap();

        assert_eq!(
            level_of(&measured, Band::G),
            Some(3),
            "the day peaked higher"
        );
    }

    #[test]
    fn only_a_scale_carrying_a_number_names_a_level() {
        assert_eq!(
            banded("x", Some("G2 - Moderate")).level(),
            Some((Band::G, 2))
        );
        assert_eq!(banded("Geomagnetic K-index of 4", None).level(), None);
        assert_eq!(
            banded("Geomagnetic Storm Category G1 Predicted", None).level(),
            None,
            "the band is in the headline, the level is not, and guessing it is not this job"
        );
        assert_eq!(banded("x", Some("G0")).level(), None);
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
