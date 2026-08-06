use crate::api::error::{ApiError, ApiResult};
use crate::store::{Filters, Order};
use apod_core::{ApodDate, MediaKind};
use std::str::FromStr;

pub fn date(raw: &str) -> ApiResult<ApodDate> {
    raw.parse()
        .map_err(|_| ApiError::bad_request(format!("'{raw}' is not a YYYY-MM-DD date")))
}

pub fn optional_date(raw: Option<&str>) -> ApiResult<Option<ApodDate>> {
    raw.map(date).transpose()
}

pub fn kind(raw: &str) -> ApiResult<MediaKind> {
    MediaKind::from_str(raw)
        .map_err(|_| ApiError::bad_request(format!("unknown media kind '{raw}'")))
}

pub fn filters(
    from: Option<&str>,
    to: Option<&str>,
    media_kind: Option<&str>,
    copyright: Option<bool>,
) -> ApiResult<Filters> {
    Ok(Filters {
        from: optional_date(from)?,
        to: optional_date(to)?,
        kind: media_kind.map(kind).transpose()?,
        copyright,
    })
}

pub fn order(raw: Option<&str>) -> ApiResult<Order> {
    match raw {
        None | Some("desc") => Ok(Order::Desc),
        Some("asc") => Ok(Order::Asc),
        Some(other) => Err(ApiError::bad_request(format!("unknown order '{other}'"))),
    }
}

pub fn sort_by_date(raw: Option<&str>) -> ApiResult<bool> {
    match raw {
        None | Some("relevance") => Ok(false),
        Some("date") => Ok(true),
        Some(other) => Err(ApiError::bad_request(format!("unknown sort '{other}'"))),
    }
}

pub fn limit(requested: Option<usize>, default: usize, max: usize) -> usize {
    requested.unwrap_or(default).clamp(1, max)
}

pub fn month_day(raw: &str) -> ApiResult<(u32, u32)> {
    raw.split_once('-')
        .and_then(|(month, day)| Some((month.parse::<u32>().ok()?, day.parse::<u32>().ok()?)))
        .filter(|(month, day)| (1..=12).contains(month) && (1..=31).contains(day))
        .ok_or_else(|| ApiError::bad_request("expected MM-DD"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_limits_inside_the_configured_ceiling() {
        assert_eq!(limit(None, 30, 100), 30);
        assert_eq!(limit(Some(0), 30, 100), 1);
        assert_eq!(limit(Some(5_000), 30, 100), 100);
        assert_eq!(limit(Some(50), 30, 100), 50);
    }

    #[test]
    fn rejects_malformed_input() {
        assert!(date("not-a-date").is_err());
        assert!(kind("mystery").is_err());
        assert!(order(Some("sideways")).is_err());
        assert!(sort_by_date(Some("vibes")).is_err());
        assert!(month_day("13-01").is_err());
        assert!(month_day("nonsense").is_err());
    }

    #[test]
    fn accepts_valid_input() {
        assert!(date("2024-03-05").is_ok());
        assert!(kind("image_jpg").is_ok());
        assert_eq!(order(None).unwrap(), Order::Desc);
        assert_eq!(order(Some("asc")).unwrap(), Order::Asc);
        assert!(!sort_by_date(None).unwrap());
        assert_eq!(month_day("03-05").unwrap(), (3, 5));
    }
}
