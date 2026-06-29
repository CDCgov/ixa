//! Trigger criteria that emit user-defined events.
//!
//! A trigger is a way to say: when some simulation criterion is met, emit a
//! concrete user-defined [`IxaEvent`]. Trigger-emitted events
//! are ordinary Ixa events, so any number of subscribers can listen for them
//! with [`Context::subscribe_to_event`](crate::Context::subscribe_to_event).
//!
//! A trigger criterion is not itself registered on a context. A criterion, such
//! as [`PropertyChangeTrigger`] or [`TimeTrigger`], defines what should be
//! monitored. A complete trigger is created only after binding that criterion
//! to a concrete event with one of the `emit_*` methods. The value returned by
//! `emit_with`, `emit_value`, or `emit_default` is the value passed to
//! [`ContextTriggersExt::register_trigger`].
//!
//! The usual flow is:
//!
//! 1. Choose one of the built-in trigger criteria.
//! 2. Bind it to the event you want emitted with [`TriggerCriterion::emit_with`],
//!    [`TriggerCriterion::emit_value`], or [`TriggerCriterion::emit_default`].
//! 3. Register the complete trigger with [`ContextTriggersExt::register_trigger`].
//! 4. Subscribe to the emitted user event as usual.
//!
//! [`TogglingTrigger`] is a composite trigger for stateful on/off behavior. Instead of binding a
//! single criterion to a single event, it combines an activation criterion and a deactivation
//! criterion, each with its own emitted event (of possibly distinct types).
//!
//! The usual flow for a toggling trigger is:
//!
//! 1. Choose the activation and deactivation criteria.
//! 2. Pair them with [`TogglingTriggerCriteria::new`].
//! 3. Bind the pair to activation and deactivation events with one of the
//!    `TogglingTriggerCriteria::emit_*` methods.
//! 4. Register the complete trigger with [`ContextTriggersExt::register_trigger`].
//! 5. Subscribe to the activation and deactivation events as usual.
//!
//! ## Construct an event from observation data
//!
//! Each trigger criterion has its own observation data type, available as the criterion's
//! [`TriggerCriterion::Observation`] associated type. For example, [`PropertyChangeTrigger`]
//! observations use [`PropertyChangeTriggerEvent`] containing the entity ID and the previous and
//! current property values. [`EntityCountTrigger`], [`PropertyValueCountTrigger`], [`TimeTrigger`],
//! and [`PeriodicTimeTrigger`] use their corresponding `*TriggerEvent` types.
//!
//! For events that do not need observation data, use [`TriggerCriterion::emit_value`] to emit
//! a constant event value, or [`TriggerCriterion::emit_default`] when the event type implements
//! [`Default`].
//!
//! Use [`TriggerCriterion::emit_with`] when the emitted event should contain data from the trigger
//! observation. When the criterion is met, this observation value is passed to the event
//! constructor (typically a closure or static constructor method) supplied to `emit_with`, and that
//! constructor returns the concrete user-defined [`IxaEvent`] that subscribers
//! will receive.
//!
//! ```rust
//! use ixa::{Context, define_entity, define_property, IxaEvent};
//! use ixa::entity::EntityId;
//! use ixa::triggers::{ContextTriggersExt, PropertyChangeTrigger, TriggerCriterion};
//! use ixa_derive::IxaEvent;
//!
//! define_entity!(Person);
//! define_property!(struct Alive(bool), Person, default_const = Alive(true));
//!
//! #[derive(IxaEvent)]
//! struct FirstDeath {
//!     person: EntityId<Person>,
//! }
//!
//! let mut context = Context::new();
//!
//! context.register_trigger(
//!     PropertyChangeTrigger::to(Alive(false))
//!         .once()
//!         .emit_with(|event| FirstDeath {
//!             person: event.entity_id,
//!         }),
//! );
//!
//! context.subscribe_to_event(|_context, _event: FirstDeath| {
//!     // perform cleanup tasks
//! });
//! ```
//!

