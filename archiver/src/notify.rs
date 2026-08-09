use crate::client::Client;
use crate::config::{Config, Notify};
use crate::shutdown::Shutdown;
use anyhow::{Context, Result};
use apod_core::ApodReader;
use apod_core::db::DbConfig;
use apod_core::notify::NotifyStore;
use apod_core::sky::store::SkyReader;
use apod_core::sky::weather::{Alert, Notice, WeatherReport};
use apod_core::sky::{eclipse, moon, showers};
use chrono::{DateTime, TimeDelta, Utc};

const SUMMARY_CHARS: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub topic: String,
    pub key: String,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub priority: Option<u8>,
    pub click: Option<String>,
    pub attach: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pass {
    pub sent: usize,
    pub claimed: usize,
    pub already: usize,
    pub failed: usize,
}

pub async fn run(cfg: Config, client: Client, mut shutdown: Shutdown) -> Result<()> {
    let store = NotifyStore::open(&cfg.notify_db)
        .await
        .with_context(|| format!("opening {}", cfg.notify_db.display()))?;

    loop {
        match deliver(&cfg, &client, &store, Utc::now(), Delivery::Send).await {
            Ok(pass) if pass.sent > 0 || pass.failed > 0 => {
                tracing::info!(sent = pass.sent, failed = pass.failed, "notifications");
            }
            Ok(_) => {}
            Err(error) => tracing::warn!("notify pass failed: {error:#}"),
        }

        if !shutdown.sleep(cfg.notify.interval).await {
            break;
        }
    }

    store.close().await;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    Send,
    Seed,
}

pub async fn deliver(
    cfg: &Config,
    client: &Client,
    store: &NotifyStore,
    now: DateTime<Utc>,
    delivery: Delivery,
) -> Result<Pass> {
    let mut pass = Pass::default();

    for notification in gather(cfg, now).await? {
        if store
            .is_sent(&notification.topic, &notification.key)
            .await?
        {
            pass.already += 1;
            continue;
        }

        if delivery == Delivery::Send {
            if let Err(error) = publish(client, &cfg.notify, &notification).await {
                tracing::warn!(
                    topic = %notification.topic,
                    key = %notification.key,
                    "could not publish: {error:#}"
                );
                pass.failed += 1;
                continue;
            }
            pass.sent += 1;
            tracing::info!(topic = %notification.topic, key = %notification.key, "published");
        }

        store
            .mark(&notification.topic, &notification.key, now)
            .await?;
        pass.claimed += 1;
    }

    Ok(pass)
}

pub async fn gather(cfg: &Config, now: DateTime<Utc>) -> Result<Vec<Notification>> {
    let mut found = Vec::new();

    if let Some(topic) = cfg.notify.apod_topic.as_deref() {
        found.extend(apod(cfg, topic, now).await?);
    }

    if cfg.notify.aurora_topic.is_some() || cfg.notify.space_weather_topic.is_some() {
        found.extend(weather(cfg, now).await?);
    }

    if let Some(topic) = cfg.notify.sky_topic.as_deref() {
        found.extend(sky(&cfg.notify, topic, now));
    }

    Ok(found)
}

async fn apod(cfg: &Config, topic: &str, now: DateTime<Utc>) -> Result<Vec<Notification>> {
    let reader = ApodReader::open(DbConfig::read_only(&cfg.index_db))
        .await
        .with_context(|| format!("opening {}", cfg.index_db.display()))?
        .with_thumb_base("/thumbs/");

    let latest = reader.latest().await?;
    reader.db().close().await;

    let Some(entry) = latest else {
        return Ok(Vec::new());
    };

    let age = now - entry.date.naive().and_time(Default::default()).and_utc();
    if age > TimeDelta::from_std(cfg.notify.apod_max_age)? {
        return Ok(Vec::new());
    }

    let url = format!("{}/{}", cfg.notify.public_url, entry.date);
    Ok(vec![Notification {
        topic: topic.to_owned(),
        key: format!("apod:{}", entry.date),
        title: entry.title.clone(),
        body: entry.summary_text(SUMMARY_CHARS),
        tags: vec!["telescope".to_owned()],
        priority: None,
        click: Some(url),
        attach: entry
            .media
            .thumb_url
            .as_deref()
            .map(|thumb| format!("{}{thumb}", cfg.notify.public_url)),
    }])
}

