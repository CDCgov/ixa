//! Trigger criterion for a specific simulation time.
//!
//! [`TimeTrigger`] observes the simulation clock and emits when the simulation reaches a configured
//! time and execution phase.
//!
//! ## Construction
//!
//! ```rust,ignore
//! TimeTrigger::at(at)
//! TimeTrigger::at_phase(at, phase)
//! TimeTrigger::at(at).with_phase(phase)
//! ```
//!
//! ## Observation
//!
//! The observation data passed to
//! [`TriggerCriterion::emit_with`](super::TriggerCriterion::emit_with) is [`TimeTriggerEvent`]. It
//! contains the simulation time observed when the scheduled plan runs and the phase used to schedule
//! it:
//!
//! ```rust,ignore
//! pub struct TimeTriggerEvent {
//!     pub time: f64,
//!     pub phase: ExecutionPhase,
//! }
//! ```
//!
//! ## Semantics
//!
//! This trigger is equivalent to scheduling a plan that emits an event with
//! [`context.add_plan`](crate::Context::add_plan) /
//! [`context.add_plan_with_phase`](crate::Context::add_plan_with_phase).
//!
//! [`TimeTrigger::at`] uses [`ExecutionPhase::Normal`](crate::ExecutionPhase::Normal).
//! Since time is monotonic, this criterion does not use [`Direction`](super::Direction) or
//! [`TriggerMode`](super::TriggerMode). It emits once, when its scheduled plan executes. If several
//! plans are scheduled for the same time, the selected [`ExecutionPhase`](crate::ExecutionPhase)
//! controls phase ordering.
//!
//! ## Example
//!
//! ```rust
//! use ixa::{Context, ExecutionPhase, IxaEvent};
//! use ixa::triggers::{ContextTriggersExt, TimeTrigger, TriggerCriterion};
//!
//! #[derive(IxaEvent)]
//! struct StopTimeReached {
//!     time: f64,
//!     phase: ExecutionPhase,
//! }
//!
//! let mut context = Context::new();
//!
//! context.register_trigger(
//!     TimeTrigger::at_phase(50.0, ExecutionPhase::Last)
//!         .emit_with(|observation| StopTimeReached {
//!             time: observation.time,
//!             phase: observation.phase,
//!         }),
//! );
//!
//! context.subscribe_to_event(|context, _event: StopTimeReached| {
//!     context.shutdown();
//! });
//! ```
//!
use super::TriggerCriterion;
use crate::{Context, ExecutionPhase};

pub struct TimeTrigger {
    at: f64,
    phase: ExecutionPhase,
}

#[derive(Clone, Copy, Debug)]
pub struct TimeTriggerEvent {
    pub time: f64,
    pub phase: ExecutionPhase,
}

impl TimeTrigger {
    #[must_use]
    pub fn at(at: f64) -> Self {
        Self {
            at,
            phase: ExecutionPhase::Normal,
        }
    }

    #[must_use]
    pub fn at_phase(at: f64, phase: ExecutionPhase) -> Self {
        Self { at, phase }
    }

    #[must_use]
    pub fn with_phase(mut self, phase: ExecutionPhase) -> Self {
        self.phase = phase;
        self
    }
}

impl TriggerCriterion for TimeTrigger {
    type Observation = TimeTriggerEvent;

    fn install<F>(self, context: &mut Context, on_match: F)
    where
        F: Fn(&mut Context, Self::Observation) + 'static,
    {
        let phase = self.phase;
        context.add_plan_with_phase(
            self.at,
            move |context| {
                let event = TimeTriggerEvent {
                    time: context.get_current_time(),
                    phase,
                };
                on_match(context, event);
            },
            phase,
        );
    }
}
