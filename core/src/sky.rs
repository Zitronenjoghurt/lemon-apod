pub mod eclipse;
pub mod moon;
pub mod planets;
pub mod showers;
#[cfg(feature = "sky-data")]
pub mod store;
pub mod sun;
pub mod time;
pub mod weather;

use chrono::{DateTime, TimeDelta, Utc};
use serde::Serialize;

pub use eclipse::{Eclipse, EclipseEvent, LunarKind, SolarKind};
pub use moon::{MoonNow, Phase, Quarter, QuarterEvent};
pub use planets::{Milestone, Planet, PlanetEvent, PlanetNow, Visibility};
pub use showers::{Moonlight, Shower, ShowerPeak};
pub use sun::{Turning, TurningEvent};
pub use weather::{Alert, Band, Notice, WeatherReport, WeatherSummary};

const TIMELINE_LENGTH: usize = 10;
const TIMELINE_HORIZON_DAYS: i64 = 400;
const TIMELINE_LOOKBACK_DAYS: i64 = 3;
const TIMELINE_PAST: usize = 3;
const SHOWERS_AHEAD: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Moon,
    Season,
    Shower,
    Eclipse,
    Planet,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkyEvent {
    pub kind: EventKind,
    pub title: String,
    pub detail: Option<String>,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkyNow {
    pub at: DateTime<Utc>,
    pub moon: MoonNow,
    pub season: TurningEvent,
    pub next_turning: TurningEvent,
    pub planets: Vec<PlanetNow>,
    pub showers: Vec<ShowerPeak>,
    pub eclipses: Vec<EclipseEvent>,
    pub events: Vec<SkyEvent>,
}

pub fn now(at: DateTime<Utc>) -> SkyNow {
    let moon = moon::now(at);
    let season = sun::current_season(at);
    let next_turning = sun::next_turning(at);
    let planets = planets::now(at);
    let showers: Vec<ShowerPeak> = showers::upcoming(at)
        .into_iter()
        .take(SHOWERS_AHEAD)
        .collect();
    let eclipses = eclipse::upcoming(at);

    let events = timeline(at);

    SkyNow {
        at,
        moon,
        season,
        next_turning,
        planets,
        showers,
        eclipses,
        events,
    }
}

/// The sky a few days back, rather than the sky right now, is what the timeline is built from: an
/// event that has only just happened is still worth a line. Everything else in [`SkyNow`] speaks
/// for the present, so this reaches for its own copies rather than sharing them.
fn timeline(at: DateTime<Utc>) -> Vec<SkyEvent> {
    let since = at - TimeDelta::days(TIMELINE_LOOKBACK_DAYS);
    let horizon = at + TimeDelta::days(TIMELINE_HORIZON_DAYS);

    let next_turning = sun::next_turning(since);
    let planets = planets::now(since);
    let showers: Vec<ShowerPeak> = showers::upcoming(since)
        .into_iter()
        .take(SHOWERS_AHEAD)
        .collect();
    let eclipses = eclipse::upcoming(since);

    let mut events = Vec::new();

    // Both phases at the dates they actually fall on. An eclipse is a syzygy, so a lunar one lands
    // on a full moon and a solar one on a new moon; where that happens the two share a line rather
    // than repeat each other, and the line is the eclipse with the phase named in its detail.
    // Reaching past it for the lunation after would put a full moon six weeks out on the list while
    // the real next one sat above it under another name.
    let phases: [QuarterEvent; 2] = [Quarter::New, Quarter::Full].map(|quarter| QuarterEvent {
        quarter,
        label: quarter.label(),
        at: moon::next_quarter(since, quarter),
    });

    let together =
        |one: DateTime<Utc>, other: DateTime<Utc>| (one - other).abs() < TimeDelta::hours(1);
    let eclipsed = |at: DateTime<Utc>| eclipses.iter().any(|found| together(found.at, at));
    let phase_at = |at: DateTime<Utc>| phases.iter().find(|phase| together(phase.at, at));

    for quarter in phases.iter().filter(|phase| !eclipsed(phase.at)) {
        let supermoon = quarter.quarter == Quarter::Full && moon::is_supermoon(quarter.at);

        events.push(SkyEvent {
            kind: EventKind::Moon,
            title: quarter.label.to_owned(),
            detail: supermoon
                .then(|| {
                    format!(
                        "A close one, {} km away",
                        thousands(moon::distance_km(quarter.at).round() as i64)
                    )
                })
                .or_else(|| {
                    (quarter.quarter == Quarter::Full).then(|| {
                        format!(
                            "{} km away",
                            thousands(moon::distance_km(quarter.at).round() as i64)
                        )
                    })
                }),
            at: quarter.at,
        });
    }

    events.push(SkyEvent {
        kind: EventKind::Season,
        title: next_turning.label.to_owned(),
        detail: Some(format!(
            "{} begins in the north, {} in the south",
            capitalized(next_turning.opens_northern),
            next_turning.opens_southern
        )),
        at: next_turning.at,
    });

    for shower in &showers {
        events.push(SkyEvent {
            kind: EventKind::Shower,
            title: format!("{} peak", shower.name),
            detail: Some(format!(
                "Up to {} an hour from {}. {}",
                shower.zenith_hourly_rate, shower.radiant, shower.moonlight_label
            )),
            at: shower.peak,
        });
    }

    for found in &eclipses {
        // Whichever phase it swallowed is named here, so the night it falls on still answers the
        // question the phase would have answered on a line of its own.
        let detail = match (phase_at(found.at), found.solar) {
            (Some(_), true) => "The new moon, crossing the sun along a track of its own".to_owned(),
            (Some(_), false) => format!(
                "The full moon, {} km away, inside Earth's shadow",
                thousands(moon::distance_km(found.at).round() as i64)
            ),
            (None, true) => "Visible along its own track only".to_owned(),
            (None, false) => "Visible from the whole night side of Earth".to_owned(),
        };

        events.push(SkyEvent {
            kind: EventKind::Eclipse,
            title: found.label.to_owned(),
            detail: Some(detail),
            at: found.at,
        });
    }

    for planet in planets.iter().filter(|planet| planet.naked_eye) {
        let Some(milestone) = &planet.next_milestone else {
            continue;
        };

        events.push(SkyEvent {
            kind: EventKind::Planet,
            title: format!("{} {}", milestone.name, milestone.label),
            detail: Some(match milestone.milestone {
                Milestone::Opposition => {
                    "Opposite the sun, so up all night and at its brightest".to_owned()
                }
                _ => format!(
                    "{:.0} degrees from the sun, its best showing of this apparition",
                    milestone.elongation
                ),
            }),
            at: milestone.at,
        });
    }

    events.retain(|event| event.at > since && event.at <= horizon);
    events.sort_by_key(|event| event.at);

    let behind = events.iter().take_while(|event| event.at <= at).count();
    let surplus = behind.saturating_sub(TIMELINE_PAST);

    events.drain(..surplus);
    events.truncate(behind - surplus + TIMELINE_LENGTH);
    events
}

fn capitalized(word: &str) -> String {
    let mut characters = word.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

fn thousands(value: i64) -> String {
    let digits = value.abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);

    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }

    if value < 0 { format!("-{out}") } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn utc(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap()
    }

    #[test]
    fn a_whole_sky_comes_back_populated() {
        let sky = now(utc(2026, 8, 8));

        assert_eq!(sky.planets.len(), 7);
        assert_eq!(sky.showers.len(), SHOWERS_AHEAD);
        assert_eq!(sky.eclipses.len(), 2);
        assert!(!sky.events.is_empty());
        assert!((0.0..=1.0).contains(&sky.moon.illumination));
    }

    #[test]
    fn the_timeline_is_ordered_and_bounded_at_both_ends() {
        let at = utc(2026, 8, 8);
        let sky = now(at);

        for pair in sky.events.windows(2) {
            assert!(pair[0].at <= pair[1].at, "{:?}", sky.events);
        }

        let behind = sky.events.iter().filter(|event| event.at <= at).count();
        assert!(behind <= TIMELINE_PAST, "{behind} events already gone by");
        assert!(
            sky.events
                .iter()
                .all(|event| event.at > at - TimeDelta::days(TIMELINE_LOOKBACK_DAYS))
        );
        assert!(sky.events.len() <= TIMELINE_PAST + TIMELINE_LENGTH);
    }

    #[test]
    fn an_event_that_has_just_happened_is_still_on_the_timeline() {
        // The Perseids peak on the 12th of August 2026.
        let sky = now(utc(2026, 8, 13));

        let peak = sky
            .events
            .iter()
            .find(|event| event.title.starts_with("Perseids"))
            .expect("a peak one day back is still worth showing");

        assert!(peak.at < sky.at, "{peak:?} has not happened yet");
    }

    #[test]
    fn an_event_falls_off_once_the_lookback_runs_out() {
        let sky = now(utc(2026, 8, 12) + TimeDelta::days(TIMELINE_LOOKBACK_DAYS + 1));

        assert!(
            !sky.events
                .iter()
                .any(|event| event.title.starts_with("Perseids")),
            "{:#?}",
            sky.events
        );
    }

    #[test]
    fn the_timeline_never_reaches_past_the_horizon() {
        let at = utc(2026, 8, 8);
        let sky = now(at);

        let horizon = at + TimeDelta::days(TIMELINE_HORIZON_DAYS);
        assert!(sky.events.iter().all(|event| event.at <= horizon));
    }

    #[test]
    fn the_timeline_mixes_more_than_one_kind_of_event() {
        let sky = now(utc(2026, 8, 8));

        let mut kinds: Vec<EventKind> = sky.events.iter().map(|event| event.kind).collect();
        kinds.sort_by_key(|kind| format!("{kind:?}"));
        kinds.dedup();

        assert!(
            kinds.len() >= 3,
            "a timeline of only {kinds:?} is not a timeline"
        );
    }

    #[test]
    fn the_season_running_now_opened_before_the_next_one_starts() {
        for month in 1..=12 {
            let sky = now(utc(2026, month, 15));
            assert!(sky.season.at <= sky.at);
            assert!(sky.next_turning.at > sky.at);
            assert!(sky.season.at < sky.next_turning.at);
        }
    }

    #[test]
    fn it_holds_up_across_a_whole_year_of_instants() {
        let start = utc(2026, 1, 1);

        for day in 0..365 {
            let at = start + TimeDelta::days(day);
            let sky = now(at);

            let lookback = at - TimeDelta::days(TIMELINE_LOOKBACK_DAYS);
            assert!(sky.events.iter().all(|event| event.at > lookback), "{at}");
            assert!(sky.showers.iter().all(|peak| peak.peak > at), "{at}");
            assert!(sky.eclipses.iter().all(|found| found.at > at), "{at}");
            assert!(sky.moon.next_quarters.iter().all(|q| q.at > at), "{at}");
        }
    }

    #[test]
    fn every_detail_line_starts_with_a_capital() {
        let sky = now(utc(2026, 8, 8));

        for event in &sky.events {
            let Some(detail) = &event.detail else {
                continue;
            };
            let first = detail.chars().next().expect("a detail is never empty");
            assert!(
                first.is_uppercase() || first.is_numeric(),
                "{:?} opens with {first:?}",
                event.detail
            );
        }
    }

    #[test]
    fn thousands_groups_the_way_a_reader_expects() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(384_400), "384,400");
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(thousands(-1_000), "-1,000");
    }

    #[test]
    fn a_full_moon_in_the_timeline_carries_its_distance() {
        let sky = now(utc(2026, 8, 8));

        let full = sky
            .events
            .iter()
            .find(|event| event.kind == EventKind::Moon && event.title == "Full moon");

        if let Some(event) = full {
            let detail = event.detail.as_deref().unwrap_or_default();
            assert!(detail.contains("km away"), "{detail}");
        }
    }

    /// Not that a line is titled "Full moon", but that the night the next one falls on is on the
    /// list at all. An eclipse takes the title on the nights it lands on, and the phase is named in
    /// its detail rather than pushed a lunation out to a date that is not the next one.
    #[test]
    fn the_next_full_moon_and_new_moon_are_on_the_timeline_at_their_own_dates() {
        let start = utc(2026, 1, 1);

        for day in 0..400 {
            let at = start + TimeDelta::days(day);
            let sky = now(at);
            let since = at - TimeDelta::days(TIMELINE_LOOKBACK_DAYS);

            for quarter in [Quarter::New, Quarter::Full] {
                let due = moon::next_quarter(since, quarter);

                let found = sky.events.iter().find(|event| {
                    matches!(event.kind, EventKind::Moon | EventKind::Eclipse)
                        && (event.at - due).abs() < TimeDelta::hours(1)
                });

                let event = found.unwrap_or_else(|| {
                    panic!(
                        "nothing at {due} for the {quarter:?} moon at {at}: {:#?}",
                        sky.events
                    )
                });

                let named = event.kind == EventKind::Moon
                    || event.detail.as_deref().is_some_and(|detail| {
                        detail.contains(quarter.label().to_lowercase().as_str())
                    });
                assert!(named, "{event:?} never says which phase it is");
            }
        }
    }

    #[test]
    fn an_eclipse_replaces_the_moon_phase_it_falls_on() {
        let sky = now(utc(2026, 8, 8));

        let eclipse = sky
            .events
            .iter()
            .find(|event| event.kind == EventKind::Eclipse)
            .expect("the August eclipse is inside the horizon");

        let clash = sky.events.iter().any(|event| {
            event.kind == EventKind::Moon && (event.at - eclipse.at).abs() < TimeDelta::hours(1)
        });
        assert!(!clash, "{:#?}", sky.events);
    }

    #[test]
    fn the_whole_thing_serializes() {
        let sky = now(utc(2026, 8, 8));
        let json = serde_json::to_string(&sky).expect("SkyNow is serializable");

        assert!(json.contains("\"moon\""));
        assert!(json.contains("\"planets\""));
        assert!(json.contains("\"events\""));
    }
}
