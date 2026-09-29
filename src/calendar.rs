//! Calendar-date utilities for day-based simulations.
//!
//! A [`Calendar`] maps simulation time `0.0` to a Gregorian calendar date. One
//! simulation-time unit is one day, and fractional times map to their
//! containing day. For example, times from `0.0` through just under `1.0` map
//! to the epoch date, while `-0.5` maps to the preceding date.
//!
//! This module deals only with dates. It does not model time zones, daylight
//! saving time, or times of day.

use chrono::TimeDelta;
pub use chrono::{Datelike, Days, Months, NaiveDate as Date, Weekday};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{define_data_plugin, Context, ContextBase, PlanId};

/// Maps day-based simulation time to Gregorian calendar dates.
///
/// The epoch always corresponds to simulation time `0.0`, independently of
/// the context's configured start time.
///
/// # Examples
///
/// ```
/// use ixa::{Calendar, Date};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let calendar = Calendar::new("2025-01-01".parse::<Date>()?);
/// assert_eq!(calendar.date_at_time(1.5)?, "2025-01-02".parse()?);
/// assert_eq!(calendar.time_at_date("2025-01-08".parse()?), 7.0);
/// # Ok(())
/// # }
/// ```
#[must_use]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Calendar {
    epoch: Date,
}

impl Calendar {
    /// Creates a calendar whose `epoch` corresponds to simulation time `0.0`.
    pub const fn new(epoch: Date) -> Self {
        Self { epoch }
    }

    /// Returns the date corresponding to simulation time `0.0`.
    pub const fn epoch(self) -> Date {
        self.epoch
    }

    /// Returns the simulation time corresponding to `date`.
    ///
    /// The returned value is an integral number of days. Chrono's supported
    /// date range spans far fewer than `2^53` days, so the conversion to `f64`
    /// is exact.
    pub fn time_at_date(self, date: Date) -> f64 {
        date.signed_duration_since(self.epoch).num_days() as f64
    }

    /// Returns the date containing the supplied simulation `time`.
    ///
    /// Fractional times are rounded down. Thus `0.9` maps to the epoch and
    /// `-0.1` maps to the day before the epoch.
    ///
    /// # Errors
    ///
    /// Returns [`CalendarError::NonFiniteTime`] for NaN or infinite values and
    /// [`CalendarError::DateOutOfRange`] when the result would be outside
    /// Chrono's supported date range.
    pub fn date_at_time(self, time: f64) -> Result<Date, CalendarError> {
        if !time.is_finite() {
            return Err(CalendarError::NonFiniteTime { time });
        }

        let whole_days = time.floor();
        let earliest_day = self.time_at_date(Date::MIN);
        let latest_day = self.time_at_date(Date::MAX);
        if whole_days < earliest_day || whole_days > latest_day {
            return Err(CalendarError::DateOutOfRange { time });
        }

        // The bounds check above limits this value to Chrono's date range,
        // which is safely representable by i64.
        let whole_days = whole_days as i64;
        let delta =
            TimeDelta::try_days(whole_days).ok_or(CalendarError::DateOutOfRange { time })?;
        self.epoch
            .checked_add_signed(delta)
            .ok_or(CalendarError::DateOutOfRange { time })
    }
}

/// Errors produced while configuring or using a calendar.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Error, PartialEq)]
pub enum CalendarError {
    /// A calendar has already been configured for this context.
    #[error("calendar is already configured")]
    AlreadyConfigured,
    /// No calendar has been configured for this context.
    #[error("calendar is not configured")]
    NotConfigured,
    /// Simulation time must be finite.
    #[error("simulation time must be finite, got {time}")]
    NonFiniteTime {
        /// The invalid simulation time.
        time: f64,
    },
    /// The simulation time cannot be represented as a supported date.
    #[error("simulation time {time} is outside the supported calendar date range")]
    DateOutOfRange {
        /// The simulation time that exceeded the date range.
        time: f64,
    },
}

define_data_plugin!(CalendarPlugin, Option<Calendar>, None);

/// Calendar operations for [`Context`].
pub trait ContextCalendarExt: ContextBase {
    /// Configures the calendar for this context.
    ///
    /// # Errors
    ///
    /// Returns [`CalendarError::AlreadyConfigured`] if a calendar has already
    /// been set. A context's calendar cannot be replaced because doing so
    /// would reinterpret dates used by existing plans and model state.
    fn set_calendar(&mut self, calendar: Calendar) -> Result<(), CalendarError> {
        let configured_calendar = self.get_data_mut(CalendarPlugin);
        if configured_calendar.is_some() {
            return Err(CalendarError::AlreadyConfigured);
        }
        *configured_calendar = Some(calendar);
        Ok(())
    }

