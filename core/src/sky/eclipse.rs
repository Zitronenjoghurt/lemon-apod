use super::moon::{self, Quarter};
use super::time::{cos_deg, dynamical_to_utc, normalize_degrees, sin_deg, to_julian};
use chrono::{DateTime, TimeDelta, Utc};
use serde::Serialize;

const NODE_LIMIT: f64 = 0.36;
const SEARCH_LUNATIONS: i32 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SolarKind {
    Total,
    Annular,
    Hybrid,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LunarKind {
    Total,
    Partial,
    Penumbral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Eclipse {
    Solar(SolarKind),
    Lunar(LunarKind),
}

impl Eclipse {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Solar(SolarKind::Total) => "Total solar eclipse",
            Self::Solar(SolarKind::Annular) => "Annular solar eclipse",
            Self::Solar(SolarKind::Hybrid) => "Hybrid solar eclipse",
            Self::Solar(SolarKind::Partial) => "Partial solar eclipse",
            Self::Lunar(LunarKind::Total) => "Total lunar eclipse",
            Self::Lunar(LunarKind::Partial) => "Partial lunar eclipse",
            Self::Lunar(LunarKind::Penumbral) => "Penumbral lunar eclipse",
        }
    }

    pub const fn is_solar(self) -> bool {
        matches!(self, Self::Solar(_))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EclipseEvent {
    pub eclipse: Eclipse,
    pub label: &'static str,
    pub solar: bool,
    pub at: DateTime<Utc>,
    pub magnitude: f64,
}

pub fn upcoming(at: DateTime<Utc>) -> Vec<EclipseEvent> {
    let mut found: Vec<EclipseEvent> = [next_solar(at), next_lunar(at)]
        .into_iter()
        .flatten()
        .collect();

    found.sort_by_key(|event| event.at);
    found
}

pub fn next_solar(at: DateTime<Utc>) -> Option<EclipseEvent> {
    next_of(at, Quarter::New)
}

pub fn next_lunar(at: DateTime<Utc>) -> Option<EclipseEvent> {
    next_of(at, Quarter::Full)
}

fn next_of(at: DateTime<Utc>, quarter: Quarter) -> Option<EclipseEvent> {
    let base = moon::lunation_index(at).floor();

    (-1..=SEARCH_LUNATIONS)
        .filter_map(|step| examine(base + f64::from(step) + quarter.offset(), quarter))
        .find(|event| event.at > at)
}

fn examine(k: f64, quarter: Quarter) -> Option<EclipseEvent> {
    let t = k / 1236.85;

    let e = 1.0 - 0.002_516 * t - 0.000_007_4 * t.powi(2);

    let m = normalize_degrees(
        2.553_4 + 29.105_356_70 * k - 0.000_001_4 * t.powi(2) - 0.000_000_11 * t.powi(3),
    );
    let mp = normalize_degrees(
        201.564_3 + 385.816_935_28 * k + 0.010_758_2 * t.powi(2) + 0.000_012_38 * t.powi(3)
            - 0.000_000_058 * t.powi(4),
    );
    let f = normalize_degrees(
        160.710_8 + 390.670_502_84 * k - 0.001_611_8 * t.powi(2) - 0.000_002_27 * t.powi(3)
            + 0.000_000_011 * t.powi(4),
    );
    let omega = normalize_degrees(
        124.774_6 - 1.563_755_88 * k + 0.002_067_2 * t.powi(2) + 0.000_002_15 * t.powi(3),
    );

    let f1 = f - 0.024_65 * sin_deg(omega);
    if sin_deg(f1).abs() > NODE_LIMIT {
        return None;
    }

    let a1 = normalize_degrees(299.77 + 0.107_408 * k - 0.009_173 * t.powi(2));

    let p = 0.207_0 * e * sin_deg(m) + 0.002_4 * e * sin_deg(2.0 * m) - 0.039_2 * sin_deg(mp)
        + 0.011_6 * sin_deg(2.0 * mp)
        - 0.007_3 * e * sin_deg(mp + m)
        + 0.006_7 * e * sin_deg(mp - m)
        + 0.011_8 * sin_deg(2.0 * f1);

    let q = 5.220_7 - 0.004_8 * e * cos_deg(m) + 0.002_0 * e * cos_deg(2.0 * m)
        - 0.329_9 * cos_deg(mp)
        - 0.006_0 * e * cos_deg(mp + m)
        + 0.004_1 * e * cos_deg(mp - m);

    let w = cos_deg(f1).abs();

    let gamma = (p * cos_deg(f1) + q * sin_deg(f1)) * (1.0 - 0.004_8 * w);

    let u = 0.005_9 + 0.004_6 * e * cos_deg(m) - 0.018_2 * cos_deg(mp)
        + 0.000_4 * cos_deg(2.0 * mp)
        - 0.000_5 * cos_deg(m + mp);

    let (eclipse, magnitude) = match quarter {
        Quarter::New => classify_solar(gamma, u)?,
        _ => classify_lunar(gamma, u)?,
    };

    let jde = syzygy_jde(k, quarter, e, m, mp, f, omega, a1, t);

    Some(EclipseEvent {
        eclipse,
        label: eclipse.label(),
        solar: eclipse.is_solar(),
        at: dynamical_to_utc(jde),
        magnitude,
    })
}

fn classify_solar(gamma: f64, u: f64) -> Option<(Eclipse, f64)> {
    let distance = gamma.abs();

    if distance > 1.5433 + u {
        return None;
    }

    if distance > 0.9972 {
        let magnitude = (1.5433 + u - distance) / (0.5461 + 2.0 * u);
        return Some((
            Eclipse::Solar(SolarKind::Partial),
            magnitude.clamp(0.0, 1.0),
        ));
    }

    let kind = if u < 0.0 {
        SolarKind::Total
    } else if u > 0.0047 {
        SolarKind::Annular
    } else {
        let omega = 0.004_64 * (1.0 - gamma * gamma).max(0.0).sqrt();
        if u < omega {
            SolarKind::Hybrid
        } else {
            SolarKind::Annular
        }
    };

    Some((Eclipse::Solar(kind), 1.0))
}

fn classify_lunar(gamma: f64, u: f64) -> Option<(Eclipse, f64)> {
    let distance = gamma.abs();

    let umbral = (1.012_8 - u - distance) / 0.545_0;
    let penumbral = (1.557_3 + u - distance) / 0.545_0;

    if penumbral < 0.0 {
        return None;
    }

    if umbral < 0.0 {
        return Some((Eclipse::Lunar(LunarKind::Penumbral), penumbral));
    }

    if umbral >= 1.0 {
        return Some((Eclipse::Lunar(LunarKind::Total), umbral));
    }

    Some((Eclipse::Lunar(LunarKind::Partial), umbral))
}

#[allow(clippy::too_many_arguments)]
fn syzygy_jde(
    k: f64,
    quarter: Quarter,
    e: f64,
    m: f64,
    mp: f64,
    f: f64,
    omega: f64,
    a1: f64,
    t: f64,
) -> f64 {
    let mean = 2_451_550.097_66 + 29.530_588_861 * k + 0.000_154_37 * t.powi(2)
        - 0.000_000_150 * t.powi(3)
        + 0.000_000_000_73 * t.powi(4);

    let leading = match quarter {
        Quarter::New => -0.407_20 * sin_deg(mp) + 0.172_41 * e * sin_deg(m),
        _ => -0.406_14 * sin_deg(mp) + 0.173_02 * e * sin_deg(m),
    };

    let correction = leading
        + 0.016_08 * sin_deg(2.0 * mp)
        + 0.010_39 * sin_deg(2.0 * f)
        + 0.007_39 * e * sin_deg(mp - m)
        - 0.005_14 * e * sin_deg(mp + m)
        + 0.002_08 * e * e * sin_deg(2.0 * m)
        - 0.001_11 * sin_deg(mp - 2.0 * f)
        - 0.000_57 * sin_deg(mp + 2.0 * f)
        + 0.000_56 * e * sin_deg(2.0 * mp + m)
        - 0.000_42 * sin_deg(3.0 * mp)
        + 0.000_42 * e * sin_deg(m + 2.0 * f)
        + 0.000_38 * e * sin_deg(m - 2.0 * f)
        - 0.000_24 * e * sin_deg(2.0 * mp - m)
        - 0.000_17 * sin_deg(omega)
        + 0.000_325 * sin_deg(a1);

    mean + correction
}

pub fn within(at: DateTime<Utc>, window: TimeDelta) -> Vec<EclipseEvent> {
    upcoming(at)
        .into_iter()
        .filter(|event| event.at - at <= window)
        .collect()
}

pub fn at_lunation(k: f64, quarter: Quarter) -> Option<EclipseEvent> {
    examine(k, quarter)
}

pub fn lunation_near(at: DateTime<Utc>) -> f64 {
    let year = super::time::approximate_year(to_julian(at));
    ((year - 2000.0) * 12.368_5).round()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone};

    fn utc(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap()
    }

    fn on(year: i32, month: u32, day: u32, event: &EclipseEvent) -> bool {
        event.at.year() == year && event.at.month() == month && event.at.day() == day
    }

    #[test]
    fn the_total_solar_eclipse_of_august_2026() {
        let found = next_solar(utc(2026, 7, 1)).expect("an eclipse is due");

        assert!(on(2026, 8, 12, &found), "found {}", found.at);
        assert_eq!(found.eclipse, Eclipse::Solar(SolarKind::Total));
    }

    #[test]
    fn the_annular_solar_eclipse_of_february_2027() {
        let found = next_solar(utc(2027, 1, 1)).expect("an eclipse is due");

        assert!(on(2027, 2, 6, &found), "found {}", found.at);
        assert_eq!(found.eclipse, Eclipse::Solar(SolarKind::Annular));
    }

    #[test]
    fn the_great_total_solar_eclipse_of_august_2027() {
        let found = next_solar(utc(2027, 7, 1)).expect("an eclipse is due");

        assert!(on(2027, 8, 2, &found), "found {}", found.at);
        assert_eq!(found.eclipse, Eclipse::Solar(SolarKind::Total));
    }

    #[test]
    fn the_total_lunar_eclipse_of_march_2026() {
        let found = next_lunar(utc(2026, 2, 1)).expect("an eclipse is due");

        assert!(on(2026, 3, 3, &found), "found {}", found.at);
        assert_eq!(found.eclipse, Eclipse::Lunar(LunarKind::Total));
    }

    #[test]
    fn the_partial_lunar_eclipse_of_august_2026() {
        let found = next_lunar(utc(2026, 8, 1)).expect("an eclipse is due");

        assert!(on(2026, 8, 28, &found), "found {}", found.at);
        assert_eq!(found.eclipse, Eclipse::Lunar(LunarKind::Partial));
    }

    #[test]
    fn a_solar_eclipse_always_falls_on_a_new_moon() {
        let mut at = utc(2026, 1, 1);

        for _ in 0..8 {
            let found = next_solar(at).expect("eclipses keep coming");
            let lit = moon::illumination(found.at);
            assert!(lit < 0.02, "{} was {lit} lit at a solar eclipse", found.at);
            at = found.at + TimeDelta::days(1);
        }
    }

    #[test]
    fn a_lunar_eclipse_always_falls_on_a_full_moon() {
        let mut at = utc(2026, 1, 1);

        for _ in 0..8 {
            let found = next_lunar(at).expect("eclipses keep coming");
            let lit = moon::illumination(found.at);
            assert!(lit > 0.98, "{} was {lit} lit at a lunar eclipse", found.at);
            at = found.at + TimeDelta::days(1);
        }
    }

    #[test]
    fn eclipses_come_in_seasons_a_few_months_apart() {
        for month in 1..=12 {
            let at = utc(2026, month, 1);
            for found in [
                next_solar(at).expect("a solar eclipse is due"),
                next_lunar(at).expect("a lunar eclipse is due"),
            ] {
                assert!(found.at > at);
                let days = (found.at - at).as_seconds_f64() / 86_400.0;
                assert!(days < 220.0, "{month}: {} is {days} days out", found.label);
            }
        }
    }

    #[test]
    fn a_year_holds_between_four_and_seven_eclipses() {
        let mut count = 0;
        let mut at = utc(2026, 1, 1);
        let end = utc(2027, 1, 1);

        while at < end {
            let next = upcoming(at).into_iter().next();
            let Some(found) = next else { break };
            if found.at >= end {
                break;
            }

            count += 1;
            at = found.at + TimeDelta::days(1);
        }

        assert!((4..=7).contains(&count), "2026 came out with {count}");
    }

    #[test]
    fn upcoming_returns_both_kinds_in_order() {
        let at = utc(2026, 6, 1);
        let found = upcoming(at);

        assert_eq!(found.len(), 2);
        assert!(found[0].at <= found[1].at);
        assert_ne!(found[0].solar, found[1].solar);
        assert!(found.iter().all(|event| event.at > at));
    }

    #[test]
    fn a_total_lunar_eclipse_is_darker_than_a_penumbral_one() {
        let mut totals = 0;
        let mut penumbrals = 0;
        let mut at = utc(2026, 1, 1);

        for _ in 0..20 {
            let found = next_lunar(at).unwrap();
            match found.eclipse {
                Eclipse::Lunar(LunarKind::Total) => {
                    assert!(found.magnitude >= 1.0, "{:?}", found);
                    totals += 1;
                }
                Eclipse::Lunar(LunarKind::Partial) => {
                    assert!((0.0..1.0).contains(&found.magnitude), "{:?}", found)
                }
                Eclipse::Lunar(LunarKind::Penumbral) => {
                    assert!(found.magnitude >= 0.0, "{:?}", found);
                    penumbrals += 1;
                }
                Eclipse::Solar(_) => panic!("a solar eclipse came back from the lunar search"),
            }
            at = found.at + TimeDelta::days(1);
        }

        assert!(totals > 0, "twenty lunar eclipses with no total among them");
        assert!(
            penumbrals > 0,
            "twenty lunar eclipses with no penumbral one"
        );
    }

    #[test]
    fn nothing_is_found_at_a_syzygy_far_from_a_node() {
        let k = lunation_near(utc(2026, 5, 1));
        let solar = at_lunation(k, Quarter::New);
        let lunar = at_lunation(k + 0.5, Quarter::Full);

        assert!(solar.is_none() || lunar.is_none() || solar.unwrap().solar != lunar.unwrap().solar);
    }

    #[test]
    fn the_window_filter_only_keeps_what_falls_inside_it() {
        let at = utc(2026, 8, 1);
        let soon = within(at, TimeDelta::days(20));

        assert!(soon.iter().all(|event| (event.at - at).num_days() <= 20));
        assert!(within(at, TimeDelta::days(0)).is_empty());
    }
}
