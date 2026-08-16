use crate::client::{Client, Limit, Response};
use crate::config::Sky;
use anyhow::{Context, Result};
use apod_core::sky::weather::{
    Alert, Band, DstPoint, FluxPoint, KpPoint, Level, Notice, ScaleDay, WeatherReport,
};
use chrono::{DateTime, NaiveDateTime, TimeDelta, Utc};
use serde::Deserialize;
use std::collections::BTreeMap;

const BODY_LIMIT: Limit = Limit {
    max_bytes: 4 * 1_048_576,
    timeout: std::time::Duration::from_secs(30),
};

const KP_DAYS: i64 = 4;
const DST_DAYS: i64 = 7;
const MAX_ALERTS: usize = 40;

pub async fn report(cfg: &Sky, client: &Client) -> Result<WeatherReport> {
    let kp_series = kp_series(cfg, client)
        .await
        .context("the Kp series is what the whole report hangs on")?;

    let latest = kp_series
        .iter()
        .filter(|point| !point.ahead)
        .max_by_key(|point| point.at)
        .context("the Kp feed carried nothing that has already happened")?;

    let (scales, outlook) =
        optional("the NOAA scales", scales(cfg, client).await).unwrap_or((None, Vec::new()));

    Ok(WeatherReport {
        kp: latest.kp,
        observed_at: latest.at,
        scales,
        outlook,
        kp_series,
        flux: optional("the 10cm flux", flux(cfg, client).await).unwrap_or_default(),
        dst: optional("the Dst index", dst(cfg, client).await).unwrap_or_default(),
        alerts: optional("the alert feed", alerts(cfg, client).await).unwrap_or_default(),
    })
}

fn optional<T>(what: &str, outcome: Result<T>) -> Option<T> {
    match outcome {
        Ok(value) => Some(value),
        Err(error) => {
            tracing::warn!("{what} could not be read: {error:#}");
            None
        }
    }
}

async fn get<T: serde::de::DeserializeOwned>(client: &Client, url: &str) -> Result<T> {
    let body = match client.get_limited(url, BODY_LIMIT).await? {
        Response::Body(bytes) => bytes,
        Response::NotFound => anyhow::bail!("{url} returned 404"),
    };

    serde_json::from_slice(&body).with_context(|| format!("parsing {url}"))
}

fn product(cfg: &Sky, name: &str) -> String {
    format!("{}/{name}", cfg.swpc_base_url)
}

#[derive(Debug, Deserialize)]
struct RawKp {
    time_tag: String,
    kp: Option<f64>,
    observed: Option<String>,
}

async fn kp_series(cfg: &Sky, client: &Client) -> Result<Vec<KpPoint>> {
    let raw: Vec<RawKp> = get(
        client,
        &product(cfg, "noaa-planetary-k-index-forecast.json"),
    )
    .await?;

    let now = Utc::now();
    let floor = now - TimeDelta::days(KP_DAYS);

    let mut points: Vec<KpPoint> = raw
        .into_iter()
        .filter_map(|row| {
            let at = parse_time(&row.time_tag)?;
            Some(KpPoint {
                at,
                kp: row.kp?,
                ahead: row.observed.as_deref() != Some("observed") || at > now,
            })
        })
        .filter(|point| point.at >= floor)
        .collect();

    points.sort_by_key(|point| point.at);
    Ok(points)
}

#[derive(Debug, Deserialize)]
struct RawScales {
    #[serde(rename = "DateStamp")]
    date: Option<String>,
    #[serde(rename = "R")]
    r: Option<RawLevel>,
    #[serde(rename = "S")]
    s: Option<RawLevel>,
    #[serde(rename = "G")]
    g: Option<RawLevel>,
}

#[derive(Debug, Deserialize)]
struct RawLevel {
    #[serde(rename = "Scale")]
    scale: Option<String>,
    #[serde(rename = "Text")]
    text: Option<String>,
}

async fn scales(cfg: &Sky, client: &Client) -> Result<(Option<ScaleDay>, Vec<ScaleDay>)> {
    let raw: BTreeMap<String, RawScales> = get(client, &product(cfg, "noaa-scales.json")).await?;

    let day = |key: &str| raw.get(key).and_then(day_from_scales);
    let outlook = ["1", "2", "3"].into_iter().filter_map(day).collect();

    Ok((day("0"), outlook))
}

fn day_from_scales(raw: &RawScales) -> Option<ScaleDay> {
    let levels: Vec<Level> = [(Band::G, &raw.g), (Band::S, &raw.s), (Band::R, &raw.r)]
        .into_iter()
        .filter_map(|(band, level)| {
            let level = level.as_ref()?;
            Some(Level {
                band,
                scale: level.scale.as_deref().and_then(|scale| scale.parse().ok()),
                text: level.text.clone().filter(|text| !text.trim().is_empty()),
            })
        })
        .collect();

    (!levels.is_empty()).then(|| ScaleDay {
        date: raw.date.clone().unwrap_or_default(),
        levels,
    })
}