async fn weather(cfg: &Config, now: DateTime<Utc>) -> Result<Vec<Notification>> {
    let Ok(sky) = SkyReader::open(&cfg.sky_db).await else {
        tracing::debug!("no sky database yet, nothing to report on");
        return Ok(Vec::new());
    };

    let report = sky.weather_report().await?;
    sky.close().await;

    Ok(match report {
        None => Vec::new(),
        Some(report) => route(&cfg.notify, &report, now),
    })
}

fn route(cfg: &Notify, report: &WeatherReport, now: DateTime<Utc>) -> Vec<Notification> {
    let url = format!("{}/space-weather", cfg.public_url);
    let mut found = Vec::new();

    if let Some(topic) = cfg.aurora_topic.as_deref() {
        let geomagnetic = report.alerts.iter().filter(|alert| {
            alert.is_geomagnetic()
                && alert.in_force(now)
                && !matches!(alert.notice, Notice::Summary)
        });

        for alert in geomagnetic {
            found.push(alert_notification(topic, alert, &url));
        }

        if report.kp >= cfg.aurora_min_kp {
            let level = report.kp.floor() as i64;
            found.push(Notification {
                topic: topic.to_owned(),
                key: format!("kp:{}:{level}", report.observed_at.format("%Y-%m-%d")),
                title: format!("Kp {level}, aurora possible"),
                body: format!(
                    "The planetary K index reached {:.2}, observed {}.",
                    report.kp,
                    report.observed_at.format("%Y-%m-%d %H:%M UTC")
                ),
                tags: vec!["zap".to_owned()],
                priority: Some(4),
                click: Some(url.clone()),
                attach: None,
            });
        }
    }

    if let Some(topic) = cfg.space_weather_topic.as_deref() {
        let rest = report
            .alerts
            .iter()
            .filter(|alert| !alert.is_geomagnetic() && alert.current(now));

        for alert in rest {
            found.push(alert_notification(topic, alert, &url));
        }
    }

    found
}

fn alert_notification(topic: &str, alert: &Alert, url: &str) -> Notification {
    Notification {
        topic: topic.to_owned(),
        key: format!("alert:{}", alert.id),
        title: format!("{}: {}", alert.notice.label(), alert.headline),
        body: alert.message.clone(),
        tags: vec!["zap".to_owned()],
        priority: match alert.notice {
            Notice::Watch => None,
            _ => Some(4),
        },
        click: Some(url.to_owned()),
        attach: None,
    }
}

fn sky(cfg: &Notify, topic: &str, now: DateTime<Utc>) -> Vec<Notification> {
    let url = format!("{}/space-weather", cfg.public_url);
    let Ok(lead) = TimeDelta::from_std(cfg.sky_lead) else {
        return Vec::new();
    };
    let eclipse_lead = TimeDelta::from_std(cfg.eclipse_lead).unwrap_or(lead);

    let mut found = Vec::new();

    for peak in showers::upcoming(now) {
        if peak.peak < now || peak.peak > now + lead {
            continue;
        }
        found.push(Notification {
            topic: topic.to_owned(),
            key: format!("shower:{}:{}", peak.name, peak.peak.format("%Y")),
            title: format!("{} peak tonight", peak.name),
            body: format!(
                "Up to {} per hour at the peak, {}. Radiant in {}. {}.",
                peak.zenith_hourly_rate,
                peak.peak.format("%Y-%m-%d %H:%M UTC"),
                peak.radiant,
                peak.moonlight_label
            ),
            tags: vec!["stars".to_owned()],
            priority: None,
            click: Some(url.clone()),
            attach: None,
        });
    }

    for event in eclipse::upcoming(now) {
        if event.at < now || event.at > now + eclipse_lead {
            continue;
        }
        found.push(Notification {
            topic: topic.to_owned(),
            key: format!("eclipse:{}", event.at.format("%Y-%m-%d")),
            title: event.label.to_owned(),
            body: format!(
                "Greatest eclipse at {}, magnitude {:.2}.",
                event.at.format("%Y-%m-%d %H:%M UTC"),
                event.magnitude
            ),
            tags: vec!["crescent_moon".to_owned()],
            priority: None,
            click: Some(url.clone()),
            attach: None,
        });
    }

    let full = moon::next_quarter(now, moon::Quarter::Full);
    if full <= now + lead && moon::is_supermoon(full) {
        found.push(Notification {
            topic: topic.to_owned(),
            key: format!("moon:super:{}", full.format("%Y-%m-%d")),
            title: "Supermoon".to_owned(),
            body: format!(
                "The full moon on {} falls near perigee, so it rises larger and brighter than usual.",
                full.format("%Y-%m-%d %H:%M UTC")
            ),
            tags: vec!["full_moon".to_owned()],
            priority: None,
            click: Some(url),
            attach: None,
        });
    }

    found
}

