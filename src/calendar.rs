//! Calendar dates for parameters. Model times are `f64` days from day 0;
//! given a start date, a document may write a day as a `YYYY-MM-DD` date,
//! a `YYYY-Www` week, or days with units such as `2w`. The `#[calendar]`
//! attribute marks those fields:
//!
//! ```
//! use ixa::calendar;
//! use ixa::calendar::Date;
//! use serde::{Deserialize, Serialize};
//!
//! #[calendar]
//! #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
//! pub struct Policy {
//!     #[calendar]
//!     pub start: f64,
//!     #[calendar]
//!     pub end: Option<f64>,
//!     #[calendar(duration)]
//!     pub delay: f64,
//! }
//!
//! #[calendar(weeks = "epi")]
//! #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
//! pub struct Parameters {
//!     #[calendar(reference)]
//!     pub start_date: Option<Date>,
//!     #[calendar]
//!     pub policies: Vec<Policy>,
//!     #[calendar(inclusive)]
//!     pub max_time: f64,
//! }
//!
//! let p: Parameters = serde_json::from_str(r#"{
//!     "start_date": "2026-05-14",
//!     "policies": [{"start": "2026-06-01", "end": "2026-W27", "delay": "2w"}],
//!     "max_time": "2026-08-05"
//! }"#).unwrap();
//! assert_eq!(p.policies[0].start, 18.0);
//! assert_eq!(p.policies[0].end, Some(52.0));
//! assert_eq!(p.policies[0].delay, 14.0);
//! assert_eq!(p.max_time, 84.0);
//!
//! assert!(serde_json::to_string(&p).unwrap().contains(r#""start":18.0"#));
//! assert!(serde_json::to_string(&p.dated()).unwrap().contains(r#""start":"2026-06-01""#));
//! ```
//!
//! `#[calendar]` generates a twin struct (`CalendarPolicy` for `Policy`)
//! holding each marked field's [`Calendar::Wire`] ([`Day`] for an `f64`),
//! and implements [`Calendar`] between the two. Field options:
//! `#[calendar(reference)]` marks the root's start date (`Date` or
//! `Option<Date>`); `inclusive` is for a day a date or week includes, such
//! as the end of a run; `duration` is for a length of time, which takes
//! units but no dates and is not moved by [`Calendar::shift_days`];
//! `signed` lets a duration be negative; `week` makes `dated()` write a
//! week where the day starts one. `#[calendar(weeks = "iso")]` or
//! `weeks = "epi"` on a struct picks the [`WeekSystem`] for it and the
//! structs inside it; a week in a document without one is an error.
//!
//! Every calendar struct deserializes through its twin and serializes days.
//! The struct with the start date is the root and has `dated()`; a struct
//! read without one takes numbers and units but no dates or weeks. A nested
//! root falls back to the outer start date. Dates are for human-readable
//! formats; a format that is not (by serde's `is_human_readable`) reads and
//! writes the struct as written, days only, so it round trips through any
//! serde format. Every field type must then be `Deserialize`, including types
//! that implement [`Calendar`] by hand.
//!
//! The twin copies the struct's derives and serde attributes, so
//! `#[calendar]` goes above `#[derive(..)]`. Limits: derive `Clone`,
//! `Debug`, `PartialEq`, `Serialize` and `Deserialize`; no generics; no serde `try_from`, `from`, `into` or `remote`,
//! and no `with`, `deserialize_with`, `serialize_with` or `default = ".."`
//! on a marked field. A struct with its own wire shape implements
//! [`Calendar`] by hand. To validate a document, wrap the root in a type
//! whose `Deserialize` reads it and checks the result.
//!
//! In code, [`Date::start_of`] and [`Date::start_of_week`] turn a date or
//! week string into days from a start date. [`ContextCalendarExt`] stores
//! the start date and week system on the context so reports can write the
//! current [`Date`], [`Week`] and [`Weekday`], which serialize as
//! `YYYY-MM-DD`, `YYYY-Www` and `Mon`.
//!
//! Field options on a field that does not hold days are a compile error:
//!
//! ```compile_fail
//! use ixa::calendar;
//! use serde::{Deserialize, Serialize};
//!
//! #[calendar]
//! #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
//! struct Window { #[calendar] from: f64 }
//!
//! #[calendar]
//! #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
//! struct Outer { #[calendar(inclusive)] windows: Vec<Window> }
//! ```

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

