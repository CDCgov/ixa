//! Trigger criterion for the count of entities of a given type.
//!
//! [`EntityCountTrigger`] observes
//! [`EntityCreatedEvent`](crate::entity::events::EntityCreatedEvent) for a single entity type and
//! emits when the total number of entities of that type increases to the configured threshold.
//!
//! ## Construction
//!
//! ```rust,ignore
//! EntityCountTrigger::<E>::increases_to(threshold)
//! ```
//!
//! ## Observation
//!
//! The observation data passed to
//! [`TriggerCriterion::emit_with`](super::TriggerCriterion::emit_with) is
//! [`EntityCountTriggerEvent`]:
//!
//! ```rust,ignore
//! pub struct EntityCountTriggerEvent<E: Entity> {
//!     pub entity_id: EntityId<E>,
//!     pub count: usize,
//! }
//! ```
//!
//! ## Semantics
//!
//! As entities can only be created, not destroyed, the count of entities is monotonic. Thus, this
//! criterion does not use [`Direction`](super::Direction) or [`TriggerMode`](super::TriggerMode).
//!
//! - It fires when a creation makes the count equal to the threshold.
//! - The observed count always equals the threshold the trigger was created with.
//! - If the entity population already equals or exceeds the threshold _before_ the trigger is
//!   registered, it will never emit.
//! - A threshold of `0` will not be reached by an entity creation and is therefore not allowed.
//!
//! ## Example
//!
//! ```rust
//! use ixa::{Context, ContextEntitiesExt, define_entity, IxaEvent};
//! use ixa::entity::EntityId;
//! use ixa::triggers::{ContextTriggersExt, EntityCountTrigger, TriggerCriterion};
//! use ixa_derive::IxaEvent;
//!
//! define_entity!(Case);
//!
//! // The event records which case caused us to reach the threshold and
//! // the value of the threshold itself (as `count`).
//! #[derive(IxaEvent)]
//! struct SecondCase {
//!     case_id: EntityId<Case>,
//!     count: usize,
//! }
//!
//! let mut context = Context::new();
//!
//! context.register_trigger(
//!     EntityCountTrigger::increases_to(2)
//!         .emit_with(|observation| SecondCase {
//!             case_id: observation.entity_id,
//!             count: observation.count,
//!         }),
//! );
//!
//! context.subscribe_to_event(|_context, _event: SecondCase| {
//!     // respond when the second Case entity is created
//! });
//! ```
//!
use std::marker::PhantomData;

use super::TriggerCriterion;
use crate::entity::events::EntityCreatedEvent;
use crate::entity::{Entity, EntityId};
use crate::Context;

pub struct EntityCountTrigger<E: Entity> {
    threshold: usize,
    _entity: PhantomData<fn() -> E>,
}

#[derive(Clone, Copy, Debug)]
pub struct EntityCountTriggerEvent<E: Entity> {
    pub entity_id: EntityId<E>,
    pub count: usize,
}

impl<E: Entity> EntityCountTrigger<E> {
    #[must_use]
    pub fn increases_to(threshold: usize) -> Self {
        assert!(threshold > 0, "threshold must be greater than 0");
        Self {
            threshold,
            _entity: PhantomData,
        }
    }
}

impl<E: Entity> TriggerCriterion for EntityCountTrigger<E> {
    type Observation = EntityCountTriggerEvent<E>;

    fn install<F>(self, context: &mut Context, on_match: F)
    where
        F: Fn(&mut Context, Self::Observation) + 'static,
    {
        let threshold = self.threshold;
        context.subscribe_to_event(move |context, event: EntityCreatedEvent<E>| {
            // Avoids a call to `context.get_entity_count` at the expense of using internal implementation.
            let count = event.entity_id.0 + 1;
            if count == threshold {
                on_match(
                    context,
                    EntityCountTriggerEvent {
                        entity_id: event.entity_id,
                        count,
                    },
                );
            }
        });
    }
}
