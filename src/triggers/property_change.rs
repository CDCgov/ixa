//! Trigger criterion for writes to an entity property with particular previous and/or current
//! values.
//!
//! [`PropertyChangeTrigger`] observes
//! [`PropertyChangeEvent`](crate::entity::events::PropertyChangeEvent) for a specific
//! entity/property pair and emits when a property write matches its configured previous value,
//! current value, or both.
//!
//! Construct one with [`PropertyChangeTrigger::from`] to match a previous value,
//! [`PropertyChangeTrigger::to`] to match a current value, or
//! [`PropertyChangeTrigger::from_to`] to require both. Since there is no constructor with both
//! sides omitted, every [`PropertyChangeTrigger`] has at least one value constraint. Call
//! [`PropertyChangeTrigger::once`] to emit only on the first observed matching property change, or
//! [`PropertyChangeTrigger::repeating`] to restore the default behavior of emitting on every
//! observed matching property change.
//!
//! The observation data passed to
//! [`TriggerCriterion::emit_with`](super::TriggerCriterion::emit_with) is
//! [`PropertyChangeTriggerEvent`]. It contains the entity ID, the previous property value, the
//! current property value, and the selected [`TriggerMode`](super::TriggerMode) with which the
//! trigger was created.
//!
//! ## Semantics
//!
//! By default, the criterion uses [`TriggerMode::Repeating`](super::TriggerMode::Repeating) and
//! emits for every matching property write. Call [`PropertyChangeTrigger::once`] to emit only for
//! the first matching write, or [`PropertyChangeTrigger::repeating`] to return to the default
//! repeating behavior.
//!
//! A `from` constraint matches `event.previous`; a `to` constraint matches `event.current`;
//! `from_to` requires both. Property writes are eventful even when the old and new values are
//! equal. For example, `PropertyChangeTrigger::to(Alive(false))` can match a write that sets
//! `Alive(false)` when the entity was already `Alive(false)`, and
//! `PropertyChangeTrigger::from_to(Alive(false), Alive(false))` matches that no-op write exactly.
//!
//! ## Example
//!
//! ```rust
//! use ixa::{Context, ContextEntitiesExt, define_entity, define_property, IxaEvent};
//! use ixa::entity::EntityId;
//! use ixa::triggers::{ContextTriggersExt, PropertyChangeTrigger, TriggerCriterion};
//! use ixa_derive::IxaEvent;
//!
//! define_entity!(Person);
//! define_property!(struct Alive(bool), Person, default_const = Alive(true));
//!
//! #[derive(IxaEvent)]
//! struct FirstDeath {
//!     person: EntityId<Person>
//! }
//!
//! let mut context = Context::new();
//!
//! context.register_trigger(
//!     PropertyChangeTrigger::from_to(Alive(true), Alive(false))
//!         .once()
//!         .emit_with(|observation| FirstDeath {
//!             person: observation.entity_id
//!         }),
//! );
//!
//! context.subscribe_to_event(|_context, _event: FirstDeath| {
//!     // respond when a person changes from alive to dead
//! });
//! ```
//!
use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use super::{TriggerCriterion, TriggerMode};
use crate::entity::events::PropertyChangeEvent;
use crate::entity::property::Property;
use crate::entity::{Entity, EntityId};
use crate::Context;

pub struct PropertyChangeTrigger<E, P>
where
    E: Entity,
    P: Property<E>,
{
    from: Option<P>,
    to: Option<P>,
    mode: TriggerMode,
    _entity: PhantomData<fn() -> E>,
}

#[derive(Clone, Copy, Debug)]
pub struct PropertyChangeTriggerEvent<E, P>
where
    E: Entity,
    P: Property<E>,
{
    pub entity_id: EntityId<E>,
    pub previous: P,
    pub current: P,
    pub mode: TriggerMode,
}

impl<E, P> PropertyChangeTrigger<E, P>
where
    E: Entity,
    P: Property<E>,
{
    #[must_use]
    pub fn from(from: P) -> Self {
        Self {
            from: Some(from),
            to: None,
            mode: TriggerMode::Repeating,
            _entity: PhantomData,
        }
    }

    #[must_use]
    pub fn to(to: P) -> Self {
        Self {
            from: None,
            to: Some(to),
            mode: TriggerMode::Repeating,
            _entity: PhantomData,
        }
    }

    #[must_use]
    pub fn from_to(from: P, to: P) -> Self {
        Self {
            from: Some(from),
            to: Some(to),
            mode: TriggerMode::Repeating,
            _entity: PhantomData,
        }
    }

    #[must_use]
    pub fn once(mut self) -> Self {
        self.mode = TriggerMode::Once;
        self
    }

    #[must_use]
    pub fn repeating(mut self) -> Self {
        self.mode = TriggerMode::Repeating;
        self
    }
}

impl<E, P> TriggerCriterion for PropertyChangeTrigger<E, P>
where
    E: Entity,
    P: Property<E>,
{
    type Observation = PropertyChangeTriggerEvent<E, P>;

    fn install<F>(self, context: &mut Context, on_match: F)
    where
        F: Fn(&mut Context, Self::Observation) + 'static,
    {
        match self.mode {
            TriggerMode::Once => {
                let active = Rc::new(Cell::new(true));
                context.subscribe_to_event(move |context, event: PropertyChangeEvent<E, P>| {
                    if !active.get() {
                        return;
                    }
                    let from_matches = self.from.is_none_or(|from| event.previous == from);
                    let to_matches = self.to.is_none_or(|to| event.current == to);
                    if from_matches && to_matches {
                        on_match(
                            context,
                            PropertyChangeTriggerEvent {
                                entity_id: event.entity_id,
                                previous: event.previous,
                                current: event.current,
                                mode: self.mode,
                            },
                        );
                        active.set(false);
                    }
                });
            }
            TriggerMode::Repeating => {
                context.subscribe_to_event(move |context, event: PropertyChangeEvent<E, P>| {
                    let from_matches = self.from.is_none_or(|from| event.previous == from);
                    let to_matches = self.to.is_none_or(|to| event.current == to);
                    if from_matches && to_matches {
                        on_match(
                            context,
                            PropertyChangeTriggerEvent {
                                entity_id: event.entity_id,
                                previous: event.previous,
                                current: event.current,
                                mode: self.mode,
                            },
                        );
                    }
                });
            }
        }
    }
}
