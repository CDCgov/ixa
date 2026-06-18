//! Composite trigger that toggles between inactive and active states.
//!
//! [`TogglingTrigger`] composes two trigger criteria: one activation criterion and one
//! deactivation criterion. The trigger starts inactive by default. When the activation criterion
//! matches while inactive, the trigger becomes active and emits the activation event. Later
//! activation matches are ignored while the trigger remains active. When the deactivation criterion
//! matches while active, the trigger becomes inactive and emits the deactivation event. Later
//! deactivation matches are ignored while the trigger remains inactive.
//!
//! This is useful for thermostat-style hysteresis. For example, a model can activate an
//! intervention when a property-value count reaches a lower threshold and deactivate it when the
//! same count reaches an upper threshold. The thresholds themselves are ordinary criteria; the
//! toggling trigger only gates those criteria by its current active/inactive state.
//!
//! ## Semantics
//!
//! ### Repeating or once
//!
//! The "mode" of the [`TogglingTrigger`] itself controls how long the active/inactive state machine
//! remains enabled. A toggling trigger defaults to
//! [`TriggerMode::Repeating`](super::TriggerMode::Repeating). In repeating mode, it can activate,
//! deactivate, and activate again for as long as its component criteria continue to match.
//!
//! Calling [`TogglingTrigger::once`] sets the toggling trigger to [`TriggerMode::Once`]. For a
//! toggling trigger, "once" means one active period, _not_ one raw criterion match. If the trigger
//! starts inactive, it can emit one activation event and then one deactivation event. After that
//! deactivation event, the toggling trigger is permanently disabled and ignores all later criterion
//! matches. If the trigger starts active with [`TogglingTrigger::initially_active`], the one active
//! period is already in progress; the first accepted deactivation emits the deactivation event and
//! then disables the trigger. (Matches of the underlying criterion ignored because they occur in
//! the wrong active/inactive state do not by themselves disable a `once` toggling trigger.)
//!
//! The mode of the `TogglingTrigger` should not be confused with the mode of each component
//! criterion, which controls how often that individual criterion reports matches to the toggling
//! trigger. In fact, component criteria should almost always be repeating, even when the
//! `TogglingTrigger` itself is configured with [`TogglingTrigger::once`]. If an underlying
//! criterion uses [`TriggerMode::Once`](super::TriggerMode::Once), that criterion can be consumed
//! by a match that the toggling trigger ignores because it occurred in the wrong state. For
//! example, a once-only activation criterion can match while the toggling trigger is already
//! active; the toggling trigger will correctly ignore that activation match, but the activation
//! criterion may never report another match.
//!
//! ## Example
//!
//! ```rust
//! use ixa::{Context, ContextEntitiesExt, define_entity, define_property, IxaEvent};
//! use ixa::triggers::{
//!     ContextTriggersExt, PropertyValueCountTrigger, TogglingTrigger,
//! };
//! use ixa_derive::IxaEvent;
//!
//! define_entity!(Person);
//! define_property!(
//!     enum InfectionStatus {
//!         Susceptible,
//!         Infectious,
//!     },
//!     Person,
//!     default_const = InfectionStatus::Susceptible
//! );
//!
//! #[derive(IxaEvent)]
//! struct InterventionActivated {
//!     count: usize,
//! }
//!
//! #[derive(IxaEvent)]
//! struct InterventionDeactivated {
//!     count: usize,
//! }
//!
//! let mut context = Context::new();
//!
//! context.register_trigger(TogglingTrigger::new(
//!     PropertyValueCountTrigger::<Person, InfectionStatus>::changes_to(
//!         InfectionStatus::Infectious,
//!         10,
//!     ),
//!     |event| InterventionActivated { count: event.count },
//!     PropertyValueCountTrigger::<Person, InfectionStatus>::changes_to(
//!         InfectionStatus::Infectious,
//!         25,
//!     ),
//!     |event| InterventionDeactivated { count: event.count },
//! ));
//! ```

use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::{Context, IxaEvent};

use super::{TriggerCriterion, TriggerMode, TriggerSpec};

/// A complete installable trigger specification that emits activation and deactivation events when
/// its paired criteria cause state changes.
pub struct TogglingTrigger<AC, DC, ActiveEv, InactiveEv, MakeActive, MakeInactive> {
    activation_criterion: AC,
    deactivation_criterion: DC,
    make_active_event: MakeActive,
    make_inactive_event: MakeInactive,
    initially_active: bool,
    mode: TriggerMode,
    _events: PhantomData<fn() -> (ActiveEv, InactiveEv)>,
}

