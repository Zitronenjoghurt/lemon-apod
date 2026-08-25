use crate::client::{Client, Limit, Response};
use crate::config::Sky;
use crate::shutdown::Shutdown;
use anyhow::{Context, Result};
use apod_core::sky::store::{self, Launch, SkyWriter};
use apod_core::sky::weather::g_level;
use chrono::{DateTime, TimeDelta, Utc};
use serde::Deserialize;

const BODY_LIMIT: Limit = Limit {
    max_bytes: 4 * 1_048_576,
    timeout: std::time::Duration::from_secs(30),
};

pub async fn run(cfg: crate::config::Config, client: Client, mut shutdown: Shutdown) -> Result<()> {
    let sky = SkyWriter::open(&cfg.sky_db)
        .await
        .with_context(|| format!("opening {}", cfg.sky_db.display()))?;

    loop {
        poll_once(&cfg, &client, &sky, shutdown.is_triggered()).await;

        if !shutdown.sleep(cfg.sky.interval).await {
            break;
        }
    }

    sky.close().await;
    Ok(())
}

pub async fn poll(cfg: &crate::config::Config) -> Result<()> {
    let client = Client::new(&cfg.user_agent, cfg.fetch_timeout, cfg.fetch_max_retries)?;
    let sky = SkyWriter::open(&cfg.sky_db)
        .await
        .with_context(|| format!("opening {}", cfg.sky_db.display()))?;

    poll_once(cfg, &client, &sky, false).await;

    let reader = sky.reader();
    let launches = reader.upcoming_launches(Utc::now(), 5).await?;

    println!("next {} launches:", launches.len());
    for launch in &launches {
        println!(
            "  {}  {}{}",
            launch.net.format("%Y-%m-%d %H:%M UTC"),
            launch.name,
            launch
                .provider
                .as_deref()
                .map(|provider| format!("  ({provider})"))
                .unwrap_or_default()
        );
    }

    match reader.weather_report().await? {
        Some(report) => println!(
            "space weather: Kp {:.2} ({}), measured {}",
            report.kp,
            match g_level(report.kp) {
                Some(level) => format!("G{level}"),
                None => "below G1".to_owned(),
            },
            report.observed_at.format("%Y-%m-%d %H:%M UTC")
        ),
        None => println!("space weather: nothing recorded"),
    }

    for feed in reader.feeds().await? {
        match feed.error {
            Some(error) => println!("feed {}: failed, {error}", feed.name),
            None => println!("feed {}: ok", feed.name),
        }
    }

    sky.close().await;
    Ok(())
}

async fn poll_once(cfg: &crate::config::Config, client: &Client, sky: &SkyWriter, stopping: bool) {
    if cfg.sky.launches_enabled {
        report(
            sky,
            store::LAUNCHES,
            poll_launches(&cfg.sky, client, sky).await,
        )
        .await;
    }

    if !stopping && cfg.sky.weather_enabled {
        report(
            sky,
            store::SPACE_WEATHER,
            poll_space_weather(&cfg.sky, client, sky).await,
        )
        .await;
    }
}

async fn report(sky: &SkyWriter, feed: &str, outcome: Result<usize>) {
    let error = match outcome {
        Ok(count) => {
            tracing::info!(feed, count, "sky feed updated");
            None
        }
        Err(error) => {
            tracing::warn!(feed, "sky feed failed: {error:#}");
            Some(format!("{error:#}"))
        }
    };

    if let Err(error) = sky.record_feed(feed, error.as_deref()).await {
        tracing::warn!(feed, "could not record the feed state: {error:#}");
    }
}

async fn poll_launches(cfg: &Sky, client: &Client, sky: &SkyWriter) -> Result<usize> {
    let url = format!(
        "{}?limit={}&mode=normal&hide_recent_previous=false",
        cfg.launches_url, cfg.launch_limit
    );

    let body = fetch(client, &url).await?;
    let page: LaunchPage = serde_json::from_slice(&body)
        .with_context(|| format!("parsing the launch feed from {url}"))?;

    let launches: Vec<Launch> = page
        .results
        .into_iter()
        .filter_map(|raw| convert(raw, &cfg.launch_page_url))
        .collect();
    let keep_from = Utc::now() - TimeDelta::hours(store::LAUNCH_LOOKBACK_HOURS);
    let written = sky.replace_launches(&launches, keep_from).await?;

    Ok(written as usize)
}

async fn poll_space_weather(cfg: &Sky, client: &Client, sky: &SkyWriter) -> Result<usize> {
    let report = crate::weather::report(cfg, client).await?;
    sky.set_weather_report(&report).await?;

    Ok(1)
}

async fn fetch(client: &Client, url: &str) -> Result<Vec<u8>> {
    match client.get_limited(url, BODY_LIMIT).await? {
        Response::Body(bytes) => Ok(bytes),
        Response::NotFound => anyhow::bail!("{url} returned 404"),
    }
}