/// Days since 1970-01-01 of a proleptic Gregorian date, negative before it.
fn civil_to_days(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
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
        civil_to_days(
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        )
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

    #[must_use]
    pub fn weekday(self) -> Weekday {
        Weekday::ALL[WeekSystem::Iso.offset(self.days_since_epoch()) as usize]
    }

    /// The week this date falls in; near New Year it may belong to the
    /// year before or after. `None` for the first days of year 0, whose
    /// week belongs to year -1.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn week(self, system: WeekSystem) -> Option<Week> {
        let days = self.days_since_epoch();
        let first = days - system.offset(days);
        // A week belongs to the year of its fourth day.
        let year = Self::from_days_since_epoch(first + 3).year;
        let week = (first - system.week_one(i64::from(year))) / 7 + 1;
        Week::new(year, week as u32).ok()
    }

    /// The start of the week in `s`, in days from this date, for weeks
    /// written in code: `start.start_of_week("2026-W12", WeekSystem::Epi)`.
    ///
    /// # Panics
    /// When `s` is not a valid `YYYY-Www` week in `system`.
    #[must_use]
    #[track_caller]
    #[allow(clippy::cast_precision_loss)]
    pub fn start_of_week(self, s: &str, system: WeekSystem) -> f64 {
        match s.parse::<Week>().and_then(|week| week.first_day(system)) {
            Ok(date) => self.days_until(date) as f64,
            Err(e) => panic!("{e}"),
        }
    }

    /// The end of the week in `s`, [`Date::start_of_week`] plus seven.
    ///
    /// # Panics
    /// When `s` is not a valid `YYYY-Www` week in `system`.
    #[must_use]
    #[track_caller]
    pub fn end_of_week(self, s: &str, system: WeekSystem) -> f64 {
        self.start_of_week(s, system) + 7.0
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

/// A day of the week, written as `Mon` to `Sun`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    const ALL: [Self; 7] = [
        Self::Monday,
        Self::Tuesday,
        Self::Wednesday,
        Self::Thursday,
        Self::Friday,
        Self::Saturday,
        Self::Sunday,
    ];

    fn abbreviation(self) -> &'static str {
        match self {
            Self::Monday => "Mon",
            Self::Tuesday => "Tue",
            Self::Wednesday => "Wed",
            Self::Thursday => "Thu",
            Self::Friday => "Fri",
            Self::Saturday => "Sat",
            Self::Sunday => "Sun",
        }
    }
}

impl fmt::Display for Weekday {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.abbreviation())
    }
}

impl FromStr for Weekday {
    type Err = CalendarError;

    fn from_str(s: &str) -> Result<Self, CalendarError> {
        Self::ALL
            .into_iter()
            .find(|day| day.abbreviation() == s)
            .ok_or_else(|| {
                CalendarError::InvalidWeekday(format!("{s:?} is not a weekday (Mon to Sun)"))
            })
    }
}

impl Serialize for Weekday {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Weekday {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
        s.parse().map_err(de::Error::custom)
    }
}

/// How weeks are numbered. Both number from the week holding January 4
/// (the first with four days in the year), so a week near New Year may
/// belong to the year before or after.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WeekSystem {
    /// ISO 8601: weeks start on Monday.
    Iso,
    /// CDC epidemiological (MMWR) weeks: weeks start on Sunday.
    Epi,
}

impl WeekSystem {
    /// Days since the start of the week of the day `days` since 1970-01-01.
    fn offset(self, days: i64) -> i64 {
        // 1970-01-01 was a Thursday.
        match self {
            Self::Iso => (days + 3).rem_euclid(7),
            Self::Epi => (days + 4).rem_euclid(7),
        }
    }

    /// Days since 1970-01-01 of the first day of week 1 of `year`.
    fn week_one(self, year: i64) -> i64 {
        let jan_4 = civil_to_days(year, 1, 4);
        jan_4 - self.offset(jan_4)
    }

    /// 52 or 53.
    #[must_use]
    pub fn weeks_in(self, year: i32) -> u32 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let weeks =
            ((self.week_one(i64::from(year) + 1) - self.week_one(i64::from(year))) / 7) as u32;
        weeks
    }
}

impl fmt::Display for WeekSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Iso => "ISO",
            Self::Epi => "epi",
        })
    }
}

/// A numbered week of a year, written and read as `YYYY-Www`. Which days
/// it covers depends on the [`WeekSystem`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Week {
    year: i32,
    week: u32,
}

impl Week {
    /// A week 1 to 53 of a four-digit year; whether the year has a week 53
    /// is checked by [`Week::first_day`].
    ///
    /// # Errors
    /// When the year or week is out of range.
    pub fn new(year: i32, week: u32) -> Result<Self, CalendarError> {
        if !(0..=9999).contains(&year) {
            return Err(CalendarError::InvalidWeek(format!(
                "year {year} is not 0 to 9999"
            )));
        }
        if !(1..=53).contains(&week) {
            return Err(CalendarError::InvalidWeek(format!(
                "week {week} is not 1 to 53"
            )));
        }
        Ok(Self { year, week })
    }

    #[must_use]
    pub fn year(self) -> i32 {
        self.year
    }

    #[must_use]
    pub fn number(self) -> u32 {
        self.week
    }

    /// The Monday (ISO) or Sunday (epi) the week starts on.
    ///
    /// # Errors
    /// Week 53 of a year with 52 weeks, or a week outside years 0 to 9999.
    pub fn first_day(self, system: WeekSystem) -> Result<Date, CalendarError> {
        let weeks = system.weeks_in(self.year);
        if self.week > weeks {
            return Err(CalendarError::InvalidWeek(format!(
                "{self}: {} has {weeks} {system} weeks",
                self.year
            )));
        }
        let days = system.week_one(i64::from(self.year)) + 7 * i64::from(self.week - 1);
        if !(MIN_DAYS..=MAX_DAYS).contains(&days) {
            return Err(CalendarError::InvalidWeek(format!(
                "{self} starts outside years 0 to 9999"
            )));
        }
        Ok(Date::from_days_since_epoch(days))
    }
}