#[derive(Debug, Deserialize)]
struct RawFlux {
    time_tag: String,
    flux: Option<f64>,
}

async fn flux(cfg: &Sky, client: &Client) -> Result<Vec<FluxPoint>> {
    let raw: Vec<RawFlux> = get(client, &product(cfg, "10cm-flux-30-day.json")).await?;

    let mut points: Vec<FluxPoint> = raw
        .into_iter()
        .filter_map(|row| {
            Some(FluxPoint {
                at: parse_time(&row.time_tag)?,
                flux: row.flux?,
            })
        })
        .collect();

    points.sort_by_key(|point| point.at);
    Ok(points)
}

#[derive(Debug, Deserialize)]
struct RawDst {
    time_tag: String,
    dst: Option<f64>,
}

async fn dst(cfg: &Sky, client: &Client) -> Result<Vec<DstPoint>> {
    let raw: Vec<RawDst> = get(client, &product(cfg, "kyoto-dst.json")).await?;

    let floor = Utc::now() - TimeDelta::days(DST_DAYS);
    let mut points: Vec<DstPoint> = raw
        .into_iter()
        .filter_map(|row| {
            Some(DstPoint {
                at: parse_time(&row.time_tag)?,
                dst: row.dst?,
            })
        })
        .filter(|point| point.at >= floor)
        .collect();

    points.sort_by_key(|point| point.at);
    Ok(points)
}

#[derive(Debug, Deserialize)]
struct RawAlert {
    product_id: String,
    issue_datetime: String,
    message: String,
}

async fn alerts(cfg: &Sky, client: &Client) -> Result<Vec<Alert>> {
    let raw: Vec<RawAlert> = get(client, &product(cfg, "alerts.json")).await?;

    let mut alerts: Vec<Alert> = raw.into_iter().filter_map(convert_alert).collect();
    alerts.sort_by_key(|alert| std::cmp::Reverse(alert.issued_at));
    alerts.truncate(MAX_ALERTS);

    Ok(alerts)
}

fn convert_alert(raw: RawAlert) -> Option<Alert> {
    let issued_at = parse_time(&raw.issue_datetime)?;
    let message = raw.message.replace('\r', "");
    let lines: Vec<&str> = message.lines().map(str::trim).collect();

    let (notice, headline) = lines.iter().find_map(|line| headline_of(line))?;

    Some(Alert {
        id: format!("{}-{}", raw.product_id, issued_at.timestamp()),
        notice,
        headline,
        scale: field(&lines, "NOAA Scale:"),
        issued_at,
        valid_until: field(&lines, "Now Valid Until:")
            .or_else(|| field(&lines, "Valid To:"))
            .as_deref()
            .and_then(parse_noaa_time),
        message: message.trim().to_owned(),
    })
}

fn headline_of(line: &str) -> Option<(Notice, String)> {
    let (kind, rest) = line.split_once(':')?;
    let notice = match kind.trim() {
        "ALERT" => Notice::Alert,
        "WARNING" | "EXTENDED WARNING" | "CANCEL WARNING" => Notice::Warning,
        "WATCH" => Notice::Watch,
        "SUMMARY" => Notice::Summary,
        _ => return None,
    };

    let headline = rest.trim();
    (!headline.is_empty()).then(|| (notice, headline.to_owned()))
}

fn field(lines: &[&str], name: &str) -> Option<String> {
    lines
        .iter()
        .find_map(|line| line.strip_prefix(name))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn parse_time(raw: &str) -> Option<DateTime<Utc>> {
    let raw = raw.trim();

    for format in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d"] {
        if let Ok(at) = NaiveDateTime::parse_from_str(raw, format) {
            return Some(at.and_utc());
        }
    }

    chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .ok()
        .map(|date| date.and_hms_opt(0, 0, 0).unwrap_or_default().and_utc())
}

