//! Prototype trigger API.
//!
//! This module provides a prototype trigger API for registering criteria that
//! emit concrete user-defined [`IxaEvent`]s.

use std::cell::Cell;
use std::rc::Rc;

use crate::entity::events::{EntityCreatedEvent, PropertyChangeEvent};
use crate::entity::property::Property;
use crate::entity::{ContextEntitiesExt, Entity, EntityId};
use crate::{Context, EntityPropertyTuple, ExecutionPhase, IxaEvent};

/// Criteria monitored by the trigger system.
///
/// When a criterion is satisfied, `ContextTriggersExt::register_trigger` maps
/// the matching [`TriggerEvent`] into a concrete user-defined [`IxaEvent`] and
/// emits that event.
#[derive(Clone, Copy, Debug)]
pub enum TriggerCriteria<P> {
    EntityCount {
        threshold: usize,
    },
    PropertyValueCount {
        value: P,
        threshold: usize,
        direction: ThresholdDirection,
        mode: TriggerMode,
    },
    PropertyTransition {
        from: Option<P>,
        to: Option<P>,
        mode: TriggerMode,
    },
    Time {
        at: f64,
        phase: ExecutionPhase,
    },
}

/// Direction used when testing threshold criteria.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThresholdDirection {
    AtLeast,
    AtMost,
}

/// Whether a trigger emits once or every time its criteria is satisfied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriggerMode {
    Once,
    Repeating,
}

/// Details about the observation that satisfied a trigger criterion.
///
/// Client code can ignore this payload, map a subset of it into a custom event,
/// or define a newtype event around `TriggerEvent<E, P>`.
#[derive(Clone, Copy, Debug)]
pub enum TriggerEvent<E, P>
where
    E: Entity,
    P: Property<E>,
{
    EntityCount {
        count: usize,
        threshold: usize,
    },
    PropertyValueCount {
        value: P,
        count: usize,
        threshold: usize,
    },
    PropertyTransition {
        entity_id: EntityId<E>,
        previous: P,
        current: P,
    },
    Time {
        time: f64,
        phase: ExecutionPhase,
    },
}

/// Extension trait for registering triggers on a [`Context`].
pub trait ContextTriggersExt {
    /// Register a trigger criterion that emits a concrete event.
    ///
    /// `make_event` is the adapter from trigger-system details to the concrete
    /// event type that subscribers use as event identity.
    fn register_trigger<E, P, Ev>(
        &mut self,
        criteria: TriggerCriteria<P>,
        make_event: impl Fn(TriggerEvent<E, P>) -> Ev + 'static,
    ) where
        E: Entity,
        P: Property<E>,
        Ev: IxaEvent;
}

