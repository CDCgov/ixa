//! Calendar dates for parameters. Model times are `f64` days from day 0;
//! given a start date, a document may write a day field as a `YYYY-MM-DD`
//! date instead. The `#[calendar]` attribute marks those fields:
//!
//! ```
//! use ixa::calendar;
//! use ixa::calendar::Date;
//! use serde::{Deserialize, Serialize};
//!
//! #[calendar]
//! #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
//! pub struct Policy {
//!     #[day]
//!     pub start: f64,
//!     #[day]
//!     pub end: Option<f64>,
//!     /// A duration, not marked.
//!     pub delay: f64,
//! }
//!
//! #[calendar]
//! #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
//! pub struct Parameters {
//!     #[reference]
//!     pub start_date: Option<Date>,
//!     #[days]
//!     pub policies: Vec<Policy>,
//!     #[day(inclusive)]
//!     pub max_time: f64,
//! }
//!
//! let p: Parameters = serde_json::from_str(r#"{
//!     "start_date": "2026-05-14",
//!     "policies": [{"start": "2026-06-01", "end": 48, "delay": 3}],
//!     "max_time": "2026-08-05"
//! }"#).unwrap();
//! assert_eq!(p.policies[0].start, 18.0);
//! assert_eq!(p.policies[0].end, Some(48.0));
//! assert_eq!(p.max_time, 84.0);
//!
//! assert!(serde_json::to_string(&p).unwrap().contains(r#""start":18.0"#));
//! assert!(serde_json::to_string(&p.dated()).unwrap().contains(r#""start":"2026-06-01""#));
//! ```
//!
//! `#[calendar]` generates a twin struct (`CalendarPolicy` for `Policy`)
//! with [`Day`] in each `#[day]` field and the twin of each `#[days]`
//! type, and implements [`Calendar`] between the two. The struct with the
//! `#[reference]` start date (`Date` or `Option<Date>`) is the root: it
//! deserializes through its twin, serializes days, and has `dated()`. A
//! nested root falls back to the outer start date. `#[day(inclusive)]` marks
//! a day a date includes, such as the end of a run.
//!
//! The twin copies the struct's derives and serde attributes, so
//! `#[calendar]` goes above `#[derive(..)]`. Limits: derive `Clone`,
//! `Debug`, `PartialEq`, `Serialize` and `Deserialize`, and not
//! `PartialOrd` (which [`Day`] lacks); no generics; no serde `try_from`, `from`, `into` or `remote`,
//! and no `with`, `deserialize_with`, `serialize_with` or `default = ".."`
//! on a `#[day]` field. A struct with its own wire shape implements
//! [`Calendar`] by hand.
//!
//! In code, [`Date::start_of`] and [`Date::end_of`] turn a date string into
//! days from a start date. [`ContextCalendarExt`] stores the start date on
//! the context so reports can write the current date; [`Date`] serializes
//! as `YYYY-MM-DD`.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::{define_data_plugin, Context, ContextBase};

/// Days since 1970-01-01 of 0000-01-01 and 9999-12-31.
const MIN_DAYS: i64 = -719_528;
const MAX_DAYS: i64 = 2_932_896;

/// A calendar date, written and read as `YYYY-MM-DD`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: i32,
    month: u32,
    day: u32,
}

impl Date {
    /// A checked date, with a four-digit year.
    ///
    /// # Errors
    /// When the year, month or day is out of range.
    pub fn new(year: i32, month: u32, day: u32) -> Result<Self, CalendarError> {
        if !(0..=9999).contains(&year) {
            return Err(CalendarError::InvalidDate(format!(
                "year {year} is not 0 to 9999"
            )));
        }
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

    #[must_use]
    pub fn year(self) -> i32 {
        self.year
    }

    #[must_use]
    pub fn month(self) -> u32 {
        self.month
    }

    #[must_use]
    pub fn day(self) -> u32 {
        self.day
    }

    /// The date `days` after this one (before it if negative), or `None`
    /// outside years 0 to 9999.
    #[must_use]
    pub fn checked_plus_days(self, days: i64) -> Option<Self> {
        self.days_since_epoch()
            .checked_add(days)
            .filter(|d| (MIN_DAYS..=MAX_DAYS).contains(d))
            .map(Self::from_days_since_epoch)
    }

    /// The date that `time` falls on, with this date as day 0, or `None`
    /// outside years 0 to 9999.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn date_at(self, time: f64) -> Option<Self> {
        if !time.is_finite() {
            return None;
        }
        self.checked_plus_days(time.floor() as i64)
    }