fn parse_noaa_time(raw: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(raw.trim().trim_end_matches(" UTC"), "%Y %b %d %H%M")
        .ok()
        .map(|at| at.and_utc())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_timestamp_spellings_in_the_products_parse() {
        assert!(parse_time("2026-08-09T10:00:00").is_some());
        assert!(parse_time("2026-08-08 21:08:07.987").is_some());
        assert!(parse_time("2026-08-09").is_some());
        assert!(parse_time("not a time").is_none());
    }

    #[test]
    fn the_timestamps_inside_a_message_parse() {
        let until = parse_noaa_time("2026 Aug 08 2359 UTC").expect("it reads");
        assert_eq!(until.to_rfc3339(), "2026-08-08T23:59:00+00:00");
        assert!(parse_noaa_time("sometime soon").is_none());
    }

    fn alert_of(message: &str) -> Option<Alert> {
        convert_alert(RawAlert {
            product_id: "K06A".to_owned(),
            issue_datetime: "2026-08-08 21:08:07.987".to_owned(),
            message: message.to_owned(),
        })
    }

    #[test]
    fn a_real_alert_message_gives_up_its_headline_and_scale() {
        let alert = alert_of(
            "Space Weather Message Code: ALTK06\r\nSerial Number: 725\r\n\
             Issue Time: 2026 Aug 08 2108 UTC\r\n\r\n\
             ALERT: Geomagnetic K-index of 6\n\
             Threshold Reached: 2026 Aug 08 2100 UTC\n\
             NOAA Scale: G2 - Moderate\r\n",
        )
        .expect("it parses");

        assert_eq!(alert.notice, Notice::Alert);
        assert_eq!(alert.headline, "Geomagnetic K-index of 6");
        assert_eq!(alert.scale.as_deref(), Some("G2 - Moderate"));
        assert_eq!(alert.valid_until, None);
        assert!(alert.id.starts_with("K06A-"));
    }

    #[test]
    fn an_extended_warning_reads_as_a_warning_and_keeps_its_expiry() {
        let alert = alert_of(
            "Space Weather Message Code: WARK05\n\n\
             EXTENDED WARNING: Geomagnetic K-index of 5 expected\n\
             Valid From: 2026 Aug 08 1222 UTC\n\
             Now Valid Until: 2026 Aug 08 2359 UTC\n\
             NOAA Scale: G1 - Minor\n",
        )
        .expect("it parses");

        assert_eq!(alert.notice, Notice::Warning);
        assert_eq!(alert.headline, "Geomagnetic K-index of 5 expected");
        assert_eq!(
            alert.valid_until.map(|at| at.to_rfc3339()),
            Some("2026-08-08T23:59:00+00:00".to_owned())
        );
    }

    #[test]
    fn a_watch_and_a_write_up_are_told_apart() {
        let watch = alert_of("WATCH: Geomagnetic Storm Category G1 Predicted \n").expect("parses");
        assert_eq!(watch.notice, Notice::Watch);
        assert_eq!(watch.headline, "Geomagnetic Storm Category G1 Predicted");

        let summary = alert_of("SUMMARY: 10cm Radio Burst \nPeak Flux: 190 sfu\n").expect("parses");
        assert_eq!(summary.notice, Notice::Summary);
        assert!(summary.scale.is_none());
    }

    #[test]
    fn a_message_with_no_headline_is_dropped_rather_than_shown_blank() {
        assert!(alert_of("Space Weather Message Code: XYZ\nSerial Number: 1\n").is_none());
        assert!(
            alert_of("ALERT: \n").is_none(),
            "an empty headline is no headline"
        );
    }

    #[test]
    fn the_scales_payload_maps_across() {
        let raw: BTreeMap<String, RawScales> = serde_json::from_str(
            r#"{
              "0": {"DateStamp":"2026-08-09","TimeStamp":"10:53:00",
                    "R":{"Scale":"1","Text":"minor","MinorProb":null},
                    "S":{"Scale":"0","Text":"none","Prob":null},
                    "G":{"Scale":"2","Text":"moderate"}},
              "1": {"DateStamp":"2026-08-09",
                    "R":{"Scale":null,"Text":null,"MinorProb":"1"},
                    "S":{"Scale":null,"Text":null,"Prob":"1"},
                    "G":{"Scale":"0","Text":"none"}}
            }"#,
        )
        .expect("the payload parses");

        let today = day_from_scales(raw.get("0").unwrap()).expect("today is there");
        assert_eq!(today.date, "2026-08-09");
        assert_eq!(today.levels.len(), 3);
        assert_eq!(today.worst().map(|level| level.band), Some(Band::G));
        assert!(!today.quiet());
        assert_eq!(
            today
                .levels
                .iter()
                .map(|level| level.band)
                .collect::<Vec<_>>(),
            vec![Band::G, Band::S, Band::R],
            "stored geomagnetic first, whichever way round NOAA sent it"
        );

        let ahead = day_from_scales(raw.get("1").unwrap()).expect("tomorrow is there");
        assert_eq!(
            ahead
                .levels
                .iter()
                .filter(|level| level.scale.is_none())
                .count(),
            2,
            "a probability without a scale comes through as no scale rather than as zero"
        );
    }
}