impl ContextTriggersExt for Context {
    fn register_trigger<E, P, Ev>(
        &mut self,
        criteria: TriggerCriteria<P>,
        make_event: impl Fn(TriggerEvent<E, P>) -> Ev + 'static,
    ) where
        E: Entity,
        P: Property<E>,
        Ev: IxaEvent,
    {
        match criteria {
            TriggerCriteria::Time { at, phase } => {
                self.add_plan_with_phase(
                    at,
                    move |context| {
                        let trigger_event = TriggerEvent::Time {
                            time: context.get_current_time(),
                            phase,
                        };
                        context.emit_event(make_event(trigger_event));
                    },
                    phase,
                );
            }
            TriggerCriteria::EntityCount { threshold } => {
                self.subscribe_to_event(move |context, _event: EntityCreatedEvent<E>| {
                    let count = context.get_entity_count::<E>();
                    if count == threshold {
                        let trigger_event = TriggerEvent::<E, P>::EntityCount { count, threshold };
                        context.emit_event(make_event(trigger_event));
                    }
                });
            }
            TriggerCriteria::PropertyValueCount {
                value,
                threshold,
                direction,
                mode,
            } => match mode {
                TriggerMode::Once => {
                    let state = Rc::new(Cell::new(CountTriggerState {
                        active: true,
                        count: self.query_entity_count(EntityPropertyTuple::<E, _>::new((value,))),
                    }));
                    let make_event = Rc::new(make_event);

                    self.subscribe_to_event({
                        let state = Rc::clone(&state);
                        let make_event = Rc::clone(&make_event);
                        move |context, event: EntityCreatedEvent<E>| {
                            let current = context.get_property::<E, P>(event.entity_id);
                            if current == value {
                                let mut state_value = state.get();
                                if !state_value.active {
                                    return;
                                }
                                let previous_count = state_value.count;
                                state_value.count += 1;
                                let threshold_crossed = match direction {
                                    ThresholdDirection::AtLeast => {
                                        previous_count < threshold && state_value.count >= threshold
                                    }
                                    ThresholdDirection::AtMost => {
                                        previous_count > threshold && state_value.count <= threshold
                                    }
                                };
                                if threshold_crossed {
                                    let trigger_event = TriggerEvent::<E, P>::PropertyValueCount {
                                        value,
                                        count: state_value.count,
                                        threshold,
                                    };
                                    context.emit_event(make_event(trigger_event));
                                    state_value.active = false;
                                }
                                state.set(state_value);
                            }
                        }
                    });

                    self.subscribe_to_event({
                        let state = Rc::clone(&state);
                        move |context, event: PropertyChangeEvent<E, P>| {
                            let mut state_value = state.get();
                            if !state_value.active {
                                return;
                            }
                            let previous_count = state_value.count;
                            state_value.count =
                                match (event.previous == value, event.current == value) {
                                    (false, true) => state_value.count + 1,
                                    (true, false) => state_value.count - 1,
                                    _ => state_value.count,
                                };
                            let threshold_crossed = match direction {
                                ThresholdDirection::AtLeast => {
                                    previous_count < threshold && state_value.count >= threshold
                                }
                                ThresholdDirection::AtMost => {
                                    previous_count > threshold && state_value.count <= threshold
                                }
                            };
                            if threshold_crossed {
                                let trigger_event = TriggerEvent::PropertyValueCount {
                                    value,
                                    count: state_value.count,
                                    threshold,
                                };
                                context.emit_event(make_event(trigger_event));
                                state_value.active = false;
                            }
                            state.set(state_value);
                        }
                    });
                }
                TriggerMode::Repeating => {
                    let count = Rc::new(Cell::new(self.query_entity_count(EntityPropertyTuple::<
                        E,
                        _,
                    >::new(
                        (
                        value,
                    )
                    ))));
                    let make_event = Rc::new(make_event);

                    self.subscribe_to_event({
                        let count = Rc::clone(&count);
                        let make_event = Rc::clone(&make_event);
                        move |context, event: EntityCreatedEvent<E>| {
                            let current = context.get_property::<E, P>(event.entity_id);
                            if current == value {
                                let previous_count = count.get();
                                let current_count = previous_count + 1;
                                count.set(current_count);
                                let threshold_crossed = match direction {
                                    ThresholdDirection::AtLeast => {
                                        previous_count < threshold && current_count >= threshold
                                    }
                                    ThresholdDirection::AtMost => {
                                        previous_count > threshold && current_count <= threshold
                                    }
                                };
                                if threshold_crossed {
                                    let trigger_event = TriggerEvent::<E, P>::PropertyValueCount {
                                        value,
                                        count: current_count,
                                        threshold,
                                    };
                                    context.emit_event(make_event(trigger_event));
                                }
                            }
                        }
                    });

                    self.subscribe_to_event({
                        let count = Rc::clone(&count);
                        move |context, event: PropertyChangeEvent<E, P>| {
                            let previous_count = count.get();
                            let current_count =
                                match (event.previous == value, event.current == value) {
                                    (false, true) => previous_count + 1,
                                    (true, false) => previous_count - 1,
                                    _ => previous_count,
                                };
                            count.set(current_count);
                            let threshold_crossed = match direction {
                                ThresholdDirection::AtLeast => {
                                    previous_count < threshold && current_count >= threshold
                                }
                                ThresholdDirection::AtMost => {
                                    previous_count > threshold && current_count <= threshold
                                }
                            };
                            if threshold_crossed {
                                let trigger_event = TriggerEvent::PropertyValueCount {
                                    value,
                                    count: current_count,
                                    threshold,
                                };
                                context.emit_event(make_event(trigger_event));
                            }
                        }
                    });
                }
            },
            TriggerCriteria::PropertyTransition { from, to, mode } => match mode {
                TriggerMode::Once => {
                    // We store the active state in the closure itself
                    let active = Rc::new(Cell::new(true));
                    self.subscribe_to_event(move |context, event: PropertyChangeEvent<E, P>| {
                        if !active.get() {
                            return;
                        }
                        let from_matches = from.is_none_or(|from| event.previous == from);
                        let to_matches = to.is_none_or(|to| event.current == to);
                        if from_matches && to_matches {
                            let trigger_event = TriggerEvent::PropertyTransition {
                                entity_id: event.entity_id,
                                previous: event.previous,
                                current: event.current,
                            };
                            context.emit_event(make_event(trigger_event));
                            active.set(false);
                        }
                    });
                }
                TriggerMode::Repeating => {
                    self.subscribe_to_event(move |context, event: PropertyChangeEvent<E, P>| {
                        let from_matches = from.is_none_or(|from| event.previous == from);
                        let to_matches = to.is_none_or(|to| event.current == to);
                        if from_matches && to_matches {
                            let trigger_event = TriggerEvent::PropertyTransition {
                                entity_id: event.entity_id,
                                previous: event.previous,
                                current: event.current,
                            };
                            context.emit_event(make_event(trigger_event));
                        }
                    });
                }
            },
        }
    }
}