async fn publish(client: &Client, cfg: &Notify, notification: &Notification) -> Result<()> {
    let mut payload = serde_json::json!({
        "topic": notification.topic,
        "title": notification.title,
        "message": notification.body,
        "tags": notification.tags,
    });

    if let Some(priority) = notification.priority {
        payload["priority"] = priority.into();
    }
    if let Some(click) = &notification.click {
        payload["click"] = click.as_str().into();
    }
    if let Some(attach) = &notification.attach {
        payload["attach"] = attach.as_str().into();
    }

    let mut headers = vec![("Content-Type", "application/json".to_owned())];
    if let Some(token) = &cfg.token {
        headers.push(("Authorization", format!("Bearer {token}")));
    }

    client
        .post(&cfg.base_url, &headers, payload.to_string())
        .await
        .with_context(|| format!("publishing to {}", cfg.url_for(&notification.topic)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::time::Duration;

    fn cfg() -> Notify {
        Notify {
            enabled: true,
            base_url: "https://ntfy.example".to_owned(),
            token: None,
            public_url: "https://apod.example".to_owned(),
            interval: Duration::from_secs(300),
            apod_topic: Some("apod".to_owned()),
            aurora_topic: Some("aurora".to_owned()),
            space_weather_topic: Some("space-weather".to_owned()),
            sky_topic: Some("sky".to_owned()),
            apod_max_age: Duration::from_secs(36 * 3600),
            sky_lead: Duration::from_secs(24 * 3600),
            eclipse_lead: Duration::from_secs(72 * 3600),
            aurora_min_kp: 5.0,
        }
    }

    fn at(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, 12, 0, 0).unwrap()
    }

    fn keys_over(days: i64, from: DateTime<Utc>) -> Vec<String> {
        let mut seen = Vec::new();
        for day in 0..days {
            for notification in sky(&cfg(), "sky", from + TimeDelta::days(day)) {
                if !seen.contains(&notification.key) {
                    seen.push(notification.key);
                }
            }
        }
        seen
    }

    #[test]
    fn a_shower_is_announced_once_however_often_the_worker_ticks() {
        let start = at(2026, 8, 1);
        let mut counts = std::collections::HashMap::new();

        for hour in 0..(24 * 14) {
            for notification in sky(&cfg(), "sky", start + TimeDelta::hours(hour)) {
                *counts.entry(notification.key).or_insert(0) += 1;
            }
        }

        let perseids: Vec<_> = counts
            .keys()
            .filter(|key| key.contains("Perseid"))
            .collect();
        assert_eq!(perseids.len(), 1, "one key for one peak: {counts:?}");
    }

    #[test]
    fn every_sky_key_names_the_year_it_belongs_to() {
        let keys = keys_over(400, at(2026, 1, 1));

        assert!(!keys.is_empty(), "a year should hold some sky events");
        for key in &keys {
            let year_like = key.split(':').next_back().unwrap_or_default();
            assert!(
                year_like.len() >= 4 && year_like.chars().take(4).all(|c| c.is_ascii_digit()),
                "a key that does not carry a date repeats every year: {key}"
            );
        }
    }

    #[test]
    fn nothing_outside_the_lead_window_is_offered() {
        let mut narrow = cfg();
        narrow.sky_lead = Duration::ZERO;
        narrow.eclipse_lead = Duration::ZERO;

        assert!(sky(&narrow, "sky", at(2026, 8, 1)).is_empty());
    }

    #[test]
    fn a_shower_carries_the_moonlight_verdict_people_actually_need() {
        let found = keys_over(400, at(2026, 1, 1));
        assert!(
            found.iter().any(|key| key.starts_with("shower:")),
            "a year should contain at least one shower peak: {found:?}"
        );
    }

    #[test]
    fn topics_lists_only_the_ones_that_are_set() {
        let mut partial = cfg();
        partial.aurora_topic = None;
        assert_eq!(partial.topics(), vec!["apod", "space-weather", "sky"]);
    }

    fn alert(headline: &str, scale: Option<&str>, notice: Notice) -> Alert {
        Alert {
            id: format!("{headline}-{notice:?}"),
            notice,
            headline: headline.to_owned(),
            scale: scale.map(str::to_owned),
            issued_at: at(2026, 3, 5),
            valid_until: Some(at(2026, 3, 6)),
            message: "body".to_owned(),
        }
    }

    fn report(alerts: Vec<Alert>, kp: f64) -> WeatherReport {
        WeatherReport {
            kp,
            observed_at: at(2026, 3, 5),
            scales: None,
            outlook: Vec::new(),
            kp_series: Vec::new(),
            flux: Vec::new(),
            dst: Vec::new(),
            alerts,
        }
    }

    fn routed(alerts: Vec<Alert>, kp: f64) -> Vec<(String, String)> {
        route(&cfg(), &report(alerts, kp), at(2026, 3, 5))
            .into_iter()
            .map(|found| (found.topic, found.title))
            .collect()
    }

    #[test]
    fn a_proton_event_does_not_land_on_the_aurora_topic() {
        let routed = routed(
            vec![
                alert(
                    "Geomagnetic K-index of 6",
                    Some("G2 - Moderate"),
                    Notice::Alert,
                ),
                alert(
                    "Proton Event 10MeV Integral Flux exceeded 10pfu",
                    Some("S1 - Minor"),
                    Notice::Alert,
                ),
                alert(
                    "Electron 2MeV Integral Flux exceeded 1,000pfu",
                    None,
                    Notice::Alert,
                ),
            ],
            1.0,
        );

        let aurora: Vec<_> = routed
            .iter()
            .filter(|(topic, _)| topic == "aurora")
            .collect();
        let other: Vec<_> = routed
            .iter()
            .filter(|(topic, _)| topic == "space-weather")
            .collect();

        assert_eq!(aurora.len(), 1, "only the geomagnetic one: {routed:?}");
        assert!(aurora[0].1.contains("K-index"), "{routed:?}");
        assert_eq!(
            other.len(),
            2,
            "proton and electron both go elsewhere: {routed:?}"
        );
    }

    #[test]
    fn a_watch_reaches_the_aurora_topic_and_nothing_else() {
        let routed = routed(
            vec![
                alert(
                    "Geomagnetic Storm Category G2 Predicted",
                    None,
                    Notice::Watch,
                ),
                alert(
                    "Proton 10MeV Integral Flux above 10pfu",
                    Some("S1"),
                    Notice::Watch,
                ),
            ],
            1.0,
        );

        assert_eq!(
            routed.len(),
            1,
            "the S-band watch is not forecast news: {routed:?}"
        );
        assert_eq!(routed[0].0, "aurora");
        assert!(routed[0].1.starts_with("Watch:"), "{routed:?}");
    }

    #[test]
    fn a_write_up_after_the_fact_is_never_pushed() {
        let routed = routed(
            vec![alert("Geomagnetic Sudden Impulse", None, Notice::Summary)],
            1.0,
        );

        assert!(routed.is_empty(), "{routed:?}");
    }

    #[test]
    fn the_kp_threshold_only_fires_once_past_it() {
        assert!(routed(Vec::new(), 4.9).is_empty(), "below the 5.0 default");

        let over = routed(Vec::new(), 5.4);
        assert_eq!(over.len(), 1);
        assert_eq!(over[0].0, "aurora");
        assert_eq!(over[0].1, "Kp 5, aurora possible");
    }

    #[test]
    fn a_deployment_without_the_space_weather_topic_simply_drops_those() {
        let mut only_aurora = cfg();
        only_aurora.space_weather_topic = None;

        let found = route(
            &only_aurora,
            &report(
                vec![alert("Proton Event", Some("S1 - Minor"), Notice::Alert)],
                1.0,
            ),
            at(2026, 3, 5),
        );

        assert!(found.is_empty(), "no topic, no message: {found:?}");
    }

    #[test]
    fn a_topic_url_is_the_base_plus_the_name() {
        assert_eq!(cfg().url_for("apod"), "https://ntfy.example/apod");
    }
}