impl FromStr for Week {
    type Err = CalendarError;

    fn from_str(s: &str) -> Result<Self, CalendarError> {
        let invalid = || CalendarError::InvalidWeek(format!("{s:?} is not a week (YYYY-Www)"));
        let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
        let (year, week) = s.split_once("-W").ok_or_else(invalid)?;
        if year.len() != 4 || week.len() != 2 || !digits(year) || !digits(week) {
            return Err(invalid());
        }
        let (year, week) = (
            year.parse().map_err(|_| invalid())?,
            week.parse().map_err(|_| invalid())?,
        );
        Self::new(year, week).map_err(|e| CalendarError::InvalidWeek(format!("{s:?}: {e}")))
    }
}

impl fmt::Display for Week {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-W{:02}", self.year, self.week)
    }
}

impl Serialize for Week {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

struct WeekVisitor;

impl Visitor<'_> for WeekVisitor {
    type Value = Week;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a week, YYYY-Www")
    }

    fn visit_str<E: de::Error>(self, s: &str) -> Result<Week, E> {
        s.parse().map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for Week {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(WeekVisitor)
    }
}

/// An error reading a date or converting one to days.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum CalendarError {
    #[error("{0}")]
    InvalidDate(String),
    #[error("{0}")]
    InvalidWeek(String),
    #[error("{0}")]
    InvalidWeekday(String),
    /// Days with units that do not parse.
    #[error("{0}")]
    InvalidDays(String),
    /// A negative or NaN value in a `duration` field.
    #[error("a duration cannot be {}", if .0.is_nan() { "NaN".to_string() } else { format!("negative: {}", .0) })]
    InvalidDuration(f64),
    /// A date or week with no start date to count from.
    #[error("{0} is a {kind}, but there is no start date", kind = .0.kind())]
    NoStartDate(Day),
    #[error("{0} is a week, but no week system is set: add `weeks = \"iso\"` or `weeks = \"epi\"` to `#[calendar]`")]
    NoWeekSystem(Week),
    /// A date or week in a `duration` field.
    #[error("a duration cannot be the {kind} {0}", kind = .0.kind())]
    NotADuration(Day),
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

/// A day field as a document writes it. Days with units (`2w`, `3d`,
/// `12h`) read as [`Day::DaysFromStart`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Day {
    DaysFromStart(f64),
    CalendarDate(Date),
    CalendarWeek(Week),
}

impl Default for Day {
    fn default() -> Self {
        Self::DaysFromStart(0.0)
    }
}

/// Days compare with days, dates with dates and weeks with weeks; other
/// pairs are incomparable without a start date.
impl PartialOrd for Day {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Self::DaysFromStart(a), Self::DaysFromStart(b)) => a.partial_cmp(b),
            (Self::CalendarDate(a), Self::CalendarDate(b)) => a.partial_cmp(b),
            (Self::CalendarWeek(a), Self::CalendarWeek(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DaysFromStart(days) => write!(f, "{days}"),
            Self::CalendarDate(date) => write!(f, "{date}"),
            Self::CalendarWeek(week) => write!(f, "{week}"),
        }
    }
}

impl Day {
    fn kind(self) -> &'static str {
        match self {
            Self::DaysFromStart(_) => "number of days",
            Self::CalendarDate(_) => "date",
            Self::CalendarWeek(_) => "week",
        }
    }

    /// Days from the start date, read as `field` says: an `inclusive` date
    /// or week covers its whole span, and a `duration` takes no date or
    /// week.
    ///
    /// # Errors
    /// A date or week with no start date, a week with no week system or
    /// that its year does not have, or a date, week or negative value
    /// where a duration does not allow one.
    pub fn resolve(self, cx: CalendarContext, field: FieldOptions) -> Result<f64, CalendarError> {
        if field.duration {
            return match self {
                Self::DaysFromStart(days) if days.is_nan() || (!field.signed && days < 0.0) => {
                    Err(CalendarError::InvalidDuration(days))
                }
                Self::DaysFromStart(days) => Ok(days),
                _ => Err(CalendarError::NotADuration(self)),
            };
        }
        let start = || cx.start.ok_or(CalendarError::NoStartDate(self));
        #[allow(clippy::cast_precision_loss)]
        match self {
            Self::DaysFromStart(days) => Ok(days),
            Self::CalendarDate(date) => {
                Ok(start()?.days_until(date) as f64 + f64::from(field.inclusive))
            }
            Self::CalendarWeek(week) => {
                let system = cx.weeks.ok_or(CalendarError::NoWeekSystem(week))?;
                let start = start()?;
                let first = week.first_day(system)?;
                Ok(start.days_until(first) as f64 + if field.inclusive { 7.0 } else { 0.0 })
            }
        }
    }

    /// With a start date and `days` a whole day in range, a week if `field`
    /// asks for one and the day starts a week, else a date; days otherwise
    /// and for a duration.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn from_days(days: f64, cx: CalendarContext, field: FieldOptions) -> Self {
        let Some(start) = cx.start.filter(|_| !field.duration) else {
            return Self::DaysFromStart(days);
        };
        let whole_day = |d: f64| {
            (d.fract() == 0.0)
                .then(|| start.checked_plus_days(d as i64))
                .flatten()
        };
        if let Some(system) = cx.weeks.filter(|_| field.write_week) {
            let week =
                whole_day(days - if field.inclusive { 7.0 } else { 0.0 }).and_then(|first| {
                    first
                        .week(system)
                        .filter(|week| week.first_day(system) == Ok(first))
                });
            if let Some(week) = week {
                return Self::CalendarWeek(week);
            }
        }
        whole_day(days - f64::from(field.inclusive))
            .map_or(Self::DaysFromStart(days), Self::CalendarDate)
    }
}

