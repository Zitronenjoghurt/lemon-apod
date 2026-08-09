use super::time::{
    self, centuries, cos_deg, dynamical_julian, dynamical_to_utc, normalize_degrees, sin_deg,
    to_julian,
};
use chrono::{DateTime, Utc};
use serde::Serialize;

const MEAN_DISTANCE_KM: f64 = 385_000.56;
const SUPERMOON_KM: f64 = 360_000.0;
pub const PERIGEE_KM: f64 = 356_500.0;
pub const APOGEE_KM: f64 = 406_700.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    New,
    WaxingCrescent,
    FirstQuarter,
    WaxingGibbous,
    Full,
    WaningGibbous,
    LastQuarter,
    WaningCrescent,
}

impl Phase {
    pub const fn label(self) -> &'static str {
        match self {
            Self::New => "New moon",
            Self::WaxingCrescent => "Waxing crescent",
            Self::FirstQuarter => "First quarter",
            Self::WaxingGibbous => "Waxing gibbous",
            Self::Full => "Full moon",
            Self::WaningGibbous => "Waning gibbous",
            Self::LastQuarter => "Last quarter",
            Self::WaningCrescent => "Waning crescent",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Quarter {
    New,
    First,
    Full,
    Last,
}

impl Quarter {
    pub const ALL: [Self; 4] = [Self::New, Self::First, Self::Full, Self::Last];

    pub const fn offset(self) -> f64 {
        match self {
            Self::New => 0.0,
            Self::First => 0.25,
            Self::Full => 0.5,
            Self::Last => 0.75,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::New => "New moon",
            Self::First => "First quarter",
            Self::Full => "Full moon",
            Self::Last => "Last quarter",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct QuarterEvent {
    pub quarter: Quarter,
    pub label: &'static str,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MoonNow {
    pub phase: Phase,
    pub label: &'static str,
    pub illumination: f64,
    pub age_days: f64,
    pub waxing: bool,
    pub distance_km: f64,
    pub perigee_km: f64,
    pub apogee_km: f64,
    pub closing: bool,
    pub cycle: f64,
    pub last_new_moon: DateTime<Utc>,
    pub next_quarters: Vec<QuarterEvent>,
}

pub fn now(at: DateTime<Utc>) -> MoonNow {
    let last_new = last_new_moon(at);
    let next_new = next_quarter(at, Quarter::New);

    let lunation = (next_new - last_new).as_seconds_f64().max(1.0);
    let age_seconds = (at - last_new).as_seconds_f64().max(0.0);
    let cycle = (age_seconds / lunation).clamp(0.0, 1.0);

    let mut next_quarters: Vec<QuarterEvent> = Quarter::ALL
        .iter()
        .map(|&quarter| QuarterEvent {
            quarter,
            label: quarter.label(),
            at: next_quarter(at, quarter),
        })
        .collect();
    next_quarters.sort_by_key(|event| event.at);

    let waxing = next_quarters
        .iter()
        .find(|event| matches!(event.quarter, Quarter::New | Quarter::Full))
        .is_some_and(|event| event.quarter == Quarter::Full);

    MoonNow {
        phase: phase_for(cycle),
        label: phase_for(cycle).label(),
        illumination: illumination(at),
        age_days: age_seconds / 86_400.0,
        waxing,
        distance_km: distance_km(at),
        perigee_km: PERIGEE_KM,
        apogee_km: APOGEE_KM,
        closing: is_closing(at),
        cycle,
        last_new_moon: last_new,
        next_quarters,
    }
}

fn phase_for(cycle: f64) -> Phase {
    match cycle {
        c if !(0.023..0.977).contains(&c) => Phase::New,
        c if c < 0.227 => Phase::WaxingCrescent,
        c if c < 0.273 => Phase::FirstQuarter,
        c if c < 0.477 => Phase::WaxingGibbous,
        c if c < 0.523 => Phase::Full,
        c if c < 0.727 => Phase::WaningGibbous,
        c if c < 0.773 => Phase::LastQuarter,
        _ => Phase::WaningCrescent,
    }
}

pub fn illumination(at: DateTime<Utc>) -> f64 {
    let t = centuries(dynamical_julian(at));

    let d = normalize_degrees(
        297.850_192_1 + 445_267.111_403_4 * t - 0.001_881_9 * t.powi(2) + t.powi(3) / 545_868.0
            - t.powi(4) / 113_065_000.0,
    );
    let m = normalize_degrees(
        357.529_109_2 + 35_999.050_290_9 * t - 0.000_153_6 * t.powi(2) + t.powi(3) / 24_490_000.0,
    );
    let m_prime = normalize_degrees(
        134.963_396_4 + 477_198.867_505_5 * t + 0.008_741_4 * t.powi(2) + t.powi(3) / 69_699.0
            - t.powi(4) / 14_712_000.0,
    );

    let phase_angle = 180.0 - d - 6.289 * sin_deg(m_prime) + 2.100 * sin_deg(m)
        - 1.274 * sin_deg(2.0 * d - m_prime)
        - 0.658 * sin_deg(2.0 * d)
        - 0.214 * sin_deg(2.0 * m_prime)
        - 0.110 * sin_deg(d);

    ((1.0 + cos_deg(phase_angle)) / 2.0).clamp(0.0, 1.0)
}

const DISTANCE_TERMS: [(f64, f64, f64, f64, f64); 46] = [
    (0.0, 0.0, 1.0, 0.0, -20_905_355.0),
    (2.0, 0.0, -1.0, 0.0, -3_699_111.0),
    (2.0, 0.0, 0.0, 0.0, -2_955_968.0),
    (0.0, 0.0, 2.0, 0.0, -569_925.0),
    (0.0, 1.0, 0.0, 0.0, 48_888.0),
    (0.0, 0.0, 0.0, 2.0, -3_149.0),
    (2.0, 0.0, -2.0, 0.0, 246_158.0),
    (2.0, -1.0, -1.0, 0.0, -152_138.0),
    (2.0, 0.0, 1.0, 0.0, -170_733.0),
    (2.0, -1.0, 0.0, 0.0, -204_586.0),
    (0.0, 1.0, -1.0, 0.0, -129_620.0),
    (1.0, 0.0, 0.0, 0.0, 108_743.0),
    (0.0, 1.0, 1.0, 0.0, 104_755.0),
    (2.0, 0.0, 0.0, -2.0, 10_321.0),
    (0.0, 0.0, 1.0, -2.0, 79_661.0),
    (4.0, 0.0, -1.0, 0.0, -34_782.0),
    (0.0, 0.0, 3.0, 0.0, -23_210.0),
    (4.0, 0.0, -2.0, 0.0, -21_636.0),
    (2.0, 1.0, -1.0, 0.0, 24_208.0),
    (2.0, 1.0, 0.0, 0.0, 30_824.0),
    (1.0, 0.0, -1.0, 0.0, -8_379.0),
    (1.0, 1.0, 0.0, 0.0, -16_675.0),
    (2.0, -1.0, 1.0, 0.0, -12_831.0),
    (2.0, 0.0, 2.0, 0.0, -10_445.0),
    (4.0, 0.0, 0.0, 0.0, -11_650.0),
    (2.0, 0.0, -3.0, 0.0, 14_403.0),
    (0.0, 1.0, -2.0, 0.0, -7_003.0),
    (2.0, -1.0, -2.0, 0.0, 10_056.0),
    (1.0, 0.0, 1.0, 0.0, 6_322.0),
    (2.0, -2.0, 0.0, 0.0, -9_884.0),
    (0.0, 1.0, 2.0, 0.0, 5_751.0),
    (2.0, -2.0, -1.0, 0.0, -4_950.0),
    (2.0, 0.0, 1.0, -2.0, 4_130.0),
    (4.0, -1.0, -1.0, 0.0, -3_958.0),
    (3.0, 0.0, -1.0, 0.0, 3_258.0),
    (2.0, 1.0, 1.0, 0.0, 2_616.0),
    (4.0, -1.0, -2.0, 0.0, -1_897.0),
    (0.0, 2.0, -1.0, 0.0, -2_117.0),
    (2.0, 2.0, -1.0, 0.0, 2_354.0),
    (4.0, 0.0, 1.0, 0.0, -1_423.0),
    (0.0, 0.0, 4.0, 0.0, -1_117.0),
    (4.0, -1.0, 0.0, 0.0, -1_571.0),
    (1.0, 0.0, -2.0, 0.0, -1_739.0),
    (0.0, 0.0, 2.0, -2.0, -4_421.0),
    (0.0, 2.0, 1.0, 0.0, 1_165.0),
    (2.0, 0.0, -1.0, -2.0, 8_752.0),
];

pub fn distance_km(at: DateTime<Utc>) -> f64 {
    distance_at(dynamical_julian(at))
}

fn distance_at(jde: f64) -> f64 {
    let t = centuries(jde);

    let d = normalize_degrees(
        297.850_192_1 + 445_267.111_403_4 * t - 0.001_881_9 * t.powi(2) + t.powi(3) / 545_868.0
            - t.powi(4) / 113_065_000.0,
    );
    let m = normalize_degrees(
        357.529_109_2 + 35_999.050_290_9 * t - 0.000_153_6 * t.powi(2) + t.powi(3) / 24_490_000.0,
    );
    let m_prime = normalize_degrees(
        134.963_396_4 + 477_198.867_505_5 * t + 0.008_741_4 * t.powi(2) + t.powi(3) / 69_699.0
            - t.powi(4) / 14_712_000.0,
    );
    let f = normalize_degrees(
        93.272_095_0 + 483_202.017_523_3 * t - 0.003_653_9 * t.powi(2) - t.powi(3) / 3_526_000.0
            + t.powi(4) / 863_310_000.0,
    );

    let e = 1.0 - 0.002_516 * t - 0.000_007_4 * t.powi(2);

    let sum: f64 = DISTANCE_TERMS
        .iter()
        .map(|&(cd, cm, cm_prime, cf, coefficient)| {
            let argument = cd * d + cm * m + cm_prime * m_prime + cf * f;
            coefficient * e.powi(cm.abs() as i32) * cos_deg(argument)
        })
        .sum();

    MEAN_DISTANCE_KM + sum / 1000.0
}

pub fn is_supermoon(at: DateTime<Utc>) -> bool {
    distance_km(at) < SUPERMOON_KM
}

pub fn is_closing(at: DateTime<Utc>) -> bool {
    distance_km(at + chrono::TimeDelta::hours(6)) < distance_km(at)
}

pub fn last_new_moon(at: DateTime<Utc>) -> DateTime<Utc> {
    let base = lunation_index(at).floor();

    (-2..=2)
        .map(|step| phase_instant(base + f64::from(step), Quarter::New))
        .filter(|instant| *instant <= at)
        .max()
        .unwrap_or_else(|| phase_instant(base - 3.0, Quarter::New))
}

pub fn next_quarter(at: DateTime<Utc>, quarter: Quarter) -> DateTime<Utc> {
    let base = lunation_index(at).floor();

    (-2..=2)
        .map(|step| phase_instant(base + f64::from(step) + quarter.offset(), quarter))
        .filter(|instant| *instant > at)
        .min()
        .unwrap_or_else(|| phase_instant(base + 3.0 + quarter.offset(), quarter))
}

pub fn lunation_index(at: DateTime<Utc>) -> f64 {
    let year = time::approximate_year(to_julian(at));
    (year - 2000.0) * 12.368_5
}

pub fn phase_instant(k: f64, quarter: Quarter) -> DateTime<Utc> {
    dynamical_to_utc(phase_jde(k, quarter))
}

fn phase_jde(k: f64, quarter: Quarter) -> f64 {
    let t = k / 1236.85;

    let mean = 2_451_550.097_66 + 29.530_588_861 * k + 0.000_154_37 * t.powi(2)
        - 0.000_000_150 * t.powi(3)
        + 0.000_000_000_73 * t.powi(4);

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

    let correction = match quarter {
        Quarter::New => new_moon_correction(e, m, mp, f, omega),
        Quarter::Full => full_moon_correction(e, m, mp, f, omega),
        Quarter::First | Quarter::Last => {
            let base = quarter_correction(e, m, mp, f, omega);
            let w = 0.003_06 - 0.000_38 * e * cos_deg(m) + 0.000_26 * cos_deg(mp)
                - 0.000_02 * cos_deg(mp - m)
                + 0.000_02 * cos_deg(mp + m)
                + 0.000_02 * cos_deg(2.0 * f);

            base + if quarter == Quarter::First { w } else { -w }
        }
    };

    mean + correction + additional_correction(k, t)
}

fn new_moon_correction(e: f64, m: f64, mp: f64, f: f64, omega: f64) -> f64 {
    -0.407_20 * sin_deg(mp)
        + 0.172_41 * e * sin_deg(m)
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
        - 0.000_07 * sin_deg(mp + 2.0 * m)
        + 0.000_04 * sin_deg(2.0 * mp - 2.0 * f)
        + 0.000_04 * sin_deg(3.0 * m)
        + 0.000_03 * sin_deg(mp + m - 2.0 * f)
        + 0.000_03 * sin_deg(2.0 * mp + 2.0 * f)
        - 0.000_03 * sin_deg(mp + m + 2.0 * f)
        + 0.000_03 * sin_deg(mp - m + 2.0 * f)
        - 0.000_02 * sin_deg(mp - m - 2.0 * f)
        - 0.000_02 * sin_deg(3.0 * mp + m)
        + 0.000_02 * sin_deg(4.0 * mp)
}

fn full_moon_correction(e: f64, m: f64, mp: f64, f: f64, omega: f64) -> f64 {
    -0.406_14 * sin_deg(mp)
        + 0.173_02 * e * sin_deg(m)
        + 0.016_14 * sin_deg(2.0 * mp)
        + 0.010_43 * sin_deg(2.0 * f)
        + 0.007_34 * e * sin_deg(mp - m)
        - 0.005_15 * e * sin_deg(mp + m)
        + 0.002_09 * e * e * sin_deg(2.0 * m)
        - 0.001_11 * sin_deg(mp - 2.0 * f)
        - 0.000_57 * sin_deg(mp + 2.0 * f)
        + 0.000_56 * e * sin_deg(2.0 * mp + m)
        - 0.000_42 * sin_deg(3.0 * mp)
        + 0.000_42 * e * sin_deg(m + 2.0 * f)
        + 0.000_38 * e * sin_deg(m - 2.0 * f)
        - 0.000_24 * e * sin_deg(2.0 * mp - m)
        - 0.000_17 * sin_deg(omega)
        - 0.000_07 * sin_deg(mp + 2.0 * m)
        + 0.000_04 * sin_deg(2.0 * mp - 2.0 * f)
        + 0.000_04 * sin_deg(3.0 * m)
        + 0.000_03 * sin_deg(mp + m - 2.0 * f)
        + 0.000_03 * sin_deg(2.0 * mp + 2.0 * f)
        - 0.000_03 * sin_deg(mp + m + 2.0 * f)
        + 0.000_03 * sin_deg(mp - m + 2.0 * f)
        - 0.000_02 * sin_deg(mp - m - 2.0 * f)
        - 0.000_02 * sin_deg(3.0 * mp + m)
        + 0.000_02 * sin_deg(4.0 * mp)
}

fn quarter_correction(e: f64, m: f64, mp: f64, f: f64, omega: f64) -> f64 {
    -0.628_01 * sin_deg(mp) + 0.171_72 * e * sin_deg(m) - 0.011_83 * e * sin_deg(mp + m)
        + 0.008_62 * sin_deg(2.0 * mp)
        + 0.008_04 * sin_deg(2.0 * f)
        + 0.004_54 * e * sin_deg(mp - m)
        + 0.002_04 * e * e * sin_deg(2.0 * m)
        - 0.001_80 * sin_deg(mp - 2.0 * f)
        - 0.000_70 * sin_deg(mp + 2.0 * f)
        - 0.000_40 * sin_deg(3.0 * mp)
        - 0.000_34 * e * sin_deg(2.0 * mp - m)
        + 0.000_32 * e * sin_deg(m + 2.0 * f)
        + 0.000_32 * e * sin_deg(m - 2.0 * f)
        - 0.000_28 * e * e * sin_deg(mp + 2.0 * m)
        + 0.000_27 * e * sin_deg(2.0 * mp + m)
        - 0.000_17 * sin_deg(omega)
        - 0.000_05 * sin_deg(mp - m - 2.0 * f)
        + 0.000_04 * sin_deg(2.0 * mp + 2.0 * f)
        - 0.000_04 * sin_deg(mp + m + 2.0 * f)
        + 0.000_04 * sin_deg(mp - 2.0 * m)
        + 0.000_03 * sin_deg(mp + m - 2.0 * f)
        + 0.000_03 * sin_deg(3.0 * m)
        + 0.000_02 * sin_deg(2.0 * mp - 2.0 * f)
        + 0.000_02 * sin_deg(mp - m + 2.0 * f)
        - 0.000_02 * sin_deg(3.0 * mp + m)
}

fn additional_correction(k: f64, t: f64) -> f64 {
    const TERMS: [(f64, f64, f64, f64); 14] = [
        (299.77, 0.107_408, -0.009_173, 0.000_325),
        (251.88, 0.016_321, 0.0, 0.000_165),
        (251.83, 26.651_886, 0.0, 0.000_164),
        (349.42, 36.412_478, 0.0, 0.000_126),
        (84.66, 18.206_239, 0.0, 0.000_110),
        (141.74, 53.303_771, 0.0, 0.000_062),
        (207.14, 2.453_732, 0.0, 0.000_060),
        (154.84, 7.306_860, 0.0, 0.000_056),
        (34.52, 27.261_239, 0.0, 0.000_047),
        (207.19, 0.121_824, 0.0, 0.000_042),
        (291.34, 1.844_379, 0.0, 0.000_040),
        (161.72, 24.198_154, 0.0, 0.000_037),
        (239.56, 25.513_099, 0.0, 0.000_035),
        (331.55, 3.592_518, 0.0, 0.000_023),
    ];

    TERMS
        .iter()
        .map(|&(constant, per_k, per_t2, amplitude)| {
            amplitude * sin_deg(constant + per_k * k + per_t2 * t.powi(2))
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sky::time::SYNODIC_MONTH;
    use chrono::TimeZone;

    fn utc(y: i32, m: u32, d: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, hour, minute, 0).unwrap()
    }

    #[test]
    fn the_new_moon_of_meeus_example_49a() {
        let instant = phase_instant(-283.0, Quarter::New);
        let expected = utc(1977, 2, 18, 3, 36);

        let error = (instant - expected).as_seconds_f64().abs();
        assert!(error < 120.0, "got {instant}, expected about {expected}");
    }

    #[test]
    fn the_last_quarter_of_meeus_example_49b() {
        let instant = phase_instant(544.75, Quarter::Last);
        let expected = utc(2044, 1, 21, 23, 47);

        let error = (instant - expected).as_seconds_f64().abs();
        assert!(error < 120.0, "got {instant}, expected about {expected}");
    }

    #[test]
    fn full_moons_land_on_the_published_dates() {
        for (from, expected) in [
            (utc(2026, 1, 1, 0, 0), utc(2026, 1, 3, 10, 3)),
            (utc(2026, 8, 1, 0, 0), utc(2026, 8, 28, 4, 18)),
            (utc(2027, 3, 1, 0, 0), utc(2027, 3, 22, 10, 43)),
        ] {
            let found = next_quarter(from, Quarter::Full);
            let error = (found - expected).as_seconds_f64().abs();
            assert!(
                error < 300.0,
                "full moon after {from}: got {found}, expected {expected}"
            );
        }
    }

    #[test]
    fn a_new_moon_is_dark_and_a_full_moon_is_lit() {
        let new = next_quarter(utc(2026, 8, 1, 0, 0), Quarter::New);
        let full = next_quarter(utc(2026, 8, 1, 0, 0), Quarter::Full);

        assert!(illumination(new) < 0.005, "{}", illumination(new));
        assert!(illumination(full) > 0.995, "{}", illumination(full));
    }

    #[test]
    fn the_quarters_are_half_lit() {
        for quarter in [Quarter::First, Quarter::Last] {
            let at = next_quarter(utc(2026, 8, 1, 0, 0), quarter);
            let lit = illumination(at);
            assert!((0.48..0.52).contains(&lit), "{quarter:?} was {lit} lit");
        }
    }

    #[test]
    fn the_named_phase_agrees_with_the_illumination() {
        let start = utc(2026, 1, 1, 0, 0);

        for day in 0..400 {
            let at = start + chrono::TimeDelta::days(day);
            let state = now(at);

            match state.phase {
                Phase::New => assert!(state.illumination < 0.06, "{at}: {state:?}"),
                Phase::Full => assert!(state.illumination > 0.94, "{at}: {state:?}"),
                Phase::FirstQuarter | Phase::LastQuarter => {
                    assert!(
                        (0.35..0.65).contains(&state.illumination),
                        "{at}: {state:?}"
                    )
                }
                _ => {}
            }
        }
    }

    #[test]
    fn the_moon_fills_from_new_to_full_and_empties_back() {
        let mut new = next_quarter(utc(2026, 1, 1, 0, 0), Quarter::New);

        for _ in 0..12 {
            let full = next_quarter(new, Quarter::Full);
            let next_new = next_quarter(full, Quarter::New);

            for (from, to, filling) in [(new, full, true), (full, next_new, false)] {
                let span = (to - from).as_seconds_f64();
                let mut previous =
                    illumination(from + chrono::TimeDelta::seconds(span as i64 / 40));

                for step in 2..40 {
                    let at =
                        from + chrono::TimeDelta::seconds((span * f64::from(step) / 40.0) as i64);
                    let lit = illumination(at);

                    assert_eq!(
                        lit > previous,
                        filling,
                        "{at}: {previous:.4} to {lit:.4} while filling was {filling}"
                    );
                    assert_eq!(now(at).waxing, filling, "{at} disagreed about waxing");

                    previous = lit;
                }
            }

            new = next_new;
        }
    }

    #[test]
    fn the_age_never_leaves_the_lunation() {
        let start = utc(2026, 1, 1, 0, 0);

        for day in 0..400 {
            let state = now(start + chrono::TimeDelta::days(day));
            assert!(
                (0.0..=SYNODIC_MONTH + 0.5).contains(&state.age_days),
                "age was {} days",
                state.age_days
            );
            assert!((0.0..=1.0).contains(&state.cycle));
        }
    }

    #[test]
    fn the_next_quarters_come_back_in_order_and_all_lie_ahead() {
        let at = utc(2026, 8, 8, 12, 0);
        let state = now(at);

        assert_eq!(state.next_quarters.len(), 4);
        for pair in state.next_quarters.windows(2) {
            assert!(pair[0].at < pair[1].at, "{:?}", state.next_quarters);
        }
        assert!(state.next_quarters[0].at > at);

        let span = (state.next_quarters[3].at - state.next_quarters[0].at).as_seconds_f64();
        assert!(span < SYNODIC_MONTH * 86_400.0, "span was {span} seconds");
    }

    #[test]
    fn the_last_new_moon_is_behind_us_and_within_a_lunation() {
        let at = utc(2026, 8, 8, 12, 0);
        let last = last_new_moon(at);

        assert!(last <= at);
        assert!((at - last).as_seconds_f64() < SYNODIC_MONTH * 86_400.0);
    }

    #[test]
    fn the_distance_of_meeus_example_47a() {
        let found = distance_at(2_448_724.5);
        assert!(
            (found - 368_409.7).abs() < 0.5,
            "got {found}, expected about 368409.7"
        );
    }

    #[test]
    fn the_distance_stays_between_perigee_and_apogee() {
        let start = utc(2026, 1, 1, 0, 0);

        let mut closest = f64::MAX;
        let mut furthest = f64::MIN;
        for hours in 0..(24 * 400) {
            let km = distance_km(start + chrono::TimeDelta::hours(hours));
            closest = closest.min(km);
            furthest = furthest.max(km);
        }

        assert!((356_000.0..359_000.0).contains(&closest), "{closest}");
        assert!((405_000.0..407_500.0).contains(&furthest), "{furthest}");

        assert!(
            PERIGEE_KM <= closest + 1_000.0 && APOGEE_KM >= furthest - 1_000.0,
            "the ends the panel measures against have to bracket what the model produces"
        );
    }

    #[test]
    fn the_direction_of_travel_follows_the_distance() {
        let start = utc(2026, 1, 1, 0, 0);

        let mut turns = 0;
        let mut previous = is_closing(start);

        for hours in 1..(24 * 60) {
            let at = start + chrono::TimeDelta::hours(hours);
            let closing = is_closing(at);

            assert_eq!(
                closing,
                distance_km(at + chrono::TimeDelta::hours(6)) < distance_km(at),
                "at {at}"
            );

            if closing != previous {
                turns += 1;
                previous = closing;
            }
        }

        assert!((3..=6).contains(&turns), "{turns} turns in sixty days");
    }
}
