use super::time::angle_difference;
use super::{moon, sun};
use chrono::{DateTime, TimeDelta, Utc};
use serde::Serialize;

const WASHED_OUT: f64 = 0.60;
const DIMMED: f64 = 0.30;
const SEARCH_DAYS: i64 = 370;

#[derive(Debug, Clone, Copy)]
pub struct Shower {
    pub name: &'static str,
    pub radiant: &'static str,
    pub parent: &'static str,
    peak_longitude: f64,
    pub zenith_hourly_rate: u32,
}

pub const SHOWERS: [Shower; 12] = [
    Shower {
        name: "Quadrantids",
        radiant: "Boötes",
        parent: "asteroid 2003 EH1",
        peak_longitude: 283.15,
        zenith_hourly_rate: 110,
    },
    Shower {
        name: "Lyrids",
        radiant: "Lyra",
        parent: "comet Thatcher",
        peak_longitude: 32.32,
        zenith_hourly_rate: 18,
    },
    Shower {
        name: "Eta Aquariids",
        radiant: "Aquarius",
        parent: "comet Halley",
        peak_longitude: 45.5,
        zenith_hourly_rate: 50,
    },
    Shower {
        name: "Southern Delta Aquariids",
        radiant: "Aquarius",
        parent: "comet 96P/Machholz",
        peak_longitude: 127.0,
        zenith_hourly_rate: 25,
    },
    Shower {
        name: "Alpha Capricornids",
        radiant: "Capricornus",
        parent: "comet 169P/NEAT",
        peak_longitude: 127.0,
        zenith_hourly_rate: 5,
    },
    Shower {
        name: "Perseids",
        radiant: "Perseus",
        parent: "comet Swift-Tuttle",
        peak_longitude: 140.0,
        zenith_hourly_rate: 100,
    },
    Shower {
        name: "Draconids",
        radiant: "Draco",
        parent: "comet 21P/Giacobini-Zinner",
        peak_longitude: 195.4,
        zenith_hourly_rate: 10,
    },
    Shower {
        name: "Orionids",
        radiant: "Orion",
        parent: "comet Halley",
        peak_longitude: 208.0,
        zenith_hourly_rate: 20,
    },
    Shower {
        name: "Southern Taurids",
        radiant: "Taurus",
        parent: "comet 2P/Encke",
        peak_longitude: 223.0,
        zenith_hourly_rate: 5,
    },
    Shower {
        name: "Leonids",
        radiant: "Leo",
        parent: "comet Tempel-Tuttle",
        peak_longitude: 235.27,
        zenith_hourly_rate: 15,
    },
    Shower {
        name: "Geminids",
        radiant: "Gemini",
        parent: "asteroid 3200 Phaethon",
        peak_longitude: 262.2,
        zenith_hourly_rate: 150,
    },
    Shower {
        name: "Ursids",
        radiant: "Ursa Minor",
        parent: "comet 8P/Tuttle",
        peak_longitude: 270.7,
        zenith_hourly_rate: 10,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Moonlight {
    Dark,
    Some,
    WashedOut,
}

impl Moonlight {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark skies at the peak",
            Self::Some => "Some moonlight at the peak",
            Self::WashedOut => "A bright moon will drown it",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ShowerPeak {
    pub name: &'static str,
    pub radiant: &'static str,
    pub parent: &'static str,
    pub zenith_hourly_rate: u32,
    pub peak: DateTime<Utc>,
    pub moon_illumination: f64,
    pub moonlight: Moonlight,
    pub moonlight_label: &'static str,
}

pub fn upcoming(at: DateTime<Utc>) -> Vec<ShowerPeak> {
    let mut peaks: Vec<ShowerPeak> = SHOWERS
        .iter()
        .filter_map(|shower| next_peak(shower, at))
        .collect();

    peaks.sort_by_key(|peak| peak.peak);
    peaks
}

pub fn next(at: DateTime<Utc>) -> Option<ShowerPeak> {
    upcoming(at).into_iter().next()
}

pub fn next_peak(shower: &Shower, at: DateTime<Utc>) -> Option<ShowerPeak> {
    let peak = solar_longitude_after(shower.peak_longitude, at)?;
    let illumination = moon::illumination(peak);

    let moonlight = if illumination >= WASHED_OUT {
        Moonlight::WashedOut
    } else if illumination >= DIMMED {
        Moonlight::Some
    } else {
        Moonlight::Dark
    };

    Some(ShowerPeak {
        name: shower.name,
        radiant: shower.radiant,
        parent: shower.parent,
        zenith_hourly_rate: shower.zenith_hourly_rate,
        peak,
        moon_illumination: illumination,
        moonlight,
        moonlight_label: moonlight.label(),
    })
}

fn solar_longitude_after(target: f64, at: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let difference = |offset: f64| -> f64 {
        angle_difference(
            target,
            sun::apparent_longitude(at + TimeDelta::milliseconds((offset * 86_400_000.0) as i64)),
        )
    };

    let mut previous = difference(0.0);

    for day in 1..=SEARCH_DAYS {
        let current = difference(day as f64);

        if previous < 0.0 && current >= 0.0 {
            let crossing = bisect(&difference, (day - 1) as f64, day as f64);
            return Some(at + TimeDelta::milliseconds((crossing * 86_400_000.0) as i64));
        }

        previous = current;
    }

    None
}

fn bisect(difference: &dyn Fn(f64) -> f64, mut low: f64, mut high: f64) -> f64 {
    for _ in 0..40 {
        let middle = (low + high) / 2.0;
        if difference(middle) < 0.0 {
            low = middle;
        } else {
            high = middle;
        }
    }

    (low + high) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone};

    fn utc(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap()
    }

    #[test]
    fn the_perseids_peak_in_the_middle_of_august() {
        let found = next_peak(&SHOWERS[5], utc(2026, 1, 1)).expect("the Perseids come every year");
        assert_eq!(found.name, "Perseids");
        assert_eq!(found.peak.month(), 8);
        assert!(
            (11..=14).contains(&found.peak.day()),
            "peaked on {}",
            found.peak
        );
    }

    #[test]
    fn every_shower_peaks_where_the_observing_guides_say() {
        for (name, month, days) in [
            ("Quadrantids", 1, 2..=4),
            ("Lyrids", 4, 21..=23),
            ("Eta Aquariids", 5, 5..=7),
            ("Perseids", 8, 11..=14),
            ("Orionids", 10, 20..=23),
            ("Leonids", 11, 16..=18),
            ("Geminids", 12, 13..=15),
            ("Ursids", 12, 21..=23),
        ] {
            let shower = SHOWERS
                .iter()
                .find(|shower| shower.name == name)
                .expect("the shower is in the table");
            let found = next_peak(shower, utc(2026, 1, 1)).expect("a peak is due");

            assert_eq!(found.peak.month(), month, "{name} peaked on {}", found.peak);
            assert!(
                days.contains(&found.peak.day()),
                "{name} peaked on {}",
                found.peak
            );
        }
    }

    #[test]
    fn a_shower_peaks_within_a_year_from_any_starting_point() {
        for month in 1..=12 {
            let at = utc(2026, month, 15);
            let found = next_peak(&SHOWERS[5], at).expect("the Perseids come every year");

            assert!(found.peak > at);
            let days = (found.peak - at).as_seconds_f64() / 86_400.0;
            assert!(days <= 366.0, "{month}: {days} days away");
        }
    }

    #[test]
    fn peaks_come_back_soonest_first_and_all_lie_ahead() {
        let at = utc(2026, 8, 8);
        let peaks = upcoming(at);

        assert_eq!(peaks.len(), SHOWERS.len());
        for pair in peaks.windows(2) {
            assert!(pair[0].peak <= pair[1].peak);
        }
        assert!(peaks.iter().all(|peak| peak.peak > at));
    }

    #[test]
    fn the_moon_verdict_follows_the_illumination() {
        for peak in upcoming(utc(2026, 1, 1)) {
            let expected = match peak.moon_illumination {
                lit if lit >= WASHED_OUT => Moonlight::WashedOut,
                lit if lit >= DIMMED => Moonlight::Some,
                _ => Moonlight::Dark,
            };
            assert_eq!(peak.moonlight, expected, "{}", peak.name);
            assert!((0.0..=1.0).contains(&peak.moon_illumination));
        }
    }

    #[test]
    fn the_2026_perseids_fall_on_a_new_moon() {
        let shower = SHOWERS.iter().find(|s| s.name == "Perseids").unwrap();
        let found = next_peak(shower, utc(2026, 7, 1)).unwrap();

        assert!(
            found.moon_illumination < 0.05,
            "the moon was {:.0}% lit at the 2026 Perseids",
            found.moon_illumination * 100.0
        );
        assert_eq!(found.moonlight, Moonlight::Dark);
    }

    #[test]
    fn the_2025_perseids_were_washed_out() {
        let shower = SHOWERS.iter().find(|s| s.name == "Perseids").unwrap();
        let found = next_peak(shower, utc(2025, 7, 1)).unwrap();

        assert!(
            found.moon_illumination > 0.8,
            "the moon was {:.0}% lit at the 2025 Perseids",
            found.moon_illumination * 100.0
        );
        assert_eq!(found.moonlight, Moonlight::WashedOut);
    }

    #[test]
    fn a_solar_longitude_search_lands_on_the_longitude_it_was_asked_for() {
        for target in [0.0, 45.5, 140.0, 283.15, 359.0] {
            let found = solar_longitude_after(target, utc(2026, 3, 1)).expect("the sun gets there");
            let landed = sun::apparent_longitude(found);

            assert!(
                angle_difference(target, landed).abs() < 0.001,
                "asked for {target}, landed on {landed}"
            );
        }
    }

    #[test]
    fn the_table_is_in_calendar_order_and_has_no_duplicates() {
        let peaks: Vec<DateTime<Utc>> = SHOWERS
            .iter()
            .map(|shower| {
                next_peak(shower, utc(2026, 1, 1))
                    .expect("a peak is due")
                    .peak
            })
            .collect();

        for (index, pair) in peaks.windows(2).enumerate() {
            assert!(
                pair[0] <= pair[1],
                "{} peaks after {}",
                SHOWERS[index].name,
                SHOWERS[index + 1].name
            );
        }

        for shower in &SHOWERS {
            assert!(
                (0.0..360.0).contains(&shower.peak_longitude),
                "{} sits at {} degrees",
                shower.name,
                shower.peak_longitude
            );
        }

        let mut names: Vec<&str> = SHOWERS.iter().map(|shower| shower.name).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "a shower is in the table twice");
    }
}