/// A date `YYYY-MM-DD`, a week `YYYY-Www`, or days with units.
impl FromStr for Day {
    type Err = CalendarError;

    fn from_str(s: &str) -> Result<Self, CalendarError> {
        let b = s.as_bytes();
        if b.len() > 4 && b[4] == b'-' && b[..4].iter().all(u8::is_ascii_digit) {
            if b.get(5) == Some(&b'W') {
                s.parse().map(Self::CalendarWeek)
            } else {
                s.parse().map(Self::CalendarDate)
            }
        } else {
            parse_days(s).map(Self::DaysFromStart)
        }
    }
}

/// Days from a sign and terms like `2w 3d 12h`.
fn parse_days(s: &str) -> Result<f64, CalendarError> {
    let invalid = || {
        CalendarError::InvalidDays(format!(
            "{s:?} is not a date (YYYY-MM-DD), a week (YYYY-Www) or days with units (3d, 2w, 12h)"
        ))
    };
    let (sign, mut rest) = match s.strip_prefix('-') {
        Some(rest) => (-1.0, rest),
        None => (1.0, s),
    };
    let mut total = 0.0;
    let mut terms = 0;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        let end = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(rest.len());
        let value: f64 = rest[..end].parse().map_err(|_| invalid())?;
        rest = rest[end..].trim_start();
        let end = rest
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(rest.len());
        total += match &rest[..end] {
            "w" | "week" | "weeks" => value * 7.0,
            "d" | "day" | "days" => value,
            "h" | "hour" | "hours" => value / 24.0,
            "mo" | "month" | "months" | "y" | "year" | "years" => {
                return Err(CalendarError::InvalidDays(format!(
                    "{s:?}: months and years have no fixed length, so write a date instead"
                )))
            }
            _ => return Err(invalid()),
        };
        rest = &rest[end..];
        terms += 1;
    }
    if terms == 0 || !f64::is_finite(total) {
        return Err(invalid());
    }
    Ok(sign * total)
}

/// Tagged form of [`Day`] for binary formats.
#[derive(Serialize, Deserialize)]
#[serde(remote = "Day")]
enum BinaryDay {
    DaysFromStart(f64),
    CalendarDate(Date),
    CalendarWeek(Week),
}

impl Serialize for Day {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            _ if !serializer.is_human_readable() => BinaryDay::serialize(self, serializer),
            Self::DaysFromStart(days) => serializer.serialize_f64(*days),
            Self::CalendarDate(date) => date.serialize(serializer),
            Self::CalendarWeek(week) => week.serialize(serializer),
        }
    }
}

struct DayVisitor;

impl<'de> Visitor<'de> for DayVisitor {
    type Value = Day;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("days, a date YYYY-MM-DD, a week YYYY-Www, or days with units like 2w")
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
        s.parse().map_err(E::custom)
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

/// The enclosing start date and week system of a conversion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CalendarContext {
    pub start: Option<Date>,
    pub weeks: Option<WeekSystem>,
}

/// The `#[calendar(..)]` options of the field being converted; the default
/// is a plain point in time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FieldOptions {
    pub inclusive: bool,
    pub duration: bool,
    pub signed: bool,
    /// `week`: write a day that starts a week as the week.
    pub write_week: bool,
}

/// A type with day fields and a wire form that may hold dates.
/// `#[calendar]` implements it for a struct.
pub trait Calendar: Sized {
    /// The same type with [`Day`] for each day.
    type Wire;

    /// Whether the type holds days itself (`f64`, or an `Option` or `Vec`
    /// of one), so the field options `inclusive`, `duration`, `signed` and
    /// `week` apply to it.
    const DAYS: bool = false;

    /// `field` holds the options of the field this value is in, which only
    /// a type with [`Calendar::DAYS`] reads. A struct passes each of its
    /// fields their own options, and one with its own start date or week
    /// system puts those in `cx`.
    ///
    /// # Errors
    /// A date with no start date, or a field's own conversion error.
    fn from_calendar(
        wire: Self::Wire,
        cx: CalendarContext,
        field: FieldOptions,
    ) -> Result<Self, CalendarError>;

