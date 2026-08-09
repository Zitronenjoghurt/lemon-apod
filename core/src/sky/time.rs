use chrono::{DateTime, Utc};

pub const J2000: f64 = 2_451_545.0;
pub const DAYS_PER_CENTURY: f64 = 36_525.0;
pub const SYNODIC_MONTH: f64 = 29.530_588_853;

const UNIX_EPOCH_JD: f64 = 2_440_587.5;
const MILLIS_PER_DAY: f64 = 86_400_000.0;
const SECONDS_PER_DAY: f64 = 86_400.0;

pub fn to_julian(at: DateTime<Utc>) -> f64 {
    UNIX_EPOCH_JD + at.timestamp_millis() as f64 / MILLIS_PER_DAY
}

pub fn from_julian(jd: f64) -> DateTime<Utc> {
    let millis = ((jd - UNIX_EPOCH_JD) * MILLIS_PER_DAY).round();
    if !millis.is_finite() {
        return DateTime::UNIX_EPOCH;
    }

    let clamped = millis.clamp(i64::MIN as f64, i64::MAX as f64) as i64;
    DateTime::from_timestamp_millis(clamped).unwrap_or(DateTime::UNIX_EPOCH)
}

pub fn centuries(jd: f64) -> f64 {
    (jd - J2000) / DAYS_PER_CENTURY
}

pub fn approximate_year(jd: f64) -> f64 {
    2000.0 + (jd - J2000) / 365.25
}

/// TT minus UT in seconds, from the Espenak and Meeus polynomials.
pub fn delta_t_seconds(year: f64) -> f64 {
    if year < 1920.0 {
        let t = year - 1900.0;
        -2.79 + 1.494_119 * t - 0.059_893_9 * t.powi(2) + 0.006_196_6 * t.powi(3)
            - 0.000_197 * t.powi(4)
    } else if year < 1941.0 {
        let t = year - 1920.0;
        21.20 + 0.844_93 * t - 0.076_100 * t.powi(2) + 0.002_093_6 * t.powi(3)
    } else if year < 1961.0 {
        let t = year - 1950.0;
        29.07 + 0.407 * t - t.powi(2) / 233.0 + t.powi(3) / 2547.0
    } else if year < 1986.0 {
        let t = year - 1975.0;
        45.45 + 1.067 * t - t.powi(2) / 260.0 - t.powi(3) / 718.0
    } else if year < 2005.0 {
        let t = year - 2000.0;
        63.86 + 0.334_5 * t - 0.060_374 * t.powi(2)
            + 0.001_727_5 * t.powi(3)
            + 0.000_651_814 * t.powi(4)
            + 0.000_023_735_99 * t.powi(5)
    } else if year < 2050.0 {
        let t = year - 2000.0;
        62.92 + 0.322_17 * t + 0.005_589 * t.powi(2)
    } else {
        let u = (year - 1820.0) / 100.0;
        -20.0 + 32.0 * u * u - 0.562_8 * (2150.0 - year)
    }
}

pub fn dynamical_to_utc(jde: f64) -> DateTime<Utc> {
    let correction = delta_t_seconds(approximate_year(jde)) / SECONDS_PER_DAY;
    from_julian(jde - correction)
}

pub fn dynamical_julian(at: DateTime<Utc>) -> f64 {
    let jd = to_julian(at);
    jd + delta_t_seconds(approximate_year(jd)) / SECONDS_PER_DAY
}

pub fn normalize_degrees(degrees: f64) -> f64 {
    let wrapped = degrees % 360.0;
    if wrapped < 0.0 {
        wrapped + 360.0
    } else {
        wrapped
    }
}

pub fn angle_difference(from: f64, to: f64) -> f64 {
    let difference = normalize_degrees(to - from);
    if difference > 180.0 {
        difference - 360.0
    } else {
        difference
    }
}

pub fn sin_deg(degrees: f64) -> f64 {
    degrees.to_radians().sin()
}

pub fn cos_deg(degrees: f64) -> f64 {
    degrees.to_radians().cos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn the_j2000_epoch_round_trips() {
        let epoch = Utc.with_ymd_and_hms(2000, 1, 1, 12, 0, 0).unwrap();
        assert!((to_julian(epoch) - J2000).abs() < 1e-9);
        assert_eq!(from_julian(J2000), epoch);
    }

    #[test]
    fn a_known_julian_day_matches_meeus() {
        let launch = Utc.with_ymd_and_hms(1957, 10, 4, 19, 26, 24).unwrap();
        assert!((to_julian(launch) - 2_436_116.31).abs() < 1e-4);
    }

    #[test]
    fn julian_days_round_trip_to_the_millisecond() {
        let at = Utc.with_ymd_and_hms(2026, 8, 8, 13, 47, 3).unwrap();
        assert_eq!(from_julian(to_julian(at)), at);
    }

    #[test]
    fn delta_t_is_about_seventy_seconds_in_the_twenties() {
        let now = delta_t_seconds(2026.0);
        assert!(
            (69.0..80.0).contains(&now),
            "delta T for 2026 came out as {now}"
        );
    }

    #[test]
    fn delta_t_tracks_the_measured_record() {
        for (year, expected) in [
            (1910.0, 10.0),
            (1930.0, 24.0),
            (1950.0, 29.0),
            (1970.0, 40.0),
            (1995.0, 61.0),
            (2020.0, 70.0),
        ] {
            let found = delta_t_seconds(year);
            assert!(
                (found - expected).abs() < 3.0,
                "delta T for {year} came out as {found}, expected about {expected}"
            );
        }
    }

    #[test]
    fn the_branches_meet_without_a_step() {
        for boundary in [1920.0, 1941.0, 1961.0, 1986.0, 2005.0, 2050.0] {
            let below = delta_t_seconds(boundary - 0.001);
            let above = delta_t_seconds(boundary + 0.001);
            assert!(
                (below - above).abs() < 2.0,
                "delta T jumps from {below} to {above} across {boundary}"
            );
        }
    }

    #[test]
    fn normalizing_lands_every_angle_in_one_turn() {
        for degrees in [-720.5, -0.5, 0.0, 359.5, 361.0, 1080.0] {
            let normalized = normalize_degrees(degrees);
            assert!(
                (0.0..360.0).contains(&normalized),
                "{degrees} normalized to {normalized}"
            );
        }
    }

    #[test]
    fn angle_differences_take_the_short_way_round() {
        assert!((angle_difference(350.0, 10.0) - 20.0).abs() < 1e-9);
        assert!((angle_difference(10.0, 350.0) + 20.0).abs() < 1e-9);
        assert!((angle_difference(0.0, 180.0) - 180.0).abs() < 1e-9);
    }
}