    /// Returns the configured calendar, if one has been set.
    #[must_use]
    fn get_calendar(&self) -> Option<Calendar> {
        *self.get_data(CalendarPlugin)
    }

    /// Returns the simulation time corresponding to `date`.
    ///
    /// # Errors
    ///
    /// Returns [`CalendarError::NotConfigured`] if no calendar has been set.
    fn get_time_at_date(&self, date: Date) -> Result<f64, CalendarError> {
        Ok(self
            .get_calendar()
            .ok_or(CalendarError::NotConfigured)?
            .time_at_date(date))
    }

    /// Returns the date containing the supplied simulation `time`.
    ///
    /// # Errors
    ///
    /// Returns [`CalendarError::NotConfigured`] if no calendar has been set,
    /// or propagates conversion errors from [`Calendar::date_at_time`].
    fn get_date_at_time(&self, time: f64) -> Result<Date, CalendarError> {
        self.get_calendar()
            .ok_or(CalendarError::NotConfigured)?
            .date_at_time(time)
    }

    /// Returns the calendar date containing the current simulation time.
    ///
    /// # Errors
    ///
    /// Returns [`CalendarError::NotConfigured`] if no calendar has been set,
    /// or [`CalendarError::DateOutOfRange`] if the current simulation time is
    /// outside the supported date range.
    fn get_current_date(&self) -> Result<Date, CalendarError> {
        self.get_date_at_time(self.get_current_time())
    }

    /// Adds a plan scheduled for the beginning of `date`.
    ///
    /// # Errors
    ///
    /// Returns [`CalendarError::NotConfigured`] if no calendar has been set.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Context::add_plan`], including
    /// when `date` corresponds to a simulation time in the past.
    fn add_plan_on_date(
        &mut self,
        date: Date,
        callback: impl FnOnce(&mut Context) + 'static,
    ) -> Result<PlanId, CalendarError> {
        let time = self.get_time_at_date(date)?;
        Ok(self.add_plan(time, callback))
    }
}