impl<AC, DC, ActiveEv, InactiveEv, MakeActive, MakeInactive>
    TogglingTrigger<AC, DC, ActiveEv, InactiveEv, MakeActive, MakeInactive>
where
    AC: TriggerCriterion,
    DC: TriggerCriterion,
    ActiveEv: IxaEvent,
    InactiveEv: IxaEvent,
    MakeActive: Fn(AC::Observation) -> ActiveEv + 'static,
    MakeInactive: Fn(DC::Observation) -> InactiveEv + 'static,
{
    /// Create a repeating toggling trigger that starts inactive.
    ///
    /// The activation criterion is accepted only while the trigger is inactive, and the
    /// deactivation criterion is accepted only while the trigger is active. Repeating mode means
    /// the trigger remains enabled after deactivation and can run through multiple active periods.
    ///
    /// The component criteria are used as match sources. They should usually be repeating criteria;
    /// configuring a component criterion with its own `.once()` can consume that criterion on a
    /// match that this toggling trigger ignores because it occurred in the wrong state.
    #[must_use]
    pub fn new(
        activation_criterion: AC,
        make_active_event: MakeActive,
        deactivation_criterion: DC,
        make_inactive_event: MakeInactive,
    ) -> Self {
        Self {
            activation_criterion,
            deactivation_criterion,
            make_active_event,
            make_inactive_event,
            initially_active: false,
            mode: TriggerMode::Repeating,
            _events: PhantomData,
        }
    }

    /// Start the trigger in the active state.
    ///
    /// An initially active trigger ignores activation matches until it first accepts a
    /// deactivation match. If the toggling trigger is also configured with [`Self::once`], that
    /// first accepted deactivation completes its one active period and permanently disables it.
    #[must_use]
    pub fn initially_active(mut self) -> Self {
        self.initially_active = true;
        self
    }

    /// Start the trigger in the inactive state.
    ///
    /// This is the default state and is provided as an explicit counterpart to
    /// [`Self::initially_active`]. If the toggling trigger is configured with [`Self::once`], it can
    /// accept one activation and then one deactivation before disabling itself.
    #[must_use]
    pub fn initially_inactive(mut self) -> Self {
        self.initially_active = false;
        self
    }

    /// Run through one active period and then permanently disable the toggling trigger.
    ///
    /// This method sets the mode of the toggling trigger itself. It does not change the mode of the
    /// activation or deactivation criteria supplied to [`Self::new`]. For an initially inactive
    /// trigger, one active period consists of one accepted activation followed by one accepted
    /// deactivation. For an initially active trigger, the active period is already in progress, so
    /// the first accepted deactivation disables the trigger.
    #[must_use]
    pub fn once(mut self) -> Self {
        self.mode = TriggerMode::Once;
        self
    }

    /// Keep the toggling trigger enabled after deactivation so it can activate again.
    ///
    /// This is the default mode. This method sets the mode of the toggling trigger itself. It does
    /// not change the mode of the activation or deactivation criteria supplied to [`Self::new`].
    #[must_use]
    pub fn repeating(mut self) -> Self {
        self.mode = TriggerMode::Repeating;
        self
    }
}

impl<AC, DC, ActiveEv, InactiveEv, MakeActive, MakeInactive> TriggerSpec
    for TogglingTrigger<AC, DC, ActiveEv, InactiveEv, MakeActive, MakeInactive>