    /// Whole days as dates when there is a start date.
    fn to_calendar(&self, cx: CalendarContext, field: FieldOptions) -> Self::Wire;

    /// Moves every point in time `days` later; durations and start dates
    /// stay.
    fn shift_days(&mut self, days: f64);

    /// Moves every start date, including nested roots', `days` later.
    /// Paired with `shift_days(-days)` it moves day 0 and keeps every date.
    ///
    /// # Errors
    /// A start date moved outside years 0 to 9999; earlier ones have then
    /// already moved.
    fn shift_start_dates(&mut self, _days: i64) -> Result<(), CalendarError> {
        Ok(())
    }
}

/// A start date field, `Date` or `Option<Date>`, for `#[calendar]`.
#[doc(hidden)]
pub trait StartDate {
    /// # Errors
    /// The date moved outside years 0 to 9999.
    fn shift_start_date(&mut self, days: i64) -> Result<(), CalendarError>;
}

impl StartDate for Date {
    fn shift_start_date(&mut self, days: i64) -> Result<(), CalendarError> {
        *self = self.checked_plus_days(days).ok_or_else(|| {
            CalendarError::InvalidDate(format!(
                "{self} moved {days} days is outside years 0 to 9999"
            ))
        })?;
        Ok(())
    }
}

impl StartDate for Option<Date> {
    fn shift_start_date(&mut self, days: i64) -> Result<(), CalendarError> {
        self.as_mut()
            .map_or(Ok(()), |date| date.shift_start_date(days))
    }
}

impl Calendar for f64 {
    type Wire = Day;
    const DAYS: bool = true;

    fn from_calendar(
        wire: Day,
        cx: CalendarContext,
        field: FieldOptions,
    ) -> Result<Self, CalendarError> {
        wire.resolve(cx, field)
    }

    fn to_calendar(&self, cx: CalendarContext, field: FieldOptions) -> Day {
        Day::from_days(*self, cx, field)
    }

    fn shift_days(&mut self, days: f64) {
        *self += days;
    }
}

impl<T: Calendar> Calendar for Vec<T> {
    type Wire = Vec<T::Wire>;
    const DAYS: bool = T::DAYS;

    fn from_calendar(
        wire: Self::Wire,
        cx: CalendarContext,
        field: FieldOptions,
    ) -> Result<Self, CalendarError> {
        wire.into_iter()
            .enumerate()
            .map(|(i, w)| T::from_calendar(w, cx, field).map_err(|e| e.at(&format!("[{i}]"))))
            .collect()
    }

    fn to_calendar(&self, cx: CalendarContext, field: FieldOptions) -> Self::Wire {
        self.iter().map(|t| t.to_calendar(cx, field)).collect()
    }

    fn shift_days(&mut self, days: f64) {
        for t in self {
            t.shift_days(days);
        }
    }

    fn shift_start_dates(&mut self, days: i64) -> Result<(), CalendarError> {
        for (i, t) in self.iter_mut().enumerate() {
            t.shift_start_dates(days)
                .map_err(|e| e.at(&format!("[{i}]")))?;
        }
        Ok(())
    }
}

impl<T: Calendar> Calendar for Option<T> {
    type Wire = Option<T::Wire>;
    const DAYS: bool = T::DAYS;

    fn from_calendar(
        wire: Self::Wire,
        cx: CalendarContext,
        field: FieldOptions,
    ) -> Result<Self, CalendarError> {
        wire.map(|w| T::from_calendar(w, cx, field)).transpose()
    }

    fn to_calendar(&self, cx: CalendarContext, field: FieldOptions) -> Self::Wire {
        self.as_ref().map(|t| t.to_calendar(cx, field))
    }

    fn shift_days(&mut self, days: f64) {
        if let Some(t) = self {
            t.shift_days(days);
        }
    }

