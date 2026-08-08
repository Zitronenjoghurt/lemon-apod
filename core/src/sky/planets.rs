use super::time::{J2000, angle_difference, cos_deg, normalize_degrees, sin_deg, to_julian};
use chrono::{DateTime, TimeDelta, Utc};
use serde::Serialize;

const GLARE_DEGREES: f64 = 10.0;

const KEPLER_ITERATIONS: usize = 12;
const KEPLER_TOLERANCE: f64 = 1e-9;

const SEARCH_DAYS: i64 = 820;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Planet {
    Mercury,
    Venus,
    Mars,
    Jupiter,
    Saturn,
    Uranus,
    Neptune,
}

impl Planet {
    pub const ALL: [Self; 7] = [
        Self::Mercury,
        Self::Venus,
        Self::Mars,
        Self::Jupiter,
        Self::Saturn,
        Self::Uranus,
        Self::Neptune,
    ];

    pub const NAKED_EYE: [Self; 5] = [
        Self::Mercury,
        Self::Venus,
        Self::Mars,
        Self::Jupiter,
        Self::Saturn,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Mercury => "Mercury",
            Self::Venus => "Venus",
            Self::Mars => "Mars",
            Self::Jupiter => "Jupiter",
            Self::Saturn => "Saturn",
            Self::Uranus => "Uranus",
            Self::Neptune => "Neptune",
        }
    }

    pub const fn is_inferior(self) -> bool {
        matches!(self, Self::Mercury | Self::Venus)
    }

    pub const fn naked_eye(self) -> bool {
        matches!(
            self,
            Self::Mercury | Self::Venus | Self::Mars | Self::Jupiter | Self::Saturn
        )
    }

    const fn elements(self) -> Elements {
        match self {
            Self::Mercury => Elements {
                a: (0.387_099_27, 0.000_000_37),
                e: (0.205_635_93, 0.000_019_06),
                inclination: (7.004_979_02, -0.005_947_49),
                mean_longitude: (252.250_323_50, 149_472.674_111_75),
                perihelion: (77.457_796_28, 0.160_476_89),
                node: (48.330_765_93, -0.125_340_81),
            },
            Self::Venus => Elements {
                a: (0.723_335_66, 0.000_003_90),
                e: (0.006_776_72, -0.000_041_07),
                inclination: (3.394_676_05, -0.000_788_90),
                mean_longitude: (181.979_099_50, 58_517.815_387_29),
                perihelion: (131.602_467_18, 0.002_683_29),
                node: (76.679_842_55, -0.277_694_18),
            },
            Self::Mars => Elements {
                a: (1.523_710_34, 0.000_018_47),
                e: (0.093_394_10, 0.000_078_82),
                inclination: (1.849_691_42, -0.008_131_31),
                mean_longitude: (-4.553_432_05, 19_140.302_684_99),
                perihelion: (-23.943_629_59, 0.444_410_88),
                node: (49.559_538_91, -0.292_573_43),
            },
            Self::Jupiter => Elements {
                a: (5.202_887_00, -0.000_116_07),
                e: (0.048_386_24, -0.000_132_53),
                inclination: (1.304_396_95, -0.001_837_14),
                mean_longitude: (34.396_440_51, 3_034.746_127_75),
                perihelion: (14.728_479_83, 0.212_526_68),
                node: (100.473_909_09, 0.204_691_06),
            },
            Self::Saturn => Elements {
                a: (9.536_675_94, -0.001_250_60),
                e: (0.053_861_79, -0.000_509_91),
                inclination: (2.485_991_87, 0.001_936_09),
                mean_longitude: (49.954_244_23, 1_222.493_622_01),
                perihelion: (92.598_878_31, -0.418_972_16),
                node: (113.662_424_48, -0.288_677_94),
            },
            Self::Uranus => Elements {
                a: (19.189_164_64, -0.001_961_76),
                e: (0.047_257_44, -0.000_043_97),
                inclination: (0.772_637_83, -0.002_429_39),
                mean_longitude: (313.238_104_51, 428.482_027_85),
                perihelion: (170.954_276_30, 0.408_052_81),
                node: (74.016_925_03, 0.042_405_89),
            },
            Self::Neptune => Elements {
                a: (30.069_922_76, 0.000_262_91),
                e: (0.008_590_48, 0.000_051_05),
                inclination: (1.770_043_47, 0.000_353_72),
                mean_longitude: (-55.120_029_69, 218.459_453_25),
                perihelion: (44.964_762_27, -0.322_414_64),
                node: (131.784_225_74, -0.005_086_64),
            },
        }
    }

    const fn brightness(self) -> (f64, f64, f64, f64) {
        match self {
            Self::Mercury => (-0.42, 0.038_0, -0.000_273, 0.000_002),
            Self::Venus => (-4.40, 0.000_9, 0.000_239, -0.000_000_65),
            Self::Mars => (-1.52, 0.016_0, 0.0, 0.0),
            Self::Jupiter => (-9.40, 0.005_0, 0.0, 0.0),
            Self::Saturn => (-8.88, 0.0, 0.0, 0.0),
            Self::Uranus => (-7.19, 0.0, 0.0, 0.0),
            Self::Neptune => (-6.87, 0.0, 0.0, 0.0),
        }
    }
}