    /// Days from this date to `other`, negative if `other` is earlier.
    #[must_use]
    pub fn days_until(self, other: Date) -> i64 {
        other.days_since_epoch() - self.days_since_epoch()
    }

    /// The start of the date in `s`, in days from this date, for dates
    /// written in code: `start.start_of("2026-06-01")`.
    ///
    /// # Panics
    /// When `s` is not a valid `YYYY-MM-DD` date.
    #[must_use]
    #[track_caller]
    #[allow(clippy::cast_precision_loss)]
    pub fn start_of(self, s: &str) -> f64 {
        match s.parse::<Date>() {
            Ok(date) => self.days_until(date) as f64,
            Err(e) => panic!("{e}"),
        }
    }

    /// The end of the date in `s`, [`Date::start_of`] plus one, for an
    /// inclusive bound such as the end of a run.
    ///
    /// # Panics
    /// When `s` is not a valid `YYYY-MM-DD` date.
    #[must_use]
    #[track_caller]
    pub fn end_of(self, s: &str) -> f64 {
        self.start_of(s) + 1.0
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
                .filter(|p| p.len() == len && p.bytes().all(|b| b.is_ascii_digit()))
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

/// The key of the map the `toml` crate hands over for a native TOML date.
const TOML_DATETIME: &str = "$__toml_private_datetime";

/// `YYYY-MM-DD`, or a native TOML date.
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
        if deserializer.is_human_readable() {
            deserializer.deserialize_any(DateVisitor)
        } else {
            deserializer.deserialize_str(DateVisitor)
        }
    }
}

/// An error reading a date or converting one to days.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CalendarError {
    #[error("{0}")]
    InvalidDate(String),
    #[error("{0} is a date, but there is no start date")]
    NoStartDate(Date),
    /// From a hand-written [`Calendar`] impl.
    #[error("{0}")]
    Custom(String),
    /// An error in a field, such as `policies[0].start`.
    #[error("{path}: {error}")]
    At {
        path: String,
        error: Box<CalendarError>,
    },
}

impl CalendarError {
    /// The error inside field `segment` (or index `[i]`) of a struct.
    #[must_use]
    pub fn at(self, segment: &str) -> Self {
        match self {
            Self::At { path, error } => {
                let dot = if path.starts_with('[') { "" } else { "." };
                Self::At {
                    path: format!("{segment}{dot}{path}"),
                    error,
                }
            }
            error => Self::At {
                path: segment.to_owned(),
                error: Box::new(error),
            },
        }
    }
}

/// A day field as a document writes it.
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
    /// Days from `start`, the date of day 0; with `inclusive`, a date
    /// covers its whole day.
    ///
    /// # Errors
    /// A date with no start date.
    pub fn resolve(self, start: Option<Date>, inclusive: bool) -> Result<f64, CalendarError> {
        match self {
            Self::DaysFromStart(days) => Ok(days),
            Self::CalendarDate(date) => {
                let start = start.ok_or(CalendarError::NoStartDate(date))?;
                #[allow(clippy::cast_precision_loss)]
                Ok(start.days_until(date) as f64 + f64::from(inclusive))
            }
        }
    }

    /// A date if there is a start date and `days` is a whole day in range,
    /// else days.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn from_days(days: f64, start: Option<Date>, inclusive: bool) -> Self {
        let d = days - f64::from(inclusive);
        start
            .filter(|_| d.fract() == 0.0)
            .and_then(|start| start.checked_plus_days(d as i64))
            .map_or(Self::DaysFromStart(days), Self::CalendarDate)
    }
}