where
    AC: TriggerCriterion,
    DC: TriggerCriterion,
    ActiveEv: IxaEvent,
    InactiveEv: IxaEvent,
    MakeActive: Fn(AC::Observation) -> ActiveEv + 'static,
    MakeInactive: Fn(DC::Observation) -> InactiveEv + 'static,
{
    fn install_in_context(self, context: &mut Context) {
        let Self {
            activation_criterion,
            deactivation_criterion,
            make_active_event,
            make_inactive_event,
            initially_active,
            mode,
            _events,
        } = self;

        let active = Rc::new(Cell::new(initially_active));
        let enabled = Rc::new(Cell::new(true));

        activation_criterion.install(context, {
            let active = Rc::clone(&active);
            let enabled = Rc::clone(&enabled);
            move |context, observation| {
                if enabled.get() && !active.get() {
                    active.set(true);
                    context.emit_event(make_active_event(observation));
                }
            }
        });

        deactivation_criterion.install(context, {
            let active = Rc::clone(&active);
            let enabled = Rc::clone(&enabled);
            move |context, observation| {
                if enabled.get() && active.get() {
                    active.set(false);
                    context.emit_event(make_inactive_event(observation));
                    if mode == TriggerMode::Once {
                        enabled.set(false);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use ixa_derive::IxaEvent;

    use super::*;
    use crate::entity::EntityId;
    use crate::{define_entity, define_property, Context, ContextEntitiesExt, IxaEvent};

    use super::super::{
        ContextTriggersExt, Direction, EntityCountTrigger, PropertyChangeTrigger,
        PropertyValueCountTrigger, TimeTrigger,
    };

    define_entity!(TogglePerson);
    define_entity!(ToggleCase);

    define_property!(
        enum ToggleStatus {
            Susceptible,
            Infectious,
        },
        TogglePerson,
        default_const = ToggleStatus::Susceptible
    );

    define_property!(struct ToggleAlive(bool), TogglePerson, default_const = ToggleAlive(true));

    #[test]
    fn toggling_trigger_gates_property_change_criteria() {
        let mut context = Context::new();
        let observed = Rc::new(RefCell::new(Vec::new()));

        #[derive(IxaEvent)]
        struct Activated {
            previous: ToggleAlive,
            current: ToggleAlive,
        }

        #[derive(IxaEvent)]
        struct Deactivated {
            previous: ToggleAlive,
            current: ToggleAlive,
        }

        context.register_trigger(TogglingTrigger::new(
            PropertyChangeTrigger::<TogglePerson, ToggleAlive>::to(ToggleAlive(false)),
            |event| Activated {
                previous: event.previous,
                current: event.current,
            },
            PropertyChangeTrigger::<TogglePerson, ToggleAlive>::to(ToggleAlive(true)),
            |event| Deactivated {
                previous: event.previous,
                current: event.current,
            },
        ));

        context.subscribe_to_event({
            let observed = Rc::clone(&observed);
            move |_context, event: Activated| {
                observed
                    .borrow_mut()
                    .push(("active", event.previous.0, event.current.0));
            }
        });
        context.subscribe_to_event({
            let observed = Rc::clone(&observed);
            move |_context, event: Deactivated| {
                observed
                    .borrow_mut()
                    .push(("inactive", event.previous.0, event.current.0));
            }
        });

        let person = context.add_entity(TogglePerson).unwrap();
        context.set_property(person, ToggleAlive(false));
        context.set_property(person, ToggleAlive(false));
        context.set_property(person, ToggleAlive(true));
        context.set_property(person, ToggleAlive(true));
        context.set_property(person, ToggleAlive(false));
        context.execute();

        assert_eq!(
            *observed.borrow(),
            vec![
                ("active", true, false),
                ("inactive", false, true),
                ("active", true, false)
            ]
        );
    }

    #[test]
    fn toggling_trigger_can_start_active() {
        let mut context = Context::new();
        let observed = Rc::new(RefCell::new(Vec::new()));

        #[derive(IxaEvent)]
        struct Activated;

        #[derive(IxaEvent)]
        struct Deactivated;

        context.register_trigger(
            TogglingTrigger::new(
                PropertyChangeTrigger::<TogglePerson, ToggleAlive>::to(ToggleAlive(false)),
                |_| Activated,
                PropertyChangeTrigger::<TogglePerson, ToggleAlive>::to(ToggleAlive(true)),
                |_| Deactivated,
            )
            .initially_active(),
        );

        context.subscribe_to_event({
            let observed = Rc::clone(&observed);
            move |_context, _event: Activated| {
                observed.borrow_mut().push("active");
            }
        });
        context.subscribe_to_event({
            let observed = Rc::clone(&observed);
            move |_context, _event: Deactivated| {
                observed.borrow_mut().push("inactive");
            }
        });

        let person = context.add_entity(TogglePerson).unwrap();
        context.set_property(person, ToggleAlive(false));
        context.set_property(person, ToggleAlive(true));
        context.set_property(person, ToggleAlive(false));
        context.execute();

        assert_eq!(*observed.borrow(), vec!["inactive", "active"]);
    }

    #[test]
    fn toggling_trigger_once_disables_after_one_full_active_period() {
        let mut context = Context::new();
        let observed = Rc::new(RefCell::new(Vec::new()));

        #[derive(IxaEvent)]
        struct Activated;

        #[derive(IxaEvent)]
        struct Deactivated;

        context.register_trigger(
            TogglingTrigger::new(
                PropertyChangeTrigger::<TogglePerson, ToggleAlive>::to(ToggleAlive(false)),
                |_| Activated,
                PropertyChangeTrigger::<TogglePerson, ToggleAlive>::to(ToggleAlive(true)),
                |_| Deactivated,
            )
            .once(),
        );

        context.subscribe_to_event({
            let observed = Rc::clone(&observed);
            move |_context, _event: Activated| {
                observed.borrow_mut().push("active");
            }
        });
        context.subscribe_to_event({
            let observed = Rc::clone(&observed);
            move |_context, _event: Deactivated| {
                observed.borrow_mut().push("inactive");
            }
        });

        let person = context.add_entity(TogglePerson).unwrap();
        context.set_property(person, ToggleAlive(true));
        context.set_property(person, ToggleAlive(false));
        context.set_property(person, ToggleAlive(false));
        context.set_property(person, ToggleAlive(true));
        context.set_property(person, ToggleAlive(false));
        context.set_property(person, ToggleAlive(true));
        context.execute();

        assert_eq!(*observed.borrow(), vec!["active", "inactive"]);
    }

    #[test]
    fn toggling_trigger_once_initially_active_disables_after_first_deactivation() {
        let mut context = Context::new();
        let observed = Rc::new(RefCell::new(Vec::new()));

        #[derive(IxaEvent)]
        struct Activated;

        #[derive(IxaEvent)]
        struct Deactivated;

        context.register_trigger(
            TogglingTrigger::new(
                PropertyChangeTrigger::<TogglePerson, ToggleAlive>::to(ToggleAlive(false)),
                |_| Activated,
                PropertyChangeTrigger::<TogglePerson, ToggleAlive>::to(ToggleAlive(true)),
                |_| Deactivated,
            )
            .initially_active()
            .once(),
        );

        context.subscribe_to_event({
            let observed = Rc::clone(&observed);
            move |_context, _event: Activated| {
                observed.borrow_mut().push("active");
            }
        });
        context.subscribe_to_event({
            let observed = Rc::clone(&observed);
            move |_context, _event: Deactivated| {
                observed.borrow_mut().push("inactive");
            }
        });

        let person = context.add_entity(TogglePerson).unwrap();
        context.set_property(person, ToggleAlive(false));
        context.set_property(person, ToggleAlive(true));
        context.set_property(person, ToggleAlive(false));
        context.set_property(person, ToggleAlive(true));
        context.execute();

        assert_eq!(*observed.borrow(), vec!["inactive"]);
    }

    #[test]
    fn toggling_trigger_supports_distinct_observation_and_event_types() {
        let mut context = Context::new();
        let observed_case_count = Rc::new(Cell::new(0));
        let observed_time = Rc::new(Cell::new(0.0));

        #[derive(IxaEvent)]
        struct CasesActivated {
            count: usize,
        }

        #[derive(IxaEvent)]
        struct TimeDeactivated {
            time: f64,
        }

        context.register_trigger(TogglingTrigger::new(
            EntityCountTrigger::<ToggleCase>::increases_to(1),
            |event| CasesActivated { count: event.count },
            TimeTrigger::at(1.0),
            |event| TimeDeactivated { time: event.time },
        ));

        context.subscribe_to_event({
            let observed_case_count = Rc::clone(&observed_case_count);
            move |_context, event: CasesActivated| {
                observed_case_count.set(event.count);
            }
        });
        context.subscribe_to_event({
            let observed_time = Rc::clone(&observed_time);
            move |_context, event: TimeDeactivated| {
                observed_time.set(event.time);
            }
        });

        context.add_entity(ToggleCase).unwrap();
        context.execute();

        assert_eq!(observed_case_count.get(), 1);
        assert_eq!(observed_time.get(), 1.0);
    }

    #[test]
    fn toggling_trigger_applies_property_value_count_hysteresis() {
        let mut context = Context::new();
        let observed = Rc::new(RefCell::new(Vec::new()));

        #[derive(IxaEvent)]
        struct Activated {
            count: usize,
            direction: Direction,
        }

        #[derive(IxaEvent)]
        struct Deactivated {
            count: usize,
            direction: Direction,
        }

        context.register_trigger(TogglingTrigger::new(
            PropertyValueCountTrigger::<TogglePerson, ToggleStatus>::changes_to(
                ToggleStatus::Infectious,
                2,
            ),
            |event| Activated {
                count: event.count,
                direction: event.direction,
            },
            PropertyValueCountTrigger::<TogglePerson, ToggleStatus>::changes_to(
                ToggleStatus::Infectious,
                4,
            ),
            |event| Deactivated {
                count: event.count,
                direction: event.direction,
            },
        ));

        context.subscribe_to_event({
            let observed = Rc::clone(&observed);
            move |_context, event: Activated| {
                observed
                    .borrow_mut()
                    .push(("active", event.count, event.direction));
            }
        });
        context.subscribe_to_event({
            let observed = Rc::clone(&observed);
            move |_context, event: Deactivated| {
                observed
                    .borrow_mut()
                    .push(("inactive", event.count, event.direction));
            }
        });

        let first = context.add_entity(TogglePerson).unwrap();
        let second = context.add_entity(TogglePerson).unwrap();
        let third = context.add_entity(TogglePerson).unwrap();
        let fourth = context.add_entity(TogglePerson).unwrap();

        context.add_plan(0.1, move |context| {
            context.set_property(first, ToggleStatus::Infectious);
        });
        context.add_plan(0.2, move |context| {
            context.set_property(second, ToggleStatus::Infectious);
        });
        context.add_plan(0.3, move |context| {
            context.set_property(second, ToggleStatus::Susceptible);
        });
        context.add_plan(0.4, move |context| {
            context.set_property(second, ToggleStatus::Infectious);
        });
        context.add_plan(0.5, move |context| {
            context.set_property(third, ToggleStatus::Infectious);
        });
        context.add_plan(0.6, move |context| {
            context.set_property(fourth, ToggleStatus::Infectious);
        });
        context.add_plan(0.7, move |context| {
            context.set_property(fourth, ToggleStatus::Susceptible);
        });
        context.add_plan(0.8, move |context| {
            context.set_property(fourth, ToggleStatus::Infectious);
        });
        context.add_plan(0.9, move |context| {
            context.set_property(fourth, ToggleStatus::Susceptible);
        });
        context.add_plan(1.0, move |context| {
            context.set_property(third, ToggleStatus::Susceptible);
        });
        context.add_plan(1.1, move |context| {
            context.set_property(first, ToggleStatus::Susceptible);
        });
        context.add_plan(1.2, move |context| {
            context.set_property(first, ToggleStatus::Infectious);
        });
        context.add_plan(1.3, move |context| {
            context.set_property(third, ToggleStatus::Infectious);
        });
        context.add_plan(1.4, move |context| {
            context.set_property(fourth, ToggleStatus::Infectious);
        });

        context.execute();

        assert_eq!(
            *observed.borrow(),
            vec![
                ("active", 2, Direction::Increasing),
                ("inactive", 4, Direction::Increasing),
                ("active", 2, Direction::Decreasing),
                ("inactive", 4, Direction::Increasing),
            ]
        );
    }

    #[test]
    fn toggling_trigger_can_report_entity_ids_from_observations() {
        let mut context = Context::new();
        let observed = Rc::new(Cell::new(None));

        #[derive(IxaEvent)]
        struct Activated {
            entity_id: EntityId<TogglePerson>,
        }

        #[derive(IxaEvent)]
        struct Deactivated;

        context.register_trigger(TogglingTrigger::new(
            PropertyChangeTrigger::<TogglePerson, ToggleAlive>::to(ToggleAlive(false)),
            |event| Activated {
                entity_id: event.entity_id,
            },
            PropertyChangeTrigger::<TogglePerson, ToggleAlive>::to(ToggleAlive(true)),
            |_| Deactivated,
        ));

        context.subscribe_to_event({
            let observed = Rc::clone(&observed);
            move |_context, event: Activated| {
                observed.set(Some(event.entity_id));
            }
        });

        let person = context.add_entity(TogglePerson).unwrap();
        context.set_property(person, ToggleAlive(false));
        context.execute();

        assert_eq!(observed.get(), Some(person));
    }
}
