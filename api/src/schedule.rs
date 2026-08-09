use crate::config::Publish;
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Schedule {
    pub timezone: String,
    pub abbreviation: String,
    pub hour: u32,
    pub minute: u32,
    pub today: String,
    pub next_at: DateTime<Utc>,
}

impl Schedule {
    pub fn now(publish: &Publish) -> Self {
        Self::at(publish, Utc::now())
    }

    fn at(publish: &Publish, now: DateTime<Utc>) -> Self {
        let local = now.with_timezone(&publish.timezone);
        let today = local.date_naive();

        Self {
            timezone: publish.timezone.name().to_owned(),
            abbreviation: local.format("%Z").to_string(),
            hour: publish.hour,
            minute: publish.minute,
            today: today.format("%Y-%m-%d").to_string(),
            next_at: next_after(publish, now, today),
        }
    }
}

fn next_after(publish: &Publish, now: DateTime<Utc>, today: NaiveDate) -> DateTime<Utc> {
    let mut day = today;

    for _ in 0..3 {
        if let Some(instant) = instant_on(publish, day)
            && instant > now
        {
            return instant;
        }
        day = day.succ_opt().unwrap_or(day);
    }

    now + Duration::days(1)
}

pub fn instant_on(publish: &Publish, day: NaiveDate) -> Option<DateTime<Utc>> {
    let start = day.and_hms_opt(publish.hour, publish.minute, 0)?;

    (0..120)
        .filter_map(|minutes| start.checked_add_signed(Duration::minutes(minutes)))
        .take_while(|candidate: &NaiveDateTime| candidate.date() == day)
        .find_map(|candidate| {
            publish
                .timezone
                .from_local_datetime(&candidate)
                .earliest()
                .map(|resolved| resolved.with_timezone(&Utc))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::Tz;

    fn eastern(hour: u32, minute: u32) -> Publish {
        Publish {
            timezone: Tz::America__New_York,
            hour,
            minute,
        }
    }

    fn utc(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn midnight_in_new_york_is_five_in_the_morning_utc_in_winter() {
        let schedule = Schedule::at(&eastern(0, 0), utc("2026-01-15T12:00:00Z"));

        assert_eq!(schedule.today, "2026-01-15");
        assert_eq!(schedule.abbreviation, "EST");
        assert_eq!(schedule.next_at, utc("2026-01-16T05:00:00Z"));
    }

    #[test]
    fn the_same_midnight_is_four_in_the_morning_utc_in_summer() {
        let schedule = Schedule::at(&eastern(0, 0), utc("2026-07-15T12:00:00Z"));

        assert_eq!(schedule.abbreviation, "EDT");
        assert_eq!(schedule.next_at, utc("2026-07-16T04:00:00Z"));
    }

    #[test]
    fn a_slot_still_ahead_today_is_not_pushed_to_tomorrow() {
        let schedule = Schedule::at(&eastern(0, 0), utc("2026-01-15T03:00:00Z"));

        assert_eq!(schedule.today, "2026-01-14");
        assert_eq!(schedule.next_at, utc("2026-01-15T05:00:00Z"));
    }

    #[test]
    fn a_time_the_clocks_jumped_over_settles_on_the_next_one_that_exists() {
        let publish = Publish {
            timezone: Tz::Australia__Lord_Howe,
            hour: 2,
            minute: 15,
        };

        let next = Schedule::at(&publish, utc("2026-10-03T00:00:00Z")).next_at;
        let landed = next.with_timezone(&Tz::Australia__Lord_Howe);

        assert!(next > utc("2026-10-03T00:00:00Z"));
        assert_eq!(landed.format("%H:%M").to_string(), "02:30");
    }

    #[test]
    fn a_later_hour_works_the_same_way() {
        let schedule = Schedule::at(&eastern(17, 30), utc("2026-01-15T12:00:00Z"));
        assert_eq!(schedule.next_at, utc("2026-01-15T22:30:00Z"));
    }
}