#[derive(Clone, Copy)]
struct CountTriggerState {
    active: bool,
    count: usize,
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]

    use ixa_derive::IxaEvent;

    use super::*;
    use crate::entity::EntityId;
    use crate::{define_entity, define_property, Context, IxaEvent};

    define_entity!(Person);
    define_entity!(Case);

    define_property!(
        enum InfectionStatus {
            Susceptible,
            Infectious,
            Recovered,
        },
        Person,
        default_const = InfectionStatus::Susceptible
    );

    define_property!(struct Alive(bool), Person, default_const = Alive(true));

    define_property!(
        enum CaseStatus {
            Detected,
        },
        Case,
        default_const = CaseStatus::Detected
    );

    #[derive(IxaEvent)]
    struct InfectiousThresholdReached {
        count: usize,
    }

    #[derive(IxaEvent)]
    struct CaseThresholdReached;

    #[derive(IxaEvent)]
    struct FirstDeath {
        person: EntityId<Person>,
    }

    #[derive(IxaEvent)]
    struct StopTimeReached;

    #[test]
    fn register_property_value_count_trigger() {
        let mut context = Context::new();

        context.register_trigger::<Person, InfectionStatus, InfectiousThresholdReached>(
            TriggerCriteria::PropertyValueCount {
                value: InfectionStatus::Infectious,
                threshold: 100,
                direction: ThresholdDirection::AtLeast,
                mode: TriggerMode::Once,
            },
            |trigger_event| match trigger_event {
                TriggerEvent::PropertyValueCount { count, .. } => {
                    InfectiousThresholdReached { count }
                }
                _ => unreachable!("trigger criteria determines trigger event shape"),
            },
        );

        context.subscribe_to_event(|context, _event: InfectiousThresholdReached| {
            context.shutdown();
        });
    }

    #[test]
    fn register_entity_count_trigger() {
        let mut context = Context::new();

        context.register_trigger::<Case, CaseStatus, CaseThresholdReached>(
            TriggerCriteria::EntityCount { threshold: 10 },
            |_| CaseThresholdReached,
        );
    }

    #[test]
    fn register_property_transition_trigger() {
        let mut context = Context::new();

        context.register_trigger::<Person, Alive, FirstDeath>(
            TriggerCriteria::PropertyTransition {
                from: Some(Alive(true)),
                to: Some(Alive(false)),
                mode: TriggerMode::Once,
            },
            |trigger_event| match trigger_event {
                TriggerEvent::PropertyTransition { entity_id, .. } => {
                    FirstDeath { person: entity_id }
                }
                _ => unreachable!("trigger criteria determines trigger event shape"),
            },
        );
    }

    #[test]
    fn register_time_trigger() {
        let mut context = Context::new();

        context.register_trigger::<Person, InfectionStatus, StopTimeReached>(
            TriggerCriteria::Time {
                at: 50.0,
                phase: ExecutionPhase::Normal,
            },
            |_| StopTimeReached,
        );
    }

    #[test]
    fn register_newtype_wrapper_event() {
        #[derive(IxaEvent)]
        struct FirstDeathDetails(TriggerEvent<Person, Alive>);

        let mut context = Context::new();

        context.register_trigger::<Person, Alive, FirstDeathDetails>(
            TriggerCriteria::PropertyTransition {
                from: Some(Alive(true)),
                to: Some(Alive(false)),
                mode: TriggerMode::Once,
            },
            FirstDeathDetails,
        );
    }
}
