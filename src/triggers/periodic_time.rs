use super::TriggerCriterion;
use crate::{Context, ExecutionPhase};

/// Trigger criterion for regular simulation time intervals.
///
/// [`PeriodicTimeTrigger`] observes the simulation clock and emits repeatedly at a configured
/// period and execution phase.
///
/// ## Construction
///
/// ```rust,ignore
/// PeriodicTimeTrigger::every(period)
/// PeriodicTimeTrigger::every_with_phase(period, phase)
/// PeriodicTimeTrigger::every(period).with_phase(phase) // Equivalent to `every_with_phase`
/// PeriodicTimeTrigger::every(period).start_with_delay(delay)
/// PeriodicTimeTrigger::every(period).start_at(start_time)
/// ```
///
/// ## Observation
///
/// The observation data passed to
/// [`TriggerCriterion::emit_with`](super::TriggerCriterion::emit_with) is
/// [`PeriodicTimeTriggerEvent`]. It contains the simulation time observed when the scheduled
/// periodic plan runs, the configured period, and the phase used to schedule it:
///
/// ```rust,ignore
/// pub struct PeriodicTimeTriggerEvent {
///     pub time: f64,
///     pub period: f64,
///     pub phase: ExecutionPhase,
/// }
/// ```
///
/// ## Semantics
///
/// This trigger uses the same rescheduling behavior as periodic plans: when the scheduled callback
/// runs, the next occurrence is scheduled at `current_time + period` if there are still plans in the
/// queue. Unlike [`Context::add_periodic_plan_with_phase`](crate::Context::add_periodic_plan_with_phase),
/// the first occurrence is seeded explicitly so it can start at the current time, after a delay, or
/// at an absolute simulation time.
///
/// By default, the first occurrence is scheduled at `context.get_current_time()` when the trigger is
/// installed, and the execution phase is
/// [`ExecutionPhase::Normal`](crate::ExecutionPhase::Normal).
///
/// The period must be positive, finite, and not NaN. A delay must be non-negative, finite, and not
/// NaN. An absolute start time must be finite and not NaN; the context validates at trigger
/// installation that it is not in the past.
///
/// Since time is monotonic, this criterion does not use [`Direction`](super::Direction) or
/// [`TriggerMode`](super::TriggerMode). It emits whenever its periodic schedule executes. If several
/// plans are scheduled for the same time, the selected [`ExecutionPhase`](crate::ExecutionPhase)
/// controls phase ordering.
///
/// ## Example
///
/// ```rust
/// use ixa::{Context, ExecutionPhase, IxaEvent};
/// use ixa::triggers::{ContextTriggersExt, PeriodicTimeTrigger, TriggerCriterion};
///
/// #[derive(IxaEvent)]
/// struct ReportTimeReached {
///     time: f64,
///     period: f64,
///     phase: ExecutionPhase,
/// }
///
/// let mut context = Context::new();
///
/// context.register_trigger(
///     PeriodicTimeTrigger::every(7.0)
///         .with_phase(ExecutionPhase::Last)
///         .start_with_delay(7.0)
///         .emit_with(|observation| ReportTimeReached {
///             time: observation.time,
///             period: observation.period,
///             phase: observation.phase,
///         }),
/// );
///
/// context.subscribe_to_event(|_context, _event: ReportTimeReached| {
///     // collect periodic reports
/// });
/// ```
///
pub struct PeriodicTimeTrigger {
    period: f64,
    start: PeriodicTimeTriggerStart,
    phase: ExecutionPhase,
}

enum PeriodicTimeTriggerStart {
    CurrentTime,
    Delay(f64),
    At(f64),
}

#[derive(Clone, Copy, Debug)]
pub struct PeriodicTimeTriggerEvent {
    pub time: f64,
    pub period: f64,
    pub phase: ExecutionPhase,
}

impl PeriodicTimeTrigger {
    #[must_use]
    pub fn every(period: f64) -> Self {
        validate_period(period);
        Self {
            period,
            start: PeriodicTimeTriggerStart::CurrentTime,
            phase: ExecutionPhase::Normal,
        }
    }

    #[must_use]
    pub fn every_with_phase(period: f64, phase: ExecutionPhase) -> Self {
        validate_period(period);
        Self {
            period,
            start: PeriodicTimeTriggerStart::CurrentTime,
            phase,
        }
    }

    #[must_use]
    pub fn with_phase(mut self, phase: ExecutionPhase) -> Self {
        self.phase = phase;
        self
    }

    #[must_use]
    pub fn start_with_delay(mut self, delay: f64) -> Self {
        assert!(
            delay >= 0.0 && !delay.is_nan() && !delay.is_infinite(),
            "delay must be greater than or equal to 0"
        );
        self.start = PeriodicTimeTriggerStart::Delay(delay);
        self
    }

    #[must_use]
    pub fn start_at(mut self, start_time: f64) -> Self {
        assert!(
            !start_time.is_nan(),
            "start_time {start_time} is invalid: cannot be NaN"
        );
        assert!(
            !start_time.is_infinite(),
            "start_time {start_time} is invalid: cannot be infinite"
        );
        self.start = PeriodicTimeTriggerStart::At(start_time);
        self
    }
}

impl TriggerCriterion for PeriodicTimeTrigger {
    type Observation = PeriodicTimeTriggerEvent;

    fn install<F>(self, context: &mut Context, on_match: F)
    where
        F: Fn(&mut Context, Self::Observation) + 'static,
    {
        let start_time = match self.start {
            PeriodicTimeTriggerStart::CurrentTime => context.get_current_time(),
            PeriodicTimeTriggerStart::Delay(delay) => context.get_current_time() + delay,
            PeriodicTimeTriggerStart::At(start_time) => start_time,
        };
        let period = self.period;
        let phase = self.phase;

        context.add_plan_with_phase(
            start_time,
            move |context| {
                context.evaluate_periodic_and_schedule_next(
                    period,
                    move |context| {
                        on_match(
                            context,
                            PeriodicTimeTriggerEvent {
                                time: context.get_current_time(),
                                period,
                                phase,
                            },
                        );
                    },
                    phase,
                );
            },
            phase,
        );
    }
}

fn validate_period(period: f64) {
    assert!(
        period > 0.0 && !period.is_nan() && !period.is_infinite(),
        "period must be greater than 0"
    );
}