/// Tagged form of [`Day`] for binary formats.
#[derive(Serialize, Deserialize)]
#[serde(remote = "Day")]
enum BinaryDay {
    DaysFromStart(f64),
    CalendarDate(Date),
}

impl Serialize for Day {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            _ if !serializer.is_human_readable() => BinaryDay::serialize(self, serializer),
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
        if deserializer.is_human_readable() {
            deserializer.deserialize_any(DayVisitor)
        } else {
            BinaryDay::deserialize(deserializer)
        }
    }
}

/// A type with day fields and a wire form that may hold dates.
/// `#[calendar]` implements it for a struct.
pub trait Calendar: Sized {
    /// The same type with [`Day`] for each day field.
    type Wire;

    /// `start` is the enclosing root's start date; a struct with its own
    /// `#[reference]` uses that instead when it has one.
    ///
    /// # Errors
    /// A date with no start date, or a field's own conversion error.
    fn from_calendar(wire: Self::Wire, start: Option<Date>) -> Result<Self, CalendarError>;

    /// Whole days as dates when there is a start date.
    fn to_calendar(&self, start: Option<Date>) -> Self::Wire;
}

impl<T: Calendar> Calendar for Vec<T> {
    type Wire = Vec<T::Wire>;

    fn from_calendar(wire: Self::Wire, start: Option<Date>) -> Result<Self, CalendarError> {
        wire.into_iter()
            .enumerate()
            .map(|(i, w)| T::from_calendar(w, start).map_err(|e| e.at(&format!("[{i}]"))))
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

define_data_plugin!(StartDatePlugin, Option<Date>, None);

/// The date of day 0 on a [`Context`], for writing dates in reports.
pub trait ContextCalendarExt: ContextBase {
    /// Sets the date of day 0; takes a `Date` or a parameters' `Option<Date>`.
    fn set_start_date(&mut self, start: impl Into<Option<Date>>) {
        *self.get_data_mut(StartDatePlugin) = start.into();
    }

    fn get_start_date(&self) -> Option<Date> {
        *self.get_data(StartDatePlugin)
    }

    /// The date of the current time, if there is a start date.
    fn get_current_date(&self) -> Option<Date> {
        self.get_start_date()?.date_at(self.get_current_time())
    }
}

impl ContextCalendarExt for Context {}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn dates_round_trip_through_days() {
        let date: Date = "2026-05-14".parse().unwrap();
        assert_eq!(date.to_string(), "2026-05-14");
        assert_eq!(
            date.checked_plus_days(83).unwrap().to_string(),
            "2026-08-05"
        );
        assert_eq!(
            date.checked_plus_days(-38).unwrap().to_string(),
            "2026-04-06"
        );
        assert_eq!(date.days_until("2026-10-06".parse().unwrap()), 145);
        assert_eq!(Date::new(1970, 1, 1).unwrap().days_since_epoch(), 0);
        let leap = Date::new(2000, 2, 29).unwrap();
        assert_eq!(leap.checked_plus_days(1).unwrap().month(), 3);
    }

    /// Every day in range maps to the next valid date and back.
    #[test]
    fn every_day_in_range_is_the_next_valid_date() {
        let mut previous = Date::new(0, 1, 1).unwrap();
        assert_eq!(previous.days_since_epoch(), MIN_DAYS);
        for days in MIN_DAYS + 1..=MAX_DAYS {
            let date = Date::from_days_since_epoch(days);
            assert_eq!(Date::new(date.year, date.month, date.day), Ok(date));
            assert!(previous < date, "{previous} {date}");
            assert_eq!(date.days_since_epoch(), days, "{date}");
            previous = date;
        }
        assert_eq!(previous, Date::new(9999, 12, 31).unwrap());
    }

    #[test]
    fn a_date_string_in_code_gives_days_from_a_start() {
        let start: Date = "2026-05-14".parse().unwrap();
        assert_eq!(start.start_of("2026-06-01"), 18.0);
        assert_eq!(start.start_of("2026-05-01"), -13.0);
        assert_eq!(start.end_of("2026-08-05"), 84.0);
    }

    #[test]
    #[should_panic(expected = "day 30 is not 1 to 28")]
    fn a_bad_date_string_in_code_panics_with_the_reason() {
        let _ = Date::new(2026, 5, 14).unwrap().start_of("2026-02-30");
    }

    #[test]
    fn dates_stay_in_four_digit_years() {
        let (min, max) = (
            Date::new(0, 1, 1).unwrap(),
            Date::new(9999, 12, 31).unwrap(),
        );
        assert_eq!(min.checked_plus_days(-1), None);
        assert_eq!(max.checked_plus_days(1), None);
        assert_eq!(min.checked_plus_days(MAX_DAYS - MIN_DAYS), Some(max));
        assert_eq!(min.checked_plus_days(i64::MAX), None);
        assert_eq!(max.checked_plus_days(i64::MIN), None);
    }

    #[test]
    fn a_time_falls_on_the_date_of_its_whole_day() {
        let start: Date = "2026-05-14".parse().unwrap();
        let date = |t: f64| start.date_at(t).map(|d| d.to_string());
        assert_eq!(date(0.0).as_deref(), Some("2026-05-14"));
        assert_eq!(date(18.99).as_deref(), Some("2026-06-01"));
        assert_eq!(date(-0.5).as_deref(), Some("2026-05-13"));
        assert_eq!(date(1e7), None);
        assert_eq!(date(f64::NAN), None);
        assert_eq!(date(f64::NEG_INFINITY), None);
    }

    #[test]
    fn the_context_gives_the_date_of_the_current_time() {
        let mut context = Context::new();
        assert_eq!(context.get_current_date(), None);
        context.set_start_date(Some("2026-05-14".parse().unwrap()));
        context.add_plan(18.5, |context| {
            assert_eq!(
                context.get_current_date().unwrap().to_string(),
                "2026-06-01"
            );
            context.set_start_date(None);
            assert_eq!(context.get_current_date(), None);
        });
        context.execute();

        // As a report column.
        let mut csv = csv::Writer::from_writer(Vec::new());
        csv.serialize(("2026-06-01".parse::<Date>().unwrap(), 3))
            .unwrap();
        assert_eq!(csv.into_inner().unwrap(), b"2026-06-01,3\n");
    }

    #[test]
    fn dates_and_days_round_trip_through_bincode() {
        let config = bincode_next::config::standard();
        let date: Date = "2026-05-14".parse().unwrap();
        for day in [Day::DaysFromStart(2.5), Day::CalendarDate(date)] {
            let bytes = bincode_next::serde::encode_to_vec(day, config).unwrap();
            let (back, _): (Day, _) =
                bincode_next::serde::decode_from_slice(&bytes, config).unwrap();
            assert_eq!(back, day);
        }
        let bytes = bincode_next::serde::encode_to_vec(date, config).unwrap();
        let (back, _): (Date, _) = bincode_next::serde::decode_from_slice(&bytes, config).unwrap();
        assert_eq!(back, date);
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
            "2026-+5-14",
            "+999-05-14",
            "+2026-05-14",
            "20260514",
            " 2026-05-14",
            "2026-05-14\n",
            "10000-01-01",
            "",
        ] {
            assert!(text.parse::<Date>().is_err(), "{text}");
        }
        assert!(Date::new(2024, 2, 29).is_ok());
        assert!(Date::new(2023, 2, 29).is_err());
        assert!(Date::new(1900, 2, 29).is_err());
        assert!(Date::new(0, 2, 29).is_ok());
        assert!(Date::new(-1, 1, 1).is_err());
        assert!(Date::new(10000, 1, 1).is_err());
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
        assert_eq!(
            Day::from_days(1e12, Some(start), false),
            Day::DaysFromStart(1e12)
        );
        assert_eq!(
            Day::from_days(3e6, Some(start), false),
            Day::DaysFromStart(3e6)
        );
        assert!(matches!(
            Day::from_days(f64::NAN, Some(start), false),
            Day::DaysFromStart(d) if d.is_nan()
        ));
        assert_eq!(serde_json::to_value(date).unwrap(), json!("2026-06-01"));
        assert_eq!(serde_json::to_value(days).unwrap(), json!(2.5));
    }
}