mod entity_count;
mod periodic_time;
mod property_change;
mod property_value_count;
mod time;
mod toggling_trigger;

use std::marker::PhantomData;

pub use entity_count::{EntityCountTrigger, EntityCountTriggerEvent};
pub use periodic_time::{PeriodicTimeTrigger, PeriodicTimeTriggerEvent};
pub use property_change::{PropertyChangeTrigger, PropertyChangeTriggerEvent};
pub use property_value_count::{PropertyValueCountTrigger, PropertyValueCountTriggerEvent};
pub use time::{TimeTrigger, TimeTriggerEvent};
pub use toggling_trigger::{TogglingTrigger, TogglingTriggerCriteria};

use crate::{Context, IxaEvent};

/// Direction in which a count changed to reach a threshold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Increasing,
    Decreasing,
}

/// Whether a trigger emits once or every time its criterion is satisfied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriggerMode {
    Once,
    Repeating,
}

/// A bare trigger criterion: the condition that can be monitored. This module provides a collection
/// of types that implement this trait.
pub trait TriggerCriterion: Sized + 'static {
    /// The data that represents what is observed when the criterion is met.
    /// This data is passed to the handler installed for this criterion.
    type Observation: 'static;

    /// Install the criterion's monitoring logic in `context`.
    fn install<F>(self, context: &mut Context, on_match: F)
    where
        F: Fn(&mut Context, Self::Observation) + 'static;

    /// Bind this criterion to a constructor for a concrete user event.
    fn emit_with<Ev, F>(self, make_event: F) -> Trigger<Self, Ev, F>
    where
        Ev: IxaEvent,
        F: Fn(Self::Observation) -> Ev + 'static,
    {
        Trigger {
            criterion: self,
            make_event,
            _event: PhantomData,
        }
    }

    /// Bind this criterion to a default-valued concrete user event.
    fn emit_default<Ev>(self) -> Trigger<Self, Ev, impl Fn(Self::Observation) -> Ev>
    where
        Ev: IxaEvent + Default,
    {
        self.emit_with(|_| Ev::default())
    }

    /// Bind this criterion to a constant concrete user event value.
    fn emit_value<Ev>(self, event: Ev) -> Trigger<Self, Ev, impl Fn(Self::Observation) -> Ev>
    where
        Ev: IxaEvent,
    {
        self.emit_with(move |_| event)
    }
}

/// A complete installable trigger specification that can be passed to `context.register_trigger`.
/// This is automatically implemented by the `Trigger` types returned by the `emit_*` methods
/// on trigger criterion types. Client code should not implement this themselves.
pub trait TriggerSpec: Sized {
    fn install_in_context(self, context: &mut Context);
}

/// A criterion bound to a user event constructor. Values of this type are not constructed directly
/// but rather are returned by the `emit_*` methods on trigger criterion types. These values are
/// "complete" triggers than can be "installed" on a context with `context.register_trigger`.
pub struct Trigger<C, Ev, F> {
    criterion: C,
    make_event: F,
    _event: PhantomData<fn() -> Ev>,
}

impl<C, Ev, F> TriggerSpec for Trigger<C, Ev, F>
where
    C: TriggerCriterion,
    Ev: IxaEvent,
    F: Fn(C::Observation) -> Ev + 'static,
{
    fn install_in_context(self, context: &mut Context) {
        let make_event = self.make_event;
        self.criterion
            .install(context, move |context, observation| {
                context.emit_event(make_event(observation));
            });
    }
}

/// Extension trait for registering triggers on a [`Context`].
pub trait ContextTriggersExt {
    fn register_trigger<T: TriggerSpec>(&mut self, trigger: T);
}