#[derive(Debug, Deserialize)]
struct LaunchPage {
    results: Vec<RawLaunch>,
}

#[derive(Debug, Deserialize)]
struct RawLaunch {
    id: String,
    name: String,
    slug: Option<String>,
    net: Option<DateTime<Utc>>,
    window_start: Option<DateTime<Utc>>,
    window_end: Option<DateTime<Utc>>,
    status: Option<Named>,
    net_precision: Option<Named>,
    launch_service_provider: Option<Named>,
    rocket: Option<RawRocket>,
    mission: Option<RawMission>,
    pad: Option<RawPad>,
    image: Option<RawImage>,
}

#[derive(Debug, Deserialize)]
struct Named {
    name: Option<String>,
    abbrev: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawRocket {
    configuration: Option<RawConfiguration>,
}

#[derive(Debug, Deserialize)]
struct RawConfiguration {
    full_name: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawMission {
    description: Option<String>,
    orbit: Option<Named>,
}

#[derive(Debug, Deserialize)]
struct RawPad {
    name: Option<String>,
    location: Option<Named>,
}

#[derive(Debug, Deserialize)]
struct RawImage {
    thumbnail_url: Option<String>,
    image_url: Option<String>,
}

fn convert(raw: RawLaunch, page_template: &str) -> Option<Launch> {
    let net = raw.net?;

    let pad = match raw.pad {
        Some(pad) => match (pad.name, pad.location.and_then(|place| place.name)) {
            (Some(name), Some(place)) => Some(format!("{name}, {place}")),
            (name, place) => name.or(place),
        },
        None => None,
    };

    Some(Launch {
        id: raw.id,
        name: raw.name,
        provider: raw.launch_service_provider.and_then(|agency| agency.name),
        vehicle: raw.rocket.and_then(|rocket| {
            rocket
                .configuration
                .and_then(|configuration| configuration.full_name.or(configuration.name))
        }),
        pad,
        mission: raw
            .mission
            .as_ref()
            .and_then(|mission| mission.description.clone())
            .filter(|description| !description.trim().is_empty()),
        orbit: raw
            .mission
            .and_then(|mission| mission.orbit)
            .and_then(|orbit| orbit.name),
        status: raw.status.and_then(|status| status.name),
        net,
        window_start: raw.window_start,
        window_end: raw.window_end,
        precision: raw
            .net_precision
            .and_then(|precision| precision.abbrev.or(precision.name)),
        image_url: raw
            .image
            .and_then(|image| image.thumbnail_url.or(image.image_url)),
        info_url: launch_page(raw.slug.as_deref(), page_template),
    })
}

fn launch_page(slug: Option<&str>, template: &str) -> Option<String> {
    let slug = slug.map(str::trim).filter(|slug| !slug.is_empty())?;

    if !slug
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }

    Some(template.replace("{slug}", slug))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAUNCH_PAGE: &str = r#"{
        "count": 363,
        "results": [{
            "id": "e4316d4c-9eb8-4a18-9b0d-4307c3970ef6",
            "url": "https://ll.thespacedevs.com/2.3.0/launches/e4316d4c/",
            "name": "Falcon 9 Block 5 | Starlink Group 17-38",
            "slug": "falcon-9-block-5-starlink-group-17-38",
            "status": { "id": 1, "name": "Go for Launch", "abbrev": "Go" },
            "net": "2026-08-08T16:23:41Z",
            "net_precision": { "id": 0, "name": "Second", "abbrev": "SEC" },
            "window_start": "2026-08-08T14:00:00Z",
            "window_end": "2026-08-08T18:00:00Z",
            "image": { "image_url": "https://example.test/big.png",
                       "thumbnail_url": "https://example.test/small.png" },
            "launch_service_provider": { "id": 121, "name": "SpaceX", "abbrev": "SpX" },
            "rocket": { "id": 9097, "configuration": {
                "id": 164, "name": "Falcon 9", "full_name": "Falcon 9 Block 5" } },
            "mission": { "id": 1, "name": "Starlink Group 17-38",
                         "description": "A batch of satellites for the Starlink constellation.",
                         "orbit": { "id": 8, "name": "Low Earth Orbit", "abbrev": "LEO" } },
            "pad": { "id": 80, "name": "Space Launch Complex 4E",
                     "location": { "id": 11, "name": "Vandenberg SFB, CA, USA" } }
        }]
    }"#;

    const PAGE_TEMPLATE: &str = "https://spacelaunchnow.me/launch/{slug}/";

    fn parse_one(json: &str) -> Option<Launch> {
        let page: LaunchPage = serde_json::from_str(json).expect("the page parses");
        page.results
            .into_iter()
            .next()
            .and_then(|raw| convert(raw, PAGE_TEMPLATE))
    }

