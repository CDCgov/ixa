//! Calendar dates for parameters. A model's times are days from day 0;
//! with a start date, a parameters document may write a day field as a
//! `YYYY-MM-DD` date instead. The `#[calendar]` attribute (`ixa::calendar`,
//! the macro that shares this module's name) marks which fields are days on
//! that calendar:
//!
//! ```
//! use ixa::calendar;
//! use ixa::calendar::Date;
//! use serde::{Deserialize, Serialize};
//!
//! #[calendar]
//! #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
//! #[serde(rename_all = "camelCase")]
//! pub struct Policy {
//!     #[day]
//!     pub start: f64,
//!     #[day]
//!     pub end: Option<f64>,
//!     /// Days after the start: a duration, so not marked.
//!     pub delay: f64,
//! }
//!
//! #[calendar]
//! #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
//! #[serde(rename_all = "camelCase")]
//! pub struct Parameters {
//!     #[reference]
//!     pub start_date: Option<Date>,
//!     #[days]
//!     pub policies: Vec<Policy>,
//!     #[day(through)]
//!     pub max_time: f64,
//! }
//!
//! let p: Parameters = serde_json::from_str(r#"{
//!     "startDate": "2026-05-14",
//!     "policies": [{"start": "2026-06-01", "end": 48, "delay": 3}],
//!     "maxTime": "2026-08-05"
//! }"#).unwrap();
//! assert_eq!(p.policies[0].start, 18.0);
//! assert_eq!(p.policies[0].end, Some(48.0));
//! assert_eq!(p.max_time, 84.0); // a date runs through its day
//!
//! // Written back as days, or as dates with `dated()`.
//! assert!(serde_json::to_string(&p).unwrap().contains(r#""start":18.0"#));
//! assert!(serde_json::to_string(&p.dated()).unwrap().contains(r#""start":"2026-06-01""#));
//! ```
//!
//! The attribute generates a twin of the struct, `CalendarPolicy` for
//! `Policy`, with [`Day`] in place of each `#[day]` field and the twin of
//! each `#[days]` field's type (a struct, a `Vec` or an `Option` of one),
//! and implements [`Calendar`] to convert between the two given the date
//! of day 0. The struct with the `#[reference]` field, the start date
//! (`Date` or `Option<Date>`), is the root: it deserializes through its
//! twin (`#[serde(try_from)]`), serializes as days, and gets `dated()`
//! for the twin with dates. `#[day(through)]` is for a day a date runs
//! through, such as the end of a run, so that `"2026-08-05"` covers that
//! whole day. Everything else in days is a duration, left unmarked.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// A calendar date, written and read as `YYYY-MM-DD`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    /// A date, if `month` and `day` are in range.
    ///
    /// # Errors
    /// When the month is not 1 to 12 or the day is not one of that month.
    pub fn new(year: i32, month: u32, day: u32) -> Result<Self, CalendarError> {
        let days_in_month = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
            2 => 28,
            _ => {
                return Err(CalendarError::InvalidDate(format!(
                    "month {month} is not 1 to 12"
                )))
            }
        };
        if day == 0 || day > days_in_month {
            return Err(CalendarError::InvalidDate(format!(
                "day {day} is not 1 to {days_in_month}"
            )));
        }
        Ok(Self { year, month, day })
    }

    /// Days since 1970-01-01, negative before it.
    fn days_since_epoch(self) -> i64 {
        let (y, m, d) = (
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        );
        let y = if m <= 2 { y - 1 } else { y };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn from_days_since_epoch(days: i64) -> Self {
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        Self {
            year: year as i32,
            month: month as u32,
            day: day as u32,
        }
    }

    /// The date `days` after this one (before it if negative).
    #[must_use]
    pub fn plus_days(self, days: i64) -> Self {
        Self::from_days_since_epoch(self.days_since_epoch() + days)
    }

    /// Days from this date to `other`, negative if `other` is earlier.
    #[must_use]
    pub fn days_until(self, other: Date) -> i64 {
        other.days_since_epoch() - self.days_since_epoch()
    }
}

impl FromStr for Date {
    type Err = CalendarError;