    fn shift_start_dates(&mut self, days: i64) -> Result<(), CalendarError> {
        self.as_mut().map_or(Ok(()), |t| t.shift_start_dates(days))
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct CalendarSettings {
    start: Option<Date>,
    weeks: Option<WeekSystem>,
}

define_data_plugin!(
    CalendarPlugin,
    CalendarSettings,
    CalendarSettings::default()
);

/// The date of day 0 and the week system on a [`Context`], for writing
/// dates and weeks in reports.
pub trait ContextCalendarExt: ContextBase {
    /// Sets the date of day 0; takes a `Date` or a parameters' `Option<Date>`.
    fn set_start_date(&mut self, start: impl Into<Option<Date>>) {
        self.get_data_mut(CalendarPlugin).start = start.into();
    }

    fn get_start_date(&self) -> Option<Date> {
        self.get_data(CalendarPlugin).start
    }

    /// Takes a `WeekSystem` or a root's `WEEK_SYSTEM`.
    fn set_week_system(&mut self, weeks: impl Into<Option<WeekSystem>>) {
        self.get_data_mut(CalendarPlugin).weeks = weeks.into();
    }

    fn get_week_system(&self) -> Option<WeekSystem> {
        self.get_data(CalendarPlugin).weeks
    }

    /// The date of the current time, if there is a start date.
    fn get_current_date(&self) -> Option<Date> {
        self.get_start_date()?.date_at(self.get_current_time())
    }

    /// The week of the current time, if there is a start date and a week
    /// system.
    fn get_current_week(&self) -> Option<Week> {
        self.get_current_date()?.week(self.get_week_system()?)
    }
}

impl ContextCalendarExt for Context {}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn cx(start: Option<Date>) -> CalendarContext {
        CalendarContext { start, weeks: None }
    }

    fn inclusive(inclusive: bool) -> FieldOptions {
        FieldOptions {
            inclusive,
            ..FieldOptions::default()
        }
    }

    fn date(s: &str) -> Date {
        s.parse().unwrap()
    }

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
        assert_eq!(days.resolve(cx(None), inclusive(false)).unwrap(), 2.5);
        let date: Day = serde_json::from_value(json!("2026-06-01")).unwrap();
        assert_eq!(
            date.resolve(cx(Some(start)), inclusive(false)).unwrap(),
            18.0
        );
        assert_eq!(
            date.resolve(cx(Some(start)), inclusive(true)).unwrap(),
            19.0
        );
        assert_eq!(
            date.resolve(cx(None), inclusive(false))
                .unwrap_err()
                .to_string(),
            "2026-06-01 is a date, but there is no start date"
        );
        let toml: Day = serde_json::from_value(json!({TOML_DATETIME: "2026-06-01"})).unwrap();
        assert_eq!(toml, date);
        let err = serde_json::from_value::<Day>(json!("2026-02-30"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("day 30 is not 1 to 28"), "{err}");

        assert_eq!(
            Day::from_days(18.0, cx(Some(start)), inclusive(false)),
            date
        );
        assert_eq!(Day::from_days(19.0, cx(Some(start)), inclusive(true)), date);
        assert_eq!(Day::from_days(2.5, cx(Some(start)), inclusive(false)), days);
        assert_eq!(
            Day::from_days(18.0, cx(None), inclusive(false)),
            Day::DaysFromStart(18.0)
        );
        assert_eq!(
            Day::from_days(1e12, cx(Some(start)), inclusive(false)),
            Day::DaysFromStart(1e12)
        );
        assert_eq!(
            Day::from_days(3e6, cx(Some(start)), inclusive(false)),
            Day::DaysFromStart(3e6)
        );
        assert!(matches!(
            Day::from_days(f64::NAN, cx(Some(start)), inclusive(false)),
            Day::DaysFromStart(d) if d.is_nan()
        ));
        assert_eq!(serde_json::to_value(date).unwrap(), json!("2026-06-01"));
        assert_eq!(serde_json::to_value(days).unwrap(), json!(2.5));

        assert!(days < Day::DaysFromStart(3.0));
        assert!(date < Day::CalendarDate(start.checked_plus_days(19).unwrap()));
        assert_eq!(days.partial_cmp(&date), None);
    }

    fn week(s: &str) -> Week {
        s.parse().unwrap()
    }

    #[test]
    fn weekdays() {
        assert_eq!(date("1970-01-01").weekday(), Weekday::Thursday);
        assert_eq!(date("2000-01-01").weekday(), Weekday::Saturday);
        assert_eq!(date("2026-10-20").weekday(), Weekday::Tuesday);
        assert_eq!(date("0000-01-01").weekday(), Weekday::Saturday);
        assert_eq!(serde_json::to_value(Weekday::Sunday).unwrap(), json!("Sun"));
        for day in Weekday::ALL {
            assert_eq!(
                serde_json::from_value::<Weekday>(json!(day.to_string())).unwrap(),
                day
            );
        }
        for text in ["sun", "Sunday", "", "Sun "] {
            assert!(text.parse::<Weekday>().is_err(), "{text}");
        }

        // A report row with a weekday column reads back.
        let mut csv = csv::Writer::from_writer(Vec::new());
        csv.serialize((Weekday::Tuesday, 3)).unwrap();
        let bytes = csv.into_inner().unwrap();
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(&bytes[..]);
        let row: (Weekday, u32) = reader.deserialize().next().unwrap().unwrap();
        assert_eq!(row, (Weekday::Tuesday, 3));
        let config = bincode_next::config::standard();
        let bytes = bincode_next::serde::encode_to_vec(Weekday::Friday, config).unwrap();
        let (back, _): (Weekday, _) =
            bincode_next::serde::decode_from_slice(&bytes, config).unwrap();
        assert_eq!(back, Weekday::Friday);
    }

    /// The weeks on either side of New Year, where ISO and epi weeks disagree.
    #[test]
    fn iso_and_epi_weeks_near_new_year() {
        for (day, iso, epi) in [
            ("2024-12-29", "2024-W52", "2025-W01"),
            ("2025-12-28", "2025-W52", "2025-W53"),
            ("2026-01-01", "2026-W01", "2025-W53"),
            ("2026-01-04", "2026-W01", "2026-W01"),
            ("2026-10-20", "2026-W43", "2026-W42"),
            ("2027-01-01", "2026-W53", "2026-W52"),
            ("2021-01-02", "2020-W53", "2020-W53"),
        ] {
            assert_eq!(date(day).week(WeekSystem::Iso), Some(week(iso)), "{day}");
            assert_eq!(date(day).week(WeekSystem::Epi), Some(week(epi)), "{day}");
        }
        assert_eq!(date("0000-01-01").week(WeekSystem::Iso), None);
        assert_eq!(
            date("0000-01-02").week(WeekSystem::Epi),
            Some(week("0000-W01"))
        );
        assert_eq!(WeekSystem::Iso.weeks_in(2026), 53);
        assert_eq!(WeekSystem::Epi.weeks_in(2026), 52);
        assert_eq!(WeekSystem::Epi.weeks_in(2025), 53);
        for year in [i32::MIN, i32::MAX] {
            assert!((52..=53).contains(&WeekSystem::Iso.weeks_in(year)));
        }
        assert_eq!(
            week("2025-W01").first_day(WeekSystem::Epi),
            Ok(date("2024-12-29"))
        );
        assert_eq!(
            week("2026-W53")
                .first_day(WeekSystem::Epi)
                .unwrap_err()
                .to_string(),
            "2026-W53: 2026 has 52 epi weeks"
        );
    }

    /// Every day falls in a week that starts on the right weekday at most
    /// six days before it, and weeks count up by one or restart at 1. Only
    /// the days of year 0 whose week belongs to year -1 have none.
    #[test]
    fn every_day_is_in_its_week() {
        for (system, weekday) in [
            (WeekSystem::Iso, Weekday::Monday),
            (WeekSystem::Epi, Weekday::Sunday),
        ] {
            let mut days = MIN_DAYS;
            while Date::from_days_since_epoch(days).week(system).is_none() {
                days += 1;
            }
            assert!(days - MIN_DAYS < 7, "{system}");
            let mut previous = Date::from_days_since_epoch(days).week(system).unwrap();
            for days in days..=MAX_DAYS {
                let date = Date::from_days_since_epoch(days);
                let week = date.week(system).unwrap();
                let first = week.first_day(system).unwrap();
                assert_eq!(first.weekday(), weekday, "{date}");
                assert!((0..7).contains(&first.days_until(date)), "{date} {week}");
                assert!(week.number() <= system.weeks_in(week.year()), "{date}");
                if week != previous {
                    let next = Week::new(previous.year, previous.week + 1);
                    let rolled = previous.week == system.weeks_in(previous.year)
                        && week == Week::new(previous.year + 1, 1).unwrap();
                    assert!(next == Ok(week) || rolled, "{previous} {week}");
                }
                previous = week;
            }
        }
    }

    #[test]
    fn weeks_parse_and_print() {
        assert_eq!(week("2026-W07").to_string(), "2026-W07");
        assert_eq!(
            (week("2026-W07").year(), week("2026-W07").number()),
            (2026, 7)
        );
        for text in [
            "2026-W7",
            "2026-W00",
            "2026-W54",
            "2026W07",
            "26-W07",
            "2026-w07",
            "2026-W07 ",
            "",
        ] {
            assert!(text.parse::<Week>().is_err(), "{text}");
        }
        let config = bincode_next::config::standard();
        let bytes = bincode_next::serde::encode_to_vec(week("2026-W07"), config).unwrap();
        let (back, _): (Week, _) = bincode_next::serde::decode_from_slice(&bytes, config).unwrap();
        assert_eq!(back, week("2026-W07"));
    }

    #[test]
    fn a_week_string_in_code_gives_days_from_a_start() {
        let start = date("2026-05-14");
        assert_eq!(start.start_of_week("2026-W27", WeekSystem::Epi), 52.0);
        assert_eq!(start.end_of_week("2026-W27", WeekSystem::Epi), 59.0);
        assert_eq!(start.start_of_week("2026-W27", WeekSystem::Iso), 46.0);
    }

    #[test]
    fn days_take_units() {
        let days = |s: &str| match s.parse::<Day>() {
            Ok(Day::DaysFromStart(d)) => Ok(d),
            Ok(other) => panic!("{s}: {other:?}"),
            Err(e) => Err(e.to_string()),
        };
        for (text, value) in [
            ("2w", 14.0),
            ("14d", 14.0),
            ("1w 2d", 9.0),
            ("1w2d", 9.0),
            ("36h", 1.5),
            ("0.5w", 3.5),
            ("1.5 days", 1.5),
            ("2 weeks", 14.0),
            ("-1w", -7.0),
            ("- 3d", -3.0),
        ] {
            assert_eq!(days(text), Ok(value), "{text}");
        }
        let huge = format!("{}w", "9".repeat(400));
        for text in ["3", "", "-", "w", "3x", "3d 2", "1.2.3d", "+3d", &huge] {
            assert!(
                days(text).unwrap_err().contains("days with units"),
                "{text}"
            );
        }
        for text in ["1month", "2y", "1 year"] {
            assert!(
                days(text).unwrap_err().contains("no fixed length"),
                "{text}"
            );
        }
        assert!(days("2026-5-14").unwrap_err().contains("is not a date"));
        assert_eq!(
            "2026-W12".parse::<Day>(),
            Ok(Day::CalendarWeek(week("2026-W12")))
        );
    }

    #[test]
    fn a_week_resolves_to_its_first_day_or_its_end() {
        let epi = CalendarContext {
            start: Some(date("2026-05-14")),
            weeks: Some(WeekSystem::Epi),
        };
        let point = FieldOptions::default();
        let w27 = Day::CalendarWeek(week("2026-W27"));
        assert_eq!(w27.resolve(epi, point), Ok(52.0));
        assert_eq!(w27.resolve(epi, inclusive(true)), Ok(59.0));
        assert_eq!(
            w27.resolve(CalendarContext { weeks: None, ..epi }, point).unwrap_err().to_string(),
            "2026-W27 is a week, but no week system is set: add `weeks = \"iso\"` or `weeks = \"epi\"` to `#[calendar]`"
        );
        let no_start = w27.resolve(CalendarContext { start: None, ..epi }, point);
        assert_eq!(no_start, Err(CalendarError::NoStartDate(w27)));
        assert_eq!(
            no_start.unwrap_err().to_string(),
            "2026-W27 is a week, but there is no start date"
        );
        assert!(Day::CalendarWeek(week("2026-W53"))
            .resolve(epi, point)
            .is_err());

        let weekly = FieldOptions {
            write_week: true,
            ..point
        };
        assert_eq!(Day::from_days(52.0, epi, weekly), w27);
        assert_eq!(
            Day::from_days(
                59.0,
                epi,
                FieldOptions {
                    inclusive: true,
                    ..weekly
                }
            ),
            w27
        );
        assert_eq!(
            Day::from_days(53.0, epi, weekly),
            Day::CalendarDate(date("2026-07-06"))
        );
        assert_eq!(
            Day::from_days(52.0, epi, point),
            Day::CalendarDate(date("2026-07-05"))
        );
        assert_eq!(serde_json::to_value(w27).unwrap(), json!("2026-W27"));
    }

    #[test]
    fn a_duration_takes_units_but_no_dates_or_weeks() {
        let cx = CalendarContext {
            start: Some(date("2026-05-14")),
            weeks: Some(WeekSystem::Iso),
        };
        let duration = FieldOptions {
            duration: true,
            ..FieldOptions::default()
        };
        assert_eq!("2w".parse::<Day>().unwrap().resolve(cx, duration), Ok(14.0));
        let june = Day::CalendarDate(date("2026-06-01"));
        assert_eq!(
            june.resolve(cx, duration),
            Err(CalendarError::NotADuration(june))
        );
        assert_eq!(
            june.resolve(cx, duration).unwrap_err().to_string(),
            "a duration cannot be the date 2026-06-01"
        );
        assert_eq!(
            Day::CalendarWeek(week("2026-W01"))
                .resolve(cx, duration)
                .unwrap_err()
                .to_string(),
            "a duration cannot be the week 2026-W01"
        );
        assert_eq!(
            Day::DaysFromStart(-3.0).resolve(cx, duration),
            Err(CalendarError::InvalidDuration(-3.0))
        );
        assert_eq!(
            CalendarError::InvalidDuration(-3.0).to_string(),
            "a duration cannot be negative: -3"
        );
        assert_eq!(
            CalendarError::InvalidDuration(f64::NAN).to_string(),
            "a duration cannot be NaN"
        );
        let signed = FieldOptions {
            signed: true,
            ..duration
        };
        assert_eq!(Day::DaysFromStart(-3.0).resolve(cx, signed), Ok(-3.0));
        for field in [duration, signed] {
            assert!(Day::DaysFromStart(f64::NAN).resolve(cx, field).is_err());
            assert_eq!(
                Day::DaysFromStart(f64::INFINITY).resolve(cx, field),
                Ok(f64::INFINITY)
            );
        }
        assert_eq!(Day::from_days(18.0, cx, duration), Day::DaysFromStart(18.0));
    }

    #[test]
    fn the_context_gives_the_week_of_the_current_time() {
        let mut context = Context::new();
        context.set_start_date(date("2026-05-14"));
        assert_eq!(context.get_current_week(), None);
        context.set_week_system(WeekSystem::Epi);
        assert_eq!(context.get_week_system(), Some(WeekSystem::Epi));
        context.add_plan(52.5, |context| {
            assert_eq!(context.get_current_week(), Some(week("2026-W27")));
        });
        context.execute();

        let mut csv = csv::Writer::from_writer(Vec::new());
        csv.serialize((week("2026-W27"), Weekday::Sunday, 3))
            .unwrap();
        assert_eq!(csv.into_inner().unwrap(), b"2026-W27,Sun,3\n");
    }
}