impl ContextTriggersExt for Context {
    fn register_trigger<T: TriggerSpec>(&mut self, trigger: T) {
        trigger.install_in_context(self);
    }
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]

    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use super::*;
    use crate::entity::EntityId;
    use crate::{
        define_entity, define_property, with, Context, ContextEntitiesExt, ExecutionPhase, IxaEvent,
    };

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
        mode: TriggerMode,
    }

    #[derive(Default, IxaEvent)]
    struct CaseThresholdReached;

    #[derive(IxaEvent)]
    struct FirstDeath {
        person: EntityId<Person>,
        mode: TriggerMode,
    }

    #[derive(IxaEvent)]
    struct StopTimeReached {
        phase: ExecutionPhase,
    }

    #[derive(IxaEvent)]
    struct PeriodicTimeReached {
        time: f64,
        period: f64,
        phase: ExecutionPhase,
    }

    #[test]
    fn register_property_value_count_trigger() {
        let mut context = Context::new();

        context.register_trigger(
            PropertyValueCountTrigger::<Person, InfectionStatus>::increases_to(
                InfectionStatus::Infectious,
                100,
            )
            .emit_with(|event| InfectiousThresholdReached {
                count: event.count,
                mode: event.mode,
            }),
        );

        context.subscribe_to_event(|context, _event: InfectiousThresholdReached| {
            context.shutdown();
        });
    }

    #[test]
    fn register_entity_count_trigger() {
        let mut context = Context::new();

        context.register_trigger(
            EntityCountTrigger::<Case>::increases_to(10).emit_default::<CaseThresholdReached>(),
        );
    }

    #[test]
    fn register_property_change_trigger() {
        let mut context = Context::new();

        context.register_trigger(
            PropertyChangeTrigger::<Person, Alive>::to(Alive(false)).emit_with(|event| {
                FirstDeath {
                    person: event.entity_id,
                    mode: event.mode,
                }
            }),
        );
    }

    #[test]
    fn register_time_trigger() {
        let mut context = Context::new();

        context.register_trigger(
            TimeTrigger::at(50.0).emit_with(|event| StopTimeReached { phase: event.phase }),
        );
    }

    #[test]
    fn register_periodic_time_trigger_default_phase() {
        let mut context = Context::new();

        context.register_trigger(PeriodicTimeTrigger::every(1.0).emit_with(|event| {
            PeriodicTimeReached {
                time: event.time,
                period: event.period,
                phase: event.phase,
            }
        }));
    }

    #[test]
    fn register_constant_event_value() {
        #[derive(IxaEvent)]
        struct ShutdownRequested;

        let mut context = Context::new();

        context.register_trigger(
            TimeTrigger::at_phase(50.0, ExecutionPhase::Last)
                .emit_value::<ShutdownRequested>(ShutdownRequested),
        );
    }

    #[test]
    fn time_trigger_with_phase_sets_phase() {
        let mut context = Context::new();
        let observed_phase = Rc::new(Cell::new(None));
        let observed_phase_clone = Rc::clone(&observed_phase);

        context.register_trigger(
            TimeTrigger::at(1.0)
                .with_phase(ExecutionPhase::Last)
                .emit_with(|event| StopTimeReached { phase: event.phase }),
        );
        context.subscribe_to_event(move |_context, event: StopTimeReached| {
            observed_phase_clone.set(Some(event.phase));
        });

        context.execute();

        assert_eq!(observed_phase.get(), Some(ExecutionPhase::Last));
    }

    #[test]
    fn entity_count_trigger_emits_at_threshold() {
        let mut context = Context::new();
        let observed_count = Rc::new(Cell::new(0));
        let observed_count_clone = Rc::clone(&observed_count);

        #[derive(IxaEvent)]
        struct CountReached {
            count: usize,
        }

        context.register_trigger(
            EntityCountTrigger::<Case>::increases_to(2)
                .emit_with(|event| CountReached { count: event.count }),
        );
        context.subscribe_to_event(move |_context, event: CountReached| {
            observed_count_clone.set(event.count);
        });

        context.add_entity(Case).unwrap();
        context.add_entity(Case).unwrap();
        context.execute();

        assert_eq!(observed_count.get(), 2);
    }

    #[test]
    fn periodic_time_trigger_emits_on_current_time_then_periodically() {
        let mut context = Context::new();
        let observed = Rc::new(RefCell::new(Vec::new()));
        let observed_clone = Rc::clone(&observed);

        context.register_trigger(PeriodicTimeTrigger::every(1.0).emit_with(|event| {
            PeriodicTimeReached {
                time: event.time,
                period: event.period,
                phase: event.phase,
            }
        }));
        context.subscribe_to_event(move |_context, event: PeriodicTimeReached| {
            assert_eq!(event.period, 1.0);
            assert_eq!(event.phase, ExecutionPhase::Normal);
            observed_clone.borrow_mut().push(event.time);
        });

        context.add_plan(2.0, |_| {});
        context.execute();

        assert_eq!(*observed.borrow(), vec![0.0, 1.0, 2.0]);
    }

    #[test]
    fn periodic_time_trigger_start_with_delay() {
        let mut context = Context::new();
        let observed = Rc::new(RefCell::new(Vec::new()));
        let observed_clone = Rc::clone(&observed);

        context.register_trigger(
            PeriodicTimeTrigger::every(1.0)
                .start_with_delay(0.5)
                .emit_with(|event| PeriodicTimeReached {
                    time: event.time,
                    period: event.period,
                    phase: event.phase,
                }),
        );
        context.subscribe_to_event(move |_context, event: PeriodicTimeReached| {
            observed_clone.borrow_mut().push(event.time);
        });

        context.add_plan(2.5, |_| {});
        context.execute();

        assert_eq!(*observed.borrow(), vec![0.5, 1.5, 2.5]);
    }

    #[test]
    fn periodic_time_trigger_start_at() {
        let mut context = Context::new();
        let observed = Rc::new(RefCell::new(Vec::new()));
        let observed_clone = Rc::clone(&observed);

        context.register_trigger(PeriodicTimeTrigger::every(1.0).start_at(2.0).emit_with(
            |event| PeriodicTimeReached {
                time: event.time,
                period: event.period,
                phase: event.phase,
            },
        ));
        context.subscribe_to_event(move |_context, event: PeriodicTimeReached| {
            observed_clone.borrow_mut().push(event.time);
        });

        context.add_plan(4.0, |_| {});
        context.execute();

        assert_eq!(*observed.borrow(), vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn periodic_time_trigger_uses_requested_phase() {
        let mut context = Context::new();
        let observed = Rc::new(RefCell::new(Vec::new()));

        context.add_plan_with_phase(
            1.0,
            {
                let observed = Rc::clone(&observed);
                move |_| observed.borrow_mut().push("first")
            },
            ExecutionPhase::First,
        );
        context.add_plan_with_phase(
            1.0,
            {
                let observed = Rc::clone(&observed);
                move |_| observed.borrow_mut().push("normal")
            },
            ExecutionPhase::Normal,
        );
        context.add_plan_with_phase(
            1.0,
            {
                let observed = Rc::clone(&observed);
                move |_| observed.borrow_mut().push("last")
            },
            ExecutionPhase::Last,
        );

        PeriodicTimeTrigger::every_with_phase(1.0, ExecutionPhase::Last)
            .start_at(1.0)
            .install(&mut context, {
                let observed = Rc::clone(&observed);
                move |_context, event| {
                    assert_eq!(event.phase, ExecutionPhase::Last);
                    observed.borrow_mut().push("trigger");
                }
            });

        context.execute();

        assert_eq!(
            *observed.borrow(),
            vec!["first", "normal", "last", "trigger"]
        );
    }

    #[test]
    fn property_change_trigger_emits_matching_change() {
        let mut context = Context::new();
        let observed_person = Rc::new(Cell::new(None));
        let observed_person_clone = Rc::clone(&observed_person);

        #[derive(IxaEvent)]
        struct BecameDead {
            person: EntityId<Person>,
        }

        context.register_trigger(
            PropertyChangeTrigger::<Person, Alive>::to(Alive(false)).emit_with(|event| {
                BecameDead {
                    person: event.entity_id,
                }
            }),
        );
        context.subscribe_to_event(move |_context, event: BecameDead| {
            observed_person_clone.set(Some(event.person));
        });

        let person = context.add_entity(Person).unwrap();
        context.set_property(person, Alive(false));
        context.execute();

        assert_eq!(observed_person.get(), Some(person));
    }

    #[test]
    fn property_change_trigger_defaults_to_repeating() {
        let mut context = Context::new();
        let observed_count = Rc::new(Cell::new(0));
        let observed_count_clone = Rc::clone(&observed_count);

        #[derive(IxaEvent)]
        struct BecameDead {
            mode: TriggerMode,
        }

        context.register_trigger(
            PropertyChangeTrigger::<Person, Alive>::from_to(Alive(true), Alive(false))
                .emit_with(|event| BecameDead { mode: event.mode }),
        );
        context.subscribe_to_event(move |_context, event: BecameDead| {
            assert_eq!(event.mode, TriggerMode::Repeating);
            observed_count_clone.set(observed_count_clone.get() + 1);
        });

        let person = context.add_entity(Person).unwrap();
        context.set_property(person, Alive(false));
        context.set_property(person, Alive(true));
        context.set_property(person, Alive(false));
        context.execute();

        assert_eq!(observed_count.get(), 2);
    }

    #[test]
    fn property_value_count_trigger_defaults_to_repeating() {
        let mut context = Context::new();
        let observed_count = Rc::new(Cell::new(0));
        let observed_count_clone = Rc::clone(&observed_count);

        #[derive(IxaEvent)]
        struct InfectiousThresholdReached {
            mode: TriggerMode,
        }

        context.register_trigger(
            PropertyValueCountTrigger::<Person, InfectionStatus>::increases_to(
                InfectionStatus::Infectious,
                2,
            )
            .emit_with(|event| InfectiousThresholdReached { mode: event.mode }),
        );
        context.subscribe_to_event(move |_context, event: InfectiousThresholdReached| {
            assert_eq!(event.mode, TriggerMode::Repeating);
            observed_count_clone.set(observed_count_clone.get() + 1);
        });

        let first = context.add_entity(Person).unwrap();
        let second = context.add_entity(Person).unwrap();
        context.add_plan(0.1, move |context| {
            context.set_property(first, InfectionStatus::Infectious);
        });
        context.add_plan(0.2, move |context| {
            context.set_property(second, InfectionStatus::Infectious);
        });
        context.add_plan(0.3, move |context| {
            context.set_property(second, InfectionStatus::Susceptible);
        });
        context.add_plan(0.4, move |context| {
            context.set_property(second, InfectionStatus::Infectious);
        });
        context.execute();

        assert_eq!(observed_count.get(), 2);
    }

    #[test]
    fn property_value_count_trigger_changes_to_emits_in_either_direction() {
        let mut context = Context::new();
        let observed_directions = Rc::new(RefCell::new(Vec::new()));
        let observed_directions_clone = Rc::clone(&observed_directions);

        #[derive(IxaEvent)]
        struct InfectiousThresholdReached {
            direction_filter: Option<Direction>,
            direction: Direction,
        }

        context.register_trigger(
            PropertyValueCountTrigger::<Person, InfectionStatus>::changes_to(
                InfectionStatus::Infectious,
                2,
            )
            .repeating()
            .emit_with(|event| InfectiousThresholdReached {
                direction_filter: event.direction_filter,
                direction: event.direction,
            }),
        );
        context.subscribe_to_event(move |_context, event: InfectiousThresholdReached| {
            assert_eq!(event.direction_filter, None);
            observed_directions_clone.borrow_mut().push(event.direction);
        });

        let first = context.add_entity(Person).unwrap();
        let second = context.add_entity(Person).unwrap();
        let third = context.add_entity(Person).unwrap();
        context.add_plan(0.1, move |context| {
            context.set_property(first, InfectionStatus::Infectious);
        });
        context.add_plan(0.2, move |context| {
            context.set_property(second, InfectionStatus::Infectious);
        });
        context.add_plan(0.3, move |context| {
            context.set_property(third, InfectionStatus::Infectious);
        });
        context.add_plan(0.4, move |context| {
            context.set_property(second, InfectionStatus::Susceptible);
        });
        context.execute();

        assert_eq!(
            *observed_directions.borrow(),
            vec![Direction::Increasing, Direction::Decreasing]
        );
    }

    #[test]
    fn property_value_count_trigger_changes_to_ignores_no_op_writes() {
        let mut context = Context::new();
        let observed_count = Rc::new(Cell::new(0));
        let observed_count_clone = Rc::clone(&observed_count);

        #[derive(IxaEvent)]
        struct InfectiousThresholdReached;

        context.register_trigger(
            PropertyValueCountTrigger::<Person, InfectionStatus>::changes_to(
                InfectionStatus::Infectious,
                1,
            )
            .repeating()
            .emit_value::<InfectiousThresholdReached>(InfectiousThresholdReached),
        );
        context.subscribe_to_event(move |_context, _event: InfectiousThresholdReached| {
            observed_count_clone.set(observed_count_clone.get() + 1);
        });

        let person = context.add_entity(Person).unwrap();
        context.add_plan(0.1, move |context| {
            context.set_property(person, InfectionStatus::Infectious);
        });
        context.add_plan(0.2, move |context| {
            context.set_property(person, InfectionStatus::Infectious);
        });
        context.execute();

        assert_eq!(observed_count.get(), 1);
    }

    #[test]
    fn property_value_count_decreases_to_tracks_entities_created_with_tracked_value() {
        let mut context = Context::new();
        let observed = Rc::new(RefCell::new(Vec::new()));
        let observed_clone = Rc::clone(&observed);

        #[derive(IxaEvent)]
        struct InfectiousThresholdReached {
            count: usize,
            direction: Direction,
        }

        context.register_trigger(
            PropertyValueCountTrigger::<Person, InfectionStatus>::decreases_to(
                InfectionStatus::Infectious,
                1,
            )
            .repeating()
            .emit_with(|event| InfectiousThresholdReached {
                count: event.count,
                direction: event.direction,
            }),
        );
        context.subscribe_to_event(move |_context, event: InfectiousThresholdReached| {
            observed_clone
                .borrow_mut()
                .push((event.count, event.direction));
        });

        let first = context
            .add_entity(with!(Person, InfectionStatus::Infectious))
            .unwrap();
        let _second = context
            .add_entity(with!(Person, InfectionStatus::Infectious))
            .unwrap();
        context.add_plan(0.1, move |context| {
            context.set_property(first, InfectionStatus::Susceptible);
        });
        context.execute();

        assert_eq!(*observed.borrow(), vec![(1, Direction::Decreasing)]);
    }

    // Regression coverage for create-with-tracked-value followed by change-away before callbacks
    // drain. Property value count triggers must account for the creation-time value, not the value
    // read by the deferred creation callback, or the subsequent property-change callback decrements
    // from zero.
    #[test]
    fn property_value_count_tracks_created_value_before_same_tick_change() {
        let mut context = Context::new();
        let observed = Rc::new(RefCell::new(Vec::new()));
        let observed_clone = Rc::clone(&observed);

        #[derive(IxaEvent)]
        struct InfectiousThresholdReached {
            count: usize,
            direction: Direction,
        }

        context.register_trigger(
            PropertyValueCountTrigger::<Person, InfectionStatus>::decreases_to(
                InfectionStatus::Infectious,
                0,
            )
            .repeating()
            .emit_with(|event| InfectiousThresholdReached {
                count: event.count,
                direction: event.direction,
            }),
        );
        context.subscribe_to_event(move |_context, event: InfectiousThresholdReached| {
            observed_clone
                .borrow_mut()
                .push((event.count, event.direction));
        });

        let person = context
            .add_entity(with!(Person, InfectionStatus::Infectious))
            .unwrap();
        context.set_property(person, InfectionStatus::Susceptible);
        context.execute();

        assert_eq!(*observed.borrow(), vec![(0, Direction::Decreasing)]);
    }

    // Regression coverage for create-with-untracked-value followed by change-to-tracked before
    // callbacks drain. A deferred creation callback that reads the later tracked value must not
    // count the entity again when the property-change callback drains.
    #[test]
    fn property_value_count_does_not_double_count_same_tick_change_to_tracked() {
        let mut context = Context::new();
        let observed = Rc::new(RefCell::new(Vec::new()));
        let observed_clone = Rc::clone(&observed);

        #[derive(IxaEvent)]
        struct InfectiousThresholdReached {
            count: usize,
            direction: Direction,
        }

        context.register_trigger(
            PropertyValueCountTrigger::<Person, InfectionStatus>::changes_to(
                InfectionStatus::Infectious,
                0,
            )
            .repeating()
            .emit_with(|event| InfectiousThresholdReached {
                count: event.count,
                direction: event.direction,
            }),
        );
        context.subscribe_to_event(move |_context, event: InfectiousThresholdReached| {
            observed_clone
                .borrow_mut()
                .push((event.count, event.direction));
        });

        let person = context.add_entity(Person).unwrap();
        context.set_property(person, InfectionStatus::Infectious);
        context.execute();

        context.set_property(person, InfectionStatus::Susceptible);
        context.execute();

        assert_eq!(*observed.borrow(), vec![(0, Direction::Decreasing)]);
    }

    #[test]
    #[should_panic(expected = "period must be greater than 0")]
    fn periodic_time_trigger_zero_period_panics() {
        let _ = PeriodicTimeTrigger::every(0.0);
    }

    #[test]
    #[should_panic(expected = "period must be greater than 0")]
    fn periodic_time_trigger_negative_period_panics() {
        let _ = PeriodicTimeTrigger::every(-1.0);
    }

    #[test]
    #[should_panic(expected = "period must be greater than 0")]
    fn periodic_time_trigger_nan_period_panics() {
        let _ = PeriodicTimeTrigger::every(f64::NAN);
    }

    #[test]
    #[should_panic(expected = "period must be greater than 0")]
    fn periodic_time_trigger_infinite_period_panics() {
        let _ = PeriodicTimeTrigger::every(f64::INFINITY);
    }

    #[test]
    #[should_panic(expected = "period must be greater than 0")]
    fn periodic_time_trigger_every_with_phase_validates_period() {
        let _ = PeriodicTimeTrigger::every_with_phase(0.0, ExecutionPhase::Last);
    }

    #[test]
    #[should_panic(expected = "delay must be greater than or equal to 0")]
    fn periodic_time_trigger_negative_delay_panics() {
        let _ = PeriodicTimeTrigger::every(1.0).start_with_delay(-1.0);
    }

    #[test]
    #[should_panic(expected = "delay must be greater than or equal to 0")]
    fn periodic_time_trigger_nan_delay_panics() {
        let _ = PeriodicTimeTrigger::every(1.0).start_with_delay(f64::NAN);
    }

    #[test]
    #[should_panic(expected = "delay must be greater than or equal to 0")]
    fn periodic_time_trigger_infinite_delay_panics() {
        let _ = PeriodicTimeTrigger::every(1.0).start_with_delay(f64::INFINITY);
    }

    #[test]
    #[should_panic(expected = "cannot be NaN")]
    fn periodic_time_trigger_nan_start_time_panics() {
        let _ = PeriodicTimeTrigger::every(1.0).start_at(f64::NAN);
    }

    #[test]
    #[should_panic(expected = "cannot be infinite")]
    fn periodic_time_trigger_infinite_start_time_panics() {
        let _ = PeriodicTimeTrigger::every(1.0).start_at(f64::INFINITY);
    }

    #[test]
    #[should_panic(expected = "cannot be less than the current time")]
    fn periodic_time_trigger_start_at_past_panics() {
        let mut context = Context::new();
        context.add_plan(1.0, |_| {});
        context.execute();

        context.register_trigger(
            PeriodicTimeTrigger::every(1.0)
                .start_at(0.5)
                .emit_value::<CaseThresholdReached>(CaseThresholdReached),
        );
    }
}