    #[test]
    fn a_real_launch_payload_maps_across() {
        let launch = parse_one(LAUNCH_PAGE).expect("the launch has a time");

        assert_eq!(launch.name, "Falcon 9 Block 5 | Starlink Group 17-38");
        assert_eq!(launch.provider.as_deref(), Some("SpaceX"));
        assert_eq!(launch.vehicle.as_deref(), Some("Falcon 9 Block 5"));
        assert_eq!(
            launch.pad.as_deref(),
            Some("Space Launch Complex 4E, Vandenberg SFB, CA, USA")
        );
        assert_eq!(launch.orbit.as_deref(), Some("Low Earth Orbit"));
        assert_eq!(launch.status.as_deref(), Some("Go for Launch"));
        assert_eq!(launch.precision.as_deref(), Some("SEC"));
        assert!(launch.time_is_firm());
        assert_eq!(launch.net.to_rfc3339(), "2026-08-08T16:23:41+00:00");
        assert_eq!(
            launch.image_url.as_deref(),
            Some("https://example.test/small.png")
        );
        assert_eq!(
            launch.info_url.as_deref(),
            Some("https://spacelaunchnow.me/launch/falcon-9-block-5-starlink-group-17-38/")
        );
    }

    #[test]
    fn a_launch_with_no_slug_gets_no_link_rather_than_a_broken_one() {
        let unslugged = r#"{"results":[{
            "id": "abc", "name": "n", "net": "2026-09-01T00:00:00Z",
            "url": "https://ll.thespacedevs.com/2.3.0/launches/abc/"
        }]}"#;

        assert!(parse_one(unslugged).unwrap().info_url.is_none());
    }

    #[test]
    fn a_slug_that_could_climb_out_of_the_path_is_refused() {
        for hostile in [
            "../../admin",
            "a/b",
            "a?b=c",
            "a#b",
            "a b",
            "https://elsewhere.test",
            "",
            "   ",
        ] {
            assert_eq!(
                launch_page(Some(hostile), PAGE_TEMPLATE),
                None,
                "{hostile:?} was let through"
            );
        }

        assert_eq!(
            launch_page(Some("falcon-9_block-5"), PAGE_TEMPLATE).as_deref(),
            Some("https://spacelaunchnow.me/launch/falcon-9_block-5/")
        );
        assert_eq!(launch_page(None, PAGE_TEMPLATE), None);
    }

    #[test]
    fn a_launch_stripped_to_its_bones_still_parses() {
        let sparse = r#"{"results":[{
            "id": "abc", "name": "Some rocket | Some payload", "net": "2026-09-01T00:00:00Z"
        }]}"#;

        let launch = parse_one(sparse).expect("id, name and net are all it takes");
        assert_eq!(launch.id, "abc");
        assert!(launch.provider.is_none());
        assert!(launch.vehicle.is_none());
        assert!(launch.pad.is_none());
        assert!(!launch.time_is_firm());
    }

    #[test]
    fn a_launch_with_no_time_is_dropped() {
        let undated = r#"{"results":[{"id": "abc", "name": "Sometime, maybe"}]}"#;
        assert!(parse_one(undated).is_none());
    }

    #[test]
    fn a_pad_with_only_one_half_still_reads() {
        let only_place = r#"{"results":[{
            "id": "a", "name": "n", "net": "2026-09-01T00:00:00Z",
            "pad": { "location": { "name": "Baikonur, Kazakhstan" } }
        }]}"#;
        assert_eq!(
            parse_one(only_place).unwrap().pad.as_deref(),
            Some("Baikonur, Kazakhstan")
        );

        let only_pad = r#"{"results":[{
            "id": "a", "name": "n", "net": "2026-09-01T00:00:00Z",
            "pad": { "name": "LC-39A" }
        }]}"#;
        assert_eq!(parse_one(only_pad).unwrap().pad.as_deref(), Some("LC-39A"));
    }

    #[test]
    fn an_empty_mission_description_does_not_become_an_empty_line() {
        let blank = r#"{"results":[{
            "id": "a", "name": "n", "net": "2026-09-01T00:00:00Z",
            "mission": { "description": "   ", "orbit": { "name": "LEO" } }
        }]}"#;

        let launch = parse_one(blank).unwrap();
        assert!(launch.mission.is_none());
        assert_eq!(launch.orbit.as_deref(), Some("LEO"));
    }

    #[test]
    fn unknown_fields_in_the_feed_are_ignored() {
        let extra = r#"{"count": 1, "next": null, "results":[{
            "id": "a", "name": "n", "net": "2026-09-01T00:00:00Z",
            "something_new": { "nested": [1, 2, 3] }, "probability": 80
        }]}"#;
        assert!(parse_one(extra).is_some());
    }
}