    fn from_str(s: &str) -> Result<Self, CalendarError> {
        let invalid = || CalendarError::InvalidDate(format!("{s:?} is not a date (YYYY-MM-DD)"));
        let mut parts = s.split('-');
        let mut next = |len: usize| {
            parts
                .next()
                .filter(|p| p.len() == len)
                .and_then(|p| p.parse::<u32>().ok())
                .ok_or_else(invalid)
        };
        let (year, month, day) = (next(4)?, next(2)?, next(2)?);
        if parts.next().is_some() {
            return Err(invalid());
        }
        #[allow(clippy::cast_possible_wrap)]
        Self::new(year as i32, month, day)
            .map_err(|e| CalendarError::InvalidDate(format!("{s:?}: {e}")))
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl Serialize for Date {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// The key the `toml` crate gives a native TOML date when a document is
/// read through a self-describing format (`serde_json::Value`, say).
const TOML_DATETIME: &str = "$__toml_private_datetime";

/// A date as a document writes it: `YYYY-MM-DD`, or a native TOML date.
struct DateVisitor;

impl<'de> Visitor<'de> for DateVisitor {
    type Value = Date;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a date, YYYY-MM-DD")
    }

    fn visit_str<E: de::Error>(self, s: &str) -> Result<Date, E> {
        s.parse().map_err(E::custom)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Date, A::Error> {
        let mut text = None;
        while let Some(key) = map.next_key::<String>()? {
            if key != TOML_DATETIME {
                return Err(de::Error::unknown_field(&key, &[TOML_DATETIME]));
            }
            text = Some(map.next_value::<String>()?);
        }
        let text = text.ok_or_else(|| de::Error::missing_field(TOML_DATETIME))?;
        text.parse().map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(DateVisitor)
    }
}

/// What went wrong turning a document's days and dates into days.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CalendarError {
    #[error("{0}")]
    InvalidDate(String),
    #[error("{0} is a date, but there is no start date")]
    NoStartDate(Date),
    /// A conversion of a `#[days]` field's own making.
    #[error("{0}")]
    Custom(String),
}

/// A day field as a document writes it: days from day 0, or a date on the
/// calendar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Day {
    DaysFromStart(f64),
    CalendarDate(Date),
}

impl Default for Day {
    fn default() -> Self {
        Self::DaysFromStart(0.0)
    }
}

impl Day {
    /// Days from day 0, given `start`, the date of day 0. A date runs
    /// `through` its day when the field is the end of a span, so that it
    /// covers the whole day.
    ///
    /// # Errors
    /// A date with no start date.
    pub fn resolve(self, start: Option<Date>, through: bool) -> Result<f64, CalendarError> {
        match self {
            Self::DaysFromStart(days) => Ok(days),
            Self::CalendarDate(date) => {
                let start = start.ok_or(CalendarError::NoStartDate(date))?;
                #[allow(clippy::cast_precision_loss)]
                Ok(start.days_until(date) as f64 + f64::from(through))
            }
        }
    }

    /// `days` as a date when there is a start date and it is a whole day,
    /// else as days.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn from_days(days: f64, start: Option<Date>, through: bool) -> Self {
        if let Some(start) = start {
            let d = days - f64::from(through);
            if d.fract() == 0.0 && d.abs() < 1e15 {
                return Self::CalendarDate(start.plus_days(d as i64));
            }
        }
        Self::DaysFromStart(days)
    }
}

impl Serialize for Day {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::DaysFromStart(days) => serializer.serialize_f64(*days),
            Self::CalendarDate(date) => date.serialize(serializer),
        }
    }
}

struct DayVisitor;

impl<'de> Visitor<'de> for DayVisitor {
    type Value = Day;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("days, or a date YYYY-MM-DD")
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Day, E> {
        Ok(Day::DaysFromStart(v))
    }

    #[allow(clippy::cast_precision_loss)]
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Day, E> {
        Ok(Day::DaysFromStart(v as f64))
    }

    #[allow(clippy::cast_precision_loss)]
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Day, E> {
        Ok(Day::DaysFromStart(v as f64))
    }

    fn visit_str<E: de::Error>(self, s: &str) -> Result<Day, E> {
        DateVisitor.visit_str(s).map(Day::CalendarDate)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Day, A::Error> {
        DateVisitor.visit_map(map).map(Day::CalendarDate)
    }
}

impl<'de> Deserialize<'de> for Day {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(DayVisitor)
    }
}

/// A type with calendar day fields: in the model days from day 0, on the
/// wire ([`Calendar::Wire`]) days or dates. `#[calendar]` implements it for
/// a struct; a `Vec` or `Option` of one converts element by element.
pub trait Calendar: Sized {
    /// The same type with [`Day`] for each calendar day field.
    type Wire;