struct Elements {
    a: (f64, f64),
    e: (f64, f64),
    inclination: (f64, f64),
    mean_longitude: (f64, f64),
    perihelion: (f64, f64),
    node: (f64, f64),
}

const EARTH: Elements = Elements {
    a: (1.000_002_61, 0.000_005_62),
    e: (0.016_711_23, -0.000_043_92),
    inclination: (-0.000_015_31, -0.012_946_68),
    mean_longitude: (100.464_571_66, 35_999.372_449_81),
    perihelion: (102.937_681_93, 0.323_273_64),
    node: (0.0, 0.0),
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Evening,
    Morning,
    Lost,
}

impl Visibility {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Evening => "Evening sky",
            Self::Morning => "Morning sky",
            Self::Lost => "Too close to the sun",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Milestone {
    Opposition,
    GreatestEasternElongation,
    GreatestWesternElongation,
}

impl Milestone {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Opposition => "at opposition",
            Self::GreatestEasternElongation => "at greatest eastern elongation",
            Self::GreatestWesternElongation => "at greatest western elongation",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanetEvent {
    pub planet: Planet,
    pub name: &'static str,
    pub milestone: Milestone,
    pub label: &'static str,
    pub at: DateTime<Utc>,
    pub elongation: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanetNow {
    pub planet: Planet,
    pub name: &'static str,
    pub naked_eye: bool,
    pub visibility: Visibility,
    pub visibility_label: &'static str,
    pub elongation: f64,
    pub magnitude: f64,
    pub distance_au: f64,
    pub next_milestone: Option<PlanetEvent>,
}

pub fn now(at: DateTime<Utc>) -> Vec<PlanetNow> {
    let mut planets: Vec<PlanetNow> = Planet::ALL.iter().map(|&planet| one(planet, at)).collect();

    planets.sort_by(|left, right| {
        let lost =
            (left.visibility == Visibility::Lost).cmp(&(right.visibility == Visibility::Lost));
        lost.then_with(|| {
            left.magnitude
                .partial_cmp(&right.magnitude)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    planets
}

pub fn one(planet: Planet, at: DateTime<Utc>) -> PlanetNow {
    let view = observe(planet, at);
    let next = next_milestone(planet, at);

    PlanetNow {
        planet,
        name: planet.name(),
        naked_eye: planet.naked_eye(),
        visibility: view.visibility,
        visibility_label: view.visibility.label(),
        elongation: view.elongation,
        magnitude: view.magnitude,
        distance_au: view.distance,
        next_milestone: next,
    }
}

struct View {
    elongation: f64,
    /// Difference in ecliptic longitude from the sun, positive when the planet is east of it
    /// and negative when west. Runs from -180 to 180.
    offset: f64,
    visibility: Visibility,
    magnitude: f64,
    distance: f64,
}

impl View {
    /// How far the planet is from the point opposite the sun, in longitude. Zero exactly at
    /// opposition, negative approaching it, positive after, and plus or minus 180 at conjunction.
    fn opposition_gap(&self) -> f64 {
        angle_difference(180.0, self.offset)
    }
}

fn observe(planet: Planet, at: DateTime<Utc>) -> View {
    let t = (to_julian(at) - J2000) / super::time::DAYS_PER_CENTURY;

    let body = heliocentric(&planet.elements(), t);
    let earth = heliocentric(&EARTH, t);

    let geocentric = [body[0] - earth[0], body[1] - earth[1], body[2] - earth[2]];

    let r = length(body);
    let big_r = length(earth);
    let delta = length(geocentric).max(1e-9);

    let elongation = ((big_r.powi(2) + delta.powi(2) - r.powi(2)) / (2.0 * big_r * delta))
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees();
    let phase_angle = ((r.powi(2) + delta.powi(2) - big_r.powi(2)) / (2.0 * r * delta))
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees();

    let sun_longitude = normalize_degrees((-earth[1]).atan2(-earth[0]).to_degrees());
    let planet_longitude = normalize_degrees(geocentric[1].atan2(geocentric[0]).to_degrees());
    let offset = angle_difference(sun_longitude, planet_longitude);

    let visibility = if elongation < GLARE_DEGREES {
        Visibility::Lost
    } else if offset > 0.0 {
        Visibility::Evening
    } else {
        Visibility::Morning
    };

    View {
        elongation,
        offset,
        visibility,
        magnitude: magnitude(planet, r, delta, phase_angle),
        distance: delta,
    }
}

fn magnitude(planet: Planet, r: f64, delta: f64, phase_angle: f64) -> f64 {
    let (base, first, second, third) = planet.brightness();
    let i = phase_angle;

    base + 5.0 * (r * delta).max(1e-12).log10() + first * i + second * i.powi(2) + third * i.powi(3)
}

fn heliocentric(elements: &Elements, t: f64) -> [f64; 3] {
    let a = elements.a.0 + elements.a.1 * t;
    let e = elements.e.0 + elements.e.1 * t;
    let inclination = elements.inclination.0 + elements.inclination.1 * t;
    let mean_longitude = elements.mean_longitude.0 + elements.mean_longitude.1 * t;
    let perihelion = elements.perihelion.0 + elements.perihelion.1 * t;
    let node = elements.node.0 + elements.node.1 * t;

    let argument = perihelion - node;
    let mean_anomaly = wrap_signed(mean_longitude - perihelion);

    let eccentric = solve_kepler(mean_anomaly, e);

    let x_orbit = a * (cos_deg(eccentric) - e);
    let y_orbit = a * (1.0 - e * e).max(0.0).sqrt() * sin_deg(eccentric);

    let (cos_w, sin_w) = (cos_deg(argument), sin_deg(argument));
    let (cos_o, sin_o) = (cos_deg(node), sin_deg(node));
    let (cos_i, sin_i) = (cos_deg(inclination), sin_deg(inclination));

    [
        (cos_w * cos_o - sin_w * sin_o * cos_i) * x_orbit
            + (-sin_w * cos_o - cos_w * sin_o * cos_i) * y_orbit,
        (cos_w * sin_o + sin_w * cos_o * cos_i) * x_orbit
            + (-sin_w * sin_o + cos_w * cos_o * cos_i) * y_orbit,
        (sin_w * sin_i) * x_orbit + (cos_w * sin_i) * y_orbit,
    ]
}

fn solve_kepler(mean_anomaly: f64, e: f64) -> f64 {
    let e_degrees = e.to_degrees();
    let mut eccentric = mean_anomaly + e_degrees * sin_deg(mean_anomaly);

    for _ in 0..KEPLER_ITERATIONS {
        let residual = mean_anomaly - (eccentric - e_degrees * sin_deg(eccentric));
        let step = residual / (1.0 - e * cos_deg(eccentric));
        eccentric += step;

        if step.abs() < KEPLER_TOLERANCE {
            break;
        }
    }

    eccentric
}

fn wrap_signed(degrees: f64) -> f64 {
    let normalized = normalize_degrees(degrees);
    if normalized > 180.0 {
        normalized - 360.0
    } else {
        normalized
    }
}

fn length(vector: [f64; 3]) -> f64 {
    (vector[0].powi(2) + vector[1].powi(2) + vector[2].powi(2)).sqrt()
}

pub fn next_milestone(planet: Planet, from: DateTime<Utc>) -> Option<PlanetEvent> {
    let at = if planet.is_inferior() {
        greatest_elongation(planet, from)?
    } else {
        opposition(planet, from)?
    };

    let view = observe(planet, at);

    let milestone = if !planet.is_inferior() {
        Milestone::Opposition
    } else if view.offset > 0.0 {
        Milestone::GreatestEasternElongation
    } else {
        Milestone::GreatestWesternElongation
    };

    Some(PlanetEvent {
        planet,
        name: planet.name(),
        milestone,
        label: milestone.label(),
        at,
        elongation: view.elongation,
    })
}

fn days_from(from: DateTime<Utc>, offset: f64) -> DateTime<Utc> {
    from + TimeDelta::milliseconds((offset * 86_400_000.0) as i64)
}

fn opposition(planet: Planet, from: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let gap = |offset: f64| observe(planet, days_from(from, offset)).opposition_gap();

    let mut previous = gap(0.0);

    for day in 1..=SEARCH_DAYS {
        let current = gap(day as f64);

        if previous > 0.0 && current <= 0.0 && previous < 90.0 && current > -90.0 {
            let crossing = bisect(&gap, (day - 1) as f64, day as f64);
            return Some(days_from(from, crossing));
        }

        previous = current;
    }

    None
}

fn greatest_elongation(planet: Planet, from: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let elongation_at = |offset: f64| observe(planet, days_from(from, offset)).elongation;

    let mut previous = elongation_at(0.0);
    let mut current = elongation_at(1.0);

    for day in 1..SEARCH_DAYS {
        let next = elongation_at((day + 1) as f64);

        if current > previous && current >= next {
            let peak = refine_peak(&elongation_at, (day - 1) as f64, (day + 1) as f64);
            return Some(days_from(from, peak));
        }

        previous = current;
        current = next;
    }

    None
}

fn refine_peak(elongation_at: &dyn Fn(f64) -> f64, mut low: f64, mut high: f64) -> f64 {
    // Forty rounds narrow two days to well under a second.
    for _ in 0..40 {
        let third = (high - low) / 3.0;
        let left = low + third;
        let right = high - third;

        if elongation_at(left) < elongation_at(right) {
            low = left;
        } else {
            high = right;
        }
    }

    (low + high) / 2.0
}

fn bisect(gap: &dyn Fn(f64) -> f64, mut low: f64, mut high: f64) -> f64 {
    for _ in 0..40 {
        let middle = (low + high) / 2.0;
        if gap(middle) > 0.0 {
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
    use chrono::TimeZone;

    fn utc(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap()
    }

    #[test]
    fn earth_sits_one_astronomical_unit_from_the_sun() {
        for year in [2020, 2026, 2030] {
            let t = (to_julian(utc(year, 6, 1)) - J2000) / super::super::time::DAYS_PER_CENTURY;
            let distance = length(heliocentric(&EARTH, t));
            assert!(
                (0.98..1.02).contains(&distance),
                "Earth was {distance} AU from the sun in {year}"
            );
        }
    }

    #[test]
    fn every_planet_orbits_at_about_its_known_distance() {
        for (planet, expected) in [
            (Planet::Mercury, 0.387),
            (Planet::Venus, 0.723),
            (Planet::Mars, 1.524),
            (Planet::Jupiter, 5.203),
            (Planet::Saturn, 9.537),
            (Planet::Uranus, 19.19),
            (Planet::Neptune, 30.07),
        ] {
            let t = (to_julian(utc(2026, 1, 1)) - J2000) / super::super::time::DAYS_PER_CENTURY;
            let distance = length(heliocentric(&planet.elements(), t));

            let tolerance = expected * 0.25;
            assert!(
                (distance - expected).abs() < tolerance,
                "{} was {distance} AU out, expected about {expected}",
                planet.name()
            );
        }
    }

    #[test]
    fn kepler_solves_a_circle_and_a_very_eccentric_orbit() {
        assert!((solve_kepler(75.0, 0.0) - 75.0).abs() < 1e-9);
        assert!((solve_kepler(5.0, 0.1) - 5.554_589).abs() < 1e-5);
    }

    #[test]
    fn the_inferior_planets_never_stray_far_from_the_sun() {
        let mut mercury: f64 = 0.0;
        let mut venus: f64 = 0.0;

        for day in 0..(365 * 3) {
            let at = utc(2026, 1, 1) + TimeDelta::days(day);
            mercury = mercury.max(observe(Planet::Mercury, at).elongation);
            venus = venus.max(observe(Planet::Venus, at).elongation);
        }

        assert!((17.0..29.5).contains(&mercury), "Mercury reached {mercury}");
        assert!((44.0..48.5).contains(&venus), "Venus reached {venus}");
    }

    #[test]
    fn the_superior_planets_all_come_round_to_opposition() {
        for planet in [
            Planet::Mars,
            Planet::Jupiter,
            Planet::Saturn,
            Planet::Uranus,
            Planet::Neptune,
        ] {
            let mut best: f64 = 0.0;
            for day in 0..(365 * 3) {
                let at = utc(2026, 1, 1) + TimeDelta::days(day);
                best = best.max(observe(planet, at).elongation);
            }
            assert!(
                best > 174.0,
                "{} only reached {best} degrees of elongation",
                planet.name()
            );
        }
    }

    #[test]
    fn oppositions_land_on_the_published_dates() {
        for (planet, expected) in [
            (Planet::Neptune, utc(2026, 9, 26)),
            (Planet::Saturn, utc(2026, 10, 4)),
            (Planet::Uranus, utc(2026, 11, 25)),
            (Planet::Jupiter, utc(2027, 2, 11)),
            (Planet::Mars, utc(2027, 2, 19)),
        ] {
            let found = next_milestone(planet, utc(2026, 8, 1)).expect("an opposition is due");
            assert_eq!(found.milestone, Milestone::Opposition);

            let days = (found.at - expected).as_seconds_f64().abs() / 86_400.0;
            assert!(
                days < 1.5,
                "{}: got {}, expected about {expected}",
                planet.name(),
                found.at
            );
        }
    }

    #[test]
    fn an_opposition_really_is_opposite_the_sun_in_longitude() {
        for planet in [Planet::Jupiter, Planet::Saturn, Planet::Mars] {
            let found = next_milestone(planet, utc(2026, 8, 1)).unwrap();
            let view = observe(planet, found.at);

            assert!(
                view.offset.abs() > 179.9,
                "{} was {} degrees from the sun in longitude at opposition",
                planet.name(),
                view.offset
            );
            assert!(
                found.elongation > 174.0,
                "{}: elongation at opposition was {}",
                planet.name(),
                found.elongation
            );
        }
    }

    #[test]
    fn the_search_never_mistakes_conjunction_for_opposition() {
        let opposition = next_milestone(Planet::Mars, utc(2026, 8, 1)).unwrap().at;

        for offset in [0, 30, 90, 150, 180] {
            let found = next_milestone(Planet::Mars, utc(2026, 8, 1) + TimeDelta::days(offset))
                .expect("an opposition is due");

            assert_eq!(
                found.at, opposition,
                "starting {offset} days later moved it"
            );
        }

        let after = next_milestone(Planet::Mars, opposition + TimeDelta::days(1)).unwrap();
        let gap = (after.at - opposition).as_seconds_f64() / 86_400.0;
        assert!(
            (760.0..815.0).contains(&gap),
            "the next one was {gap} days out"
        );
    }

    #[test]
    fn oppositions_repeat_at_the_synodic_period() {
        for (planet, synodic) in [(Planet::Jupiter, 398.9), (Planet::Saturn, 378.1)] {
            let mut at = utc(2026, 1, 1);
            let mut found = Vec::new();

            for _ in 0..3 {
                let next = next_milestone(planet, at).unwrap();
                found.push(next.at);
                at = next.at + TimeDelta::days(1);
            }

            for pair in found.windows(2) {
                let gap = (pair[1] - pair[0]).as_seconds_f64() / 86_400.0;
                assert!(
                    (gap - synodic).abs() < 6.0,
                    "{}: oppositions {gap} days apart, expected about {synodic}",
                    planet.name()
                );
            }
        }
    }

    #[test]
    fn greatest_elongations_reach_the_known_maxima() {
        for (planet, range) in [(Planet::Mercury, 17.5..28.5), (Planet::Venus, 45.0..47.5)] {
            let mut at = utc(2026, 1, 1);

            for _ in 0..4 {
                let found = next_milestone(planet, at).expect("an elongation is due");
                assert!(
                    range.contains(&found.elongation),
                    "{} reached {} degrees",
                    planet.name(),
                    found.elongation
                );
                at = found.at + TimeDelta::days(5);
            }
        }
    }

    #[test]
    fn mercurys_elongations_alternate_east_and_west() {
        let mut at = utc(2026, 1, 1);
        let mut previous: Option<PlanetEvent> = None;

        for _ in 0..6 {
            let found = next_milestone(Planet::Mercury, at).expect("an elongation is due");

            if let Some(last) = &previous {
                assert_ne!(
                    found.milestone, last.milestone,
                    "two {:?} in a row, at {} and {}",
                    found.milestone, last.at, found.at
                );

                let gap = (found.at - last.at).as_seconds_f64() / 86_400.0;
                assert!((35.0..85.0).contains(&gap), "elongations {gap} days apart");
            }

            at = found.at + TimeDelta::days(5);
            previous = Some(found);
        }
    }

    #[test]
    fn a_greatest_elongation_really_is_the_widest_the_gap_gets() {
        for planet in [Planet::Mercury, Planet::Venus] {
            let found = next_milestone(planet, utc(2026, 1, 1)).unwrap();

            for days in [-3, -1, 1, 3] {
                let nearby = observe(planet, found.at + TimeDelta::days(days)).elongation;
                assert!(
                    nearby < found.elongation,
                    "{}: {days} days off was wider, {nearby} against {}",
                    planet.name(),
                    found.elongation
                );
            }
        }
    }

    #[test]
    fn mercury_and_venus_get_elongations_rather_than_oppositions() {
        for planet in [Planet::Mercury, Planet::Venus] {
            let found = next_milestone(planet, utc(2026, 8, 1)).expect("an elongation is due");
            assert!(
                matches!(
                    found.milestone,
                    Milestone::GreatestEasternElongation | Milestone::GreatestWesternElongation
                ),
                "{} got {:?}",
                planet.name(),
                found.milestone
            );
        }
    }

    #[test]
    fn an_eastern_elongation_is_an_evening_star() {
        let found = next_milestone(Planet::Venus, utc(2026, 8, 1)).unwrap();
        let view = observe(Planet::Venus, found.at);

        let expected = if found.milestone == Milestone::GreatestEasternElongation {
            Visibility::Evening
        } else {
            Visibility::Morning
        };
        assert_eq!(view.visibility, expected);
    }

    #[test]
    fn every_milestone_found_is_in_the_future() {
        let from = utc(2026, 8, 8);
        for planet in Planet::ALL {
            let found = next_milestone(planet, from).expect("all seven come round");
            assert!(found.at > from, "{}: {}", planet.name(), found.at);

            let days = (found.at - from).as_seconds_f64() / 86_400.0;
            assert!(days < SEARCH_DAYS as f64, "{}: {days} days", planet.name());
        }
    }

    #[test]
    fn the_magnitudes_are_in_the_right_range() {
        let planets = now(utc(2026, 8, 8));

        for planet in &planets {
            let plausible = match planet.planet {
                Planet::Venus => (-5.0, -3.0),
                Planet::Jupiter => (-3.0, -1.5),
                Planet::Mars => (-3.0, 2.0),
                Planet::Saturn => (-0.5, 1.5),
                Planet::Mercury => (-2.5, 4.0),
                Planet::Uranus => (5.0, 6.5),
                Planet::Neptune => (7.5, 8.5),
            };
            assert!(
                (plausible.0..plausible.1).contains(&planet.magnitude),
                "{} came out at magnitude {}",
                planet.name,
                planet.magnitude
            );
        }
    }

    #[test]
    fn everything_lost_in_the_glare_sorts_last() {
        let planets = now(utc(2026, 8, 8));
        assert_eq!(planets.len(), 7);

        let first_lost = planets
            .iter()
            .position(|planet| planet.visibility == Visibility::Lost);

        if let Some(index) = first_lost {
            assert!(
                planets[index..]
                    .iter()
                    .all(|planet| planet.visibility == Visibility::Lost),
                "a visible planet sorted below a hidden one"
            );
        }
    }

    #[test]
    fn a_planet_at_opposition_is_never_lost_in_the_glare() {
        let found = next_milestone(Planet::Saturn, utc(2026, 1, 1)).unwrap();
        let view = observe(Planet::Saturn, found.at);
        assert_ne!(view.visibility, Visibility::Lost);
    }
}
