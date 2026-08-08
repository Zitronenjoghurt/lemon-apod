use super::time::{DAYS_PER_CENTURY, J2000, cos_deg, dynamical_to_utc, to_julian};
use chrono::{DateTime, Datelike, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Turning {
    MarchEquinox,
    JuneSolstice,
    SeptemberEquinox,
    DecemberSolstice,
}

impl Turning {
    pub const ALL: [Self; 4] = [
        Self::MarchEquinox,
        Self::JuneSolstice,
        Self::SeptemberEquinox,
        Self::DecemberSolstice,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::MarchEquinox => "March equinox",
            Self::JuneSolstice => "June solstice",
            Self::SeptemberEquinox => "September equinox",
            Self::DecemberSolstice => "December solstice",
        }
    }

    pub const fn opens_northern(self) -> &'static str {
        match self {
            Self::MarchEquinox => "spring",
            Self::JuneSolstice => "summer",
            Self::SeptemberEquinox => "autumn",
            Self::DecemberSolstice => "winter",
        }
    }

    pub const fn opens_southern(self) -> &'static str {
        match self {
            Self::MarchEquinox => "autumn",
            Self::JuneSolstice => "winter",
            Self::SeptemberEquinox => "spring",
            Self::DecemberSolstice => "summer",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TurningEvent {
    pub turning: Turning,
    pub label: &'static str,
    pub at: DateTime<Utc>,
    pub opens_northern: &'static str,
    pub opens_southern: &'static str,
}

fn event(turning: Turning, at: DateTime<Utc>) -> TurningEvent {
    TurningEvent {
        turning,
        label: turning.label(),
        at,
        opens_northern: turning.opens_northern(),
        opens_southern: turning.opens_southern(),
    }
}

pub fn turning_point(year: i32, turning: Turning) -> DateTime<Utc> {
    dynamical_to_utc(correct(mean_turning(year, turning)))
}

pub fn turning_points(year: i32) -> Vec<TurningEvent> {
    Turning::ALL
        .iter()
        .map(|&turning| event(turning, turning_point(year, turning)))
        .collect()
}

pub fn next_turning(at: DateTime<Utc>) -> TurningEvent {
    let year = at.year();
    [year, year + 1]
        .into_iter()
        .flat_map(turning_points)
        .filter(|found| found.at > at)
        .min_by_key(|found| found.at)
        .unwrap_or_else(|| {
            event(
                Turning::MarchEquinox,
                turning_point(year + 2, Turning::MarchEquinox),
            )
        })
}

pub fn current_season(at: DateTime<Utc>) -> TurningEvent {
    let year = at.year();

    [year - 1, year]
        .into_iter()
        .flat_map(turning_points)
        .filter(|found| found.at <= at)
        .max_by_key(|found| found.at)
        .unwrap_or_else(|| {
            event(
                Turning::DecemberSolstice,
                turning_point(year - 2, Turning::DecemberSolstice),
            )
        })
}

fn mean_turning(year: i32, turning: Turning) -> f64 {
    let y = (f64::from(year) - 2000.0) / 1000.0;

    let (base, per_y, y2, y3, y4) = match turning {
        Turning::MarchEquinox => (
            2_451_623.809_84,
            365_242.374_04,
            0.051_69,
            -0.004_11,
            -0.000_57,
        ),
        Turning::JuneSolstice => (
            2_451_716.567_67,
            365_241.626_03,
            0.003_25,
            0.008_88,
            -0.000_30,
        ),
        Turning::SeptemberEquinox => (
            2_451_810.217_15,
            365_242.017_67,
            -0.115_75,
            0.003_37,
            0.000_78,
        ),
        Turning::DecemberSolstice => (
            2_451_900.059_52,
            365_242.740_49,
            -0.062_23,
            -0.008_23,
            0.000_32,
        ),
    };

    base + per_y * y + y2 * y.powi(2) + y3 * y.powi(3) + y4 * y.powi(4)
}

fn correct(mean_jde: f64) -> f64 {
    let t = (mean_jde - J2000) / DAYS_PER_CENTURY;
    let w = 35_999.373 * t - 2.47;
    let lambda = 1.0 + 0.033_4 * cos_deg(w) + 0.000_7 * cos_deg(2.0 * w);

    const TERMS: [(f64, f64, f64); 24] = [
        (485.0, 324.96, 1_934.136),
        (203.0, 337.23, 32_964.467),
        (199.0, 342.08, 20.186),
        (182.0, 27.85, 445_267.112),
        (156.0, 73.14, 45_036.886),
        (136.0, 171.52, 22_518.443),
        (77.0, 222.54, 65_928.934),
        (74.0, 296.72, 3_034.906),
        (70.0, 243.58, 9_037.513),
        (58.0, 119.81, 33_718.147),
        (52.0, 297.17, 150.678),
        (50.0, 21.02, 2_281.226),
        (45.0, 247.54, 29_929.562),
        (44.0, 325.15, 31_555.956),
        (29.0, 60.93, 4_443.417),
        (18.0, 155.12, 67_555.328),
        (17.0, 288.79, 4_562.452),
        (16.0, 198.04, 62_894.029),
        (14.0, 199.76, 31_436.921),
        (12.0, 95.39, 14_577.848),
        (12.0, 287.11, 31_931.756),
        (12.0, 320.81, 34_777.259),
        (9.0, 227.73, 1_222.114),
        (8.0, 15.45, 16_859.074),
    ];

    let sum: f64 = TERMS
        .iter()
        .map(|&(amplitude, phase, rate)| amplitude * cos_deg(phase + rate * t))
        .sum();

    mean_jde + (0.000_01 * sum) / lambda
}

pub fn apparent_longitude(at: DateTime<Utc>) -> f64 {
    let t = (to_julian(at) - J2000) / DAYS_PER_CENTURY;

    let mean_longitude = 280.466_46 + 36_000.769_83 * t + 0.000_303_2 * t.powi(2);
    let anomaly = 357.529_11 + 35_999.050_29 * t - 0.000_153_7 * t.powi(2);

    let centre = (1.914_602 - 0.004_817 * t - 0.000_014 * t.powi(2))
        * super::time::sin_deg(anomaly)
        + (0.019_993 - 0.000_101 * t) * super::time::sin_deg(2.0 * anomaly)
        + 0.000_289 * super::time::sin_deg(3.0 * anomaly);

    super::time::normalize_degrees(mean_longitude + centre)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Timelike};

    fn utc(y: i32, m: u32, d: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, hour, minute, 0).unwrap()
    }

    #[test]
    fn the_june_solstice_of_meeus_example_27a() {
        let found = turning_point(1962, Turning::JuneSolstice);
        let expected = utc(1962, 6, 21, 21, 23);

        let error = (found - expected).as_seconds_f64().abs();
        assert!(error < 120.0, "got {found}, expected about {expected}");
    }

    #[test]
    fn the_turnings_of_2026_land_where_the_almanacs_put_them() {
        for (turning, expected) in [
            (Turning::MarchEquinox, utc(2026, 3, 20, 14, 46)),
            (Turning::JuneSolstice, utc(2026, 6, 21, 8, 24)),
            (Turning::SeptemberEquinox, utc(2026, 9, 23, 0, 5)),
            (Turning::DecemberSolstice, utc(2026, 12, 21, 20, 50)),
        ] {
            let found = turning_point(2026, turning);
            let error = (found - expected).as_seconds_f64().abs();
            assert!(
                error < 300.0,
                "{turning:?}: got {found}, expected {expected}"
            );
        }
    }

    #[test]
    fn a_years_turnings_come_back_in_calendar_order() {
        let points = turning_points(2026);
        assert_eq!(points.len(), 4);

        for pair in points.windows(2) {
            assert!(pair[0].at < pair[1].at, "{points:?}");
        }
        assert_eq!(points[0].turning, Turning::MarchEquinox);
        assert_eq!(points[3].turning, Turning::DecemberSolstice);
    }

    #[test]
    fn the_next_turning_is_always_ahead_and_within_a_season() {
        for month in 1..=12 {
            let at = utc(2026, month, 15, 12, 0);
            let next = next_turning(at);

            assert!(next.at > at, "{month}: {next:?}");
            let days = (next.at - at).as_seconds_f64() / 86_400.0;
            assert!(days <= 95.0, "{month}: {days} days to {next:?}");
        }
    }

    #[test]
    fn december_rolls_into_the_next_years_march() {
        let next = next_turning(utc(2026, 12, 28, 0, 0));
        assert_eq!(next.turning, Turning::MarchEquinox);
        assert_eq!(next.at.year(), 2027);
    }

    #[test]
    fn the_current_season_is_the_turning_just_passed() {
        assert_eq!(
            current_season(utc(2026, 7, 15, 0, 0)).turning,
            Turning::JuneSolstice
        );
        assert_eq!(
            current_season(utc(2026, 1, 15, 0, 0)).turning,
            Turning::DecemberSolstice
        );
        assert_eq!(current_season(utc(2026, 1, 15, 0, 0)).at.year(), 2025);
    }

    #[test]
    fn the_two_hemispheres_are_never_given_the_same_season() {
        for turning in Turning::ALL {
            assert_ne!(turning.opens_northern(), turning.opens_southern());
        }
    }

    #[test]
    fn the_sun_walks_the_whole_ecliptic_over_a_year() {
        let equinox = turning_point(2026, Turning::MarchEquinox);
        let at_equinox = apparent_longitude(equinox);
        assert!(
            !(0.02..=359.98).contains(&at_equinox),
            "the sun was at {at_equinox} degrees at the March equinox"
        );

        let solstice = apparent_longitude(turning_point(2026, Turning::JuneSolstice));
        assert!((solstice - 90.0).abs() < 0.02, "{solstice}");
    }

    #[test]
    fn an_equinox_is_near_enough_to_midnight_utc_to_be_worth_a_time() {
        let found = turning_point(2026, Turning::MarchEquinox);
        assert_eq!(found.day(), 20);
        assert_eq!(found.hour(), 14);
    }
}