    /// The model form of `wire`, given `start`, the date of day 0.
    ///
    /// # Errors
    /// A date with no start date, or whatever a field's own conversion
    /// reports.
    fn from_calendar(wire: Self::Wire, start: Option<Date>) -> Result<Self, CalendarError>;

    /// The wire form: whole days as dates when there is a start date.
    fn to_calendar(&self, start: Option<Date>) -> Self::Wire;
}

impl<T: Calendar> Calendar for Vec<T> {
    type Wire = Vec<T::Wire>;

    fn from_calendar(wire: Self::Wire, start: Option<Date>) -> Result<Self, CalendarError> {
        wire.into_iter()
            .map(|w| T::from_calendar(w, start))
            .collect()
    }

    fn to_calendar(&self, start: Option<Date>) -> Self::Wire {
        self.iter().map(|t| t.to_calendar(start)).collect()
    }
}

impl<T: Calendar> Calendar for Option<T> {
    type Wire = Option<T::Wire>;

    fn from_calendar(wire: Self::Wire, start: Option<Date>) -> Result<Self, CalendarError> {
        wire.map(|w| T::from_calendar(w, start)).transpose()
    }

    fn to_calendar(&self, start: Option<Date>) -> Self::Wire {
        self.as_ref().map(|t| t.to_calendar(start))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn dates_round_trip_through_days() {
        let date: Date = "2026-05-14".parse().unwrap();
        assert_eq!(date.to_string(), "2026-05-14");
        assert_eq!(date.plus_days(83).to_string(), "2026-08-05");
        assert_eq!(date.plus_days(-38).to_string(), "2026-04-06");
        assert_eq!(date.days_until("2026-10-06".parse().unwrap()), 145);
        assert_eq!(Date::new(1970, 1, 1).unwrap().days_since_epoch(), 0);
        assert_eq!(Date::new(2000, 2, 29).unwrap().plus_days(1).month, 3);
        for days in [-1_000_000, -1, 0, 1, 59, 60, 365, 1_000_000] {
            let d = Date::from_days_since_epoch(days);
            assert_eq!(d.days_since_epoch(), days, "{d}");
        }
    }

    #[test]
    fn invalid_dates_are_rejected() {
        for text in [
            "2026-02-30",
            "2026-13-01",
            "2026-5-14",
            "26-05-14",
            "2026-05-14T00:00",
            "May 14",
        ] {
            assert!(text.parse::<Date>().is_err(), "{text}");
        }
        assert!(Date::new(2024, 2, 29).is_ok());
        assert!(Date::new(2023, 2, 29).is_err());
        let err = "2026-02-30".parse::<Date>().unwrap_err().to_string();
        assert!(err.contains("day 30 is not 1 to 28"), "{err}");
    }

    #[test]
    fn a_day_reads_days_a_date_or_a_native_toml_date() {
        let start: Date = "2026-05-14".parse().unwrap();
        let days: Day = serde_json::from_value(json!(2.5)).unwrap();
        assert_eq!(days, Day::DaysFromStart(2.5));
        assert_eq!(days.resolve(None, false).unwrap(), 2.5);
        let date: Day = serde_json::from_value(json!("2026-06-01")).unwrap();
        assert_eq!(date.resolve(Some(start), false).unwrap(), 18.0);
        assert_eq!(date.resolve(Some(start), true).unwrap(), 19.0);
        assert_eq!(
            date.resolve(None, false).unwrap_err().to_string(),
            "2026-06-01 is a date, but there is no start date"
        );
        let toml: Day = serde_json::from_value(json!({TOML_DATETIME: "2026-06-01"})).unwrap();
        assert_eq!(toml, date);
        let err = serde_json::from_value::<Day>(json!("2026-02-30"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("day 30 is not 1 to 28"), "{err}");

        assert_eq!(Day::from_days(18.0, Some(start), false), date);
        assert_eq!(Day::from_days(19.0, Some(start), true), date);
        assert_eq!(Day::from_days(2.5, Some(start), false), days);
        assert_eq!(Day::from_days(18.0, None, false), Day::DaysFromStart(18.0));
        assert_eq!(serde_json::to_value(date).unwrap(), json!("2026-06-01"));
        assert_eq!(serde_json::to_value(days).unwrap(), json!(2.5));
    }
}