impl ContextCalendarExt for Context {}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use super::*;

    fn date(year: i32, month: u32, day: u32) -> Date {
        Date::from_ymd_opt(year, month, day).expect("test date should be valid")
    }

    fn schedule_recurring_plan(
        context: &mut Context,
        current_date: Date,
        end_date: Date,
        interval_days: u64,
        executed_dates: Rc<RefCell<Vec<Date>>>,
    ) {
        context
            .add_plan_on_date(current_date, move |context| {
                executed_dates
                    .borrow_mut()
                    .push(context.get_current_date().unwrap());

                let next_date = current_date
                    .checked_add_days(Days::new(interval_days))
                    .expect("test recurrence should remain within the supported date range");
                if next_date <= end_date {
                    schedule_recurring_plan(
                        context,
                        next_date,
                        end_date,
                        interval_days,
                        executed_dates,
                    );
                }
            })
            .unwrap();
    }

    #[test]
    fn calendar_maps_fractional_times_to_containing_day() {
        let calendar = Calendar::new(date(2025, 1, 1));

        assert_eq!(calendar.date_at_time(0.0), Ok(date(2025, 1, 1)));
        assert_eq!(calendar.date_at_time(0.999), Ok(date(2025, 1, 1)));
        assert_eq!(calendar.date_at_time(1.0), Ok(date(2025, 1, 2)));
        assert_eq!(calendar.date_at_time(-0.001), Ok(date(2024, 12, 31)));
        assert_eq!(calendar.date_at_time(-1.0), Ok(date(2024, 12, 31)));
        assert_eq!(calendar.date_at_time(-1.001), Ok(date(2024, 12, 30)));
    }

    #[test]
    fn calendar_handles_leap_day_and_date_round_trips() {
        let calendar = Calendar::new(date(2024, 2, 28));
        let dates = [
            Date::MIN,
            date(2024, 2, 28),
            date(2024, 2, 29),
            date(2024, 3, 1),
            Date::MAX,
        ];

        assert_eq!(calendar.date_at_time(1.0), Ok(date(2024, 2, 29)));
        assert_eq!(calendar.date_at_time(2.0), Ok(date(2024, 3, 1)));
        for expected in dates {
            let time = calendar.time_at_date(expected);
            assert_eq!(calendar.date_at_time(time), Ok(expected));
        }
    }

    #[test]
    fn calendar_rejects_invalid_or_out_of_range_times() {
        let calendar = Calendar::new(date(2025, 1, 1));

        assert!(matches!(
            calendar.date_at_time(f64::NAN),
            Err(CalendarError::NonFiniteTime { time }) if time.is_nan()
        ));
        assert_eq!(
            calendar.date_at_time(f64::INFINITY),
            Err(CalendarError::NonFiniteTime {
                time: f64::INFINITY
            })
        );
        assert_eq!(
            calendar.date_at_time(f64::MAX),
            Err(CalendarError::DateOutOfRange { time: f64::MAX })
        );
        assert_eq!(
            calendar.date_at_time(-f64::MAX),
            Err(CalendarError::DateOutOfRange { time: -f64::MAX })
        );
    }

    #[test]
    fn date_and_calendar_serialize_as_iso_dates() {
        let epoch = date(2025, 1, 2);
        let calendar = Calendar::new(epoch);

        assert_eq!(serde_json::to_string(&epoch).unwrap(), r#""2025-01-02""#);
        assert_eq!(
            serde_json::to_string(&calendar).unwrap(),
            r#"{"epoch":"2025-01-02"}"#
        );
        assert_eq!(
            serde_json::from_str::<Calendar>(r#"{"epoch":"2025-01-02"}"#).unwrap(),
            calendar
        );
    }

    #[test]
    fn context_reports_missing_and_duplicate_calendar_configuration() {
        let mut context = Context::new();
        let calendar = Calendar::new(date(2025, 1, 1));

        assert_eq!(context.get_calendar(), None);
        assert_eq!(
            context.get_current_date(),
            Err(CalendarError::NotConfigured)
        );
        assert_eq!(context.set_calendar(calendar), Ok(()));
        assert_eq!(context.get_calendar(), Some(calendar));
        assert_eq!(
            context.set_calendar(calendar),
            Err(CalendarError::AlreadyConfigured)
        );
    }

    #[test]
    fn current_date_uses_negative_start_time() {
        let mut context = Context::new();
        context.set_start_time(-0.25);
        context
            .set_calendar(Calendar::new(date(2025, 1, 1)))
            .unwrap();

        assert_eq!(context.get_current_date(), Ok(date(2024, 12, 31)));
    }

    #[test]
    fn add_plan_on_date_schedules_at_start_of_day() {
        let mut context = Context::new();
        context
            .set_calendar(Calendar::new(date(2025, 1, 1)))
            .unwrap();
        let executed = Rc::new(Cell::new(false));
        let executed_in_plan = Rc::clone(&executed);

        context
            .add_plan_on_date(date(2025, 1, 10), move |context| {
                assert_eq!(context.get_current_time(), 9.0);
                assert_eq!(context.get_current_date(), Ok(date(2025, 1, 10)));
                executed_in_plan.set(true);
            })
            .unwrap();
        context.execute();

        assert!(executed.get());
    }

    #[test]
    fn plan_runs_every_monday() {
        let mut context = Context::new();
        context
            .set_calendar(Calendar::new(date(2025, 1, 1)))
            .unwrap();
        let executed_dates = Rc::new(RefCell::new(Vec::new()));

        schedule_recurring_plan(
            &mut context,
            date(2025, 1, 6),
            date(2025, 2, 3),
            7,
            Rc::clone(&executed_dates),
        );
        context.execute();

        let expected_dates = [
            date(2025, 1, 6),
            date(2025, 1, 13),
            date(2025, 1, 20),
            date(2025, 1, 27),
            date(2025, 2, 3),
        ];
        assert_eq!(*executed_dates.borrow(), expected_dates);
        assert!(executed_dates
            .borrow()
            .iter()
            .all(|date| date.weekday() == Weekday::Mon));
    }

    #[test]
    fn plan_runs_every_other_week() {
        let mut context = Context::new();
        context
            .set_calendar(Calendar::new(date(2025, 1, 1)))
            .unwrap();
        let executed_dates = Rc::new(RefCell::new(Vec::new()));

        schedule_recurring_plan(
            &mut context,
            date(2025, 1, 6),
            date(2025, 2, 17),
            14,
            Rc::clone(&executed_dates),
        );
        context.execute();

        assert_eq!(
            *executed_dates.borrow(),
            [
                date(2025, 1, 6),
                date(2025, 1, 20),
                date(2025, 2, 3),
                date(2025, 2, 17),
            ]
        );
    }
}
