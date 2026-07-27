use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use super::{Direction, TriggerCriterion, TriggerMode};
use crate::entity::events::{EntityCreatedEvent, PropertyChangeEvent};
use crate::entity::property::Property;
use crate::entity::{ContextEntitiesExt, Entity, EntityId};
use crate::{Context, EntityPropertyTuple};

/// Trigger criterion for the count of entities with a particular property value.
///
/// [`PropertyValueCountTrigger`] observes
/// [`EntityCreatedEvent`](crate::entity::events::EntityCreatedEvent) and
/// [`PropertyChangeEvent`](crate::entity::events::PropertyChangeEvent) for a specific
/// entity/property pair and emits when the count of entities with a configured property value
/// crosses a configured threshold.
///
/// ## Construction
///
/// ```rust,ignore
/// PropertyValueCountTrigger::<E, P>::increases_to(value, threshold)
/// PropertyValueCountTrigger::<E, P>::decreases_to(value, threshold)
/// PropertyValueCountTrigger::<E, P>::changes_to(value, threshold)
/// PropertyValueCountTrigger::<E, P>::changes_to(value, threshold).once()
/// PropertyValueCountTrigger::<E, P>::changes_to(value, threshold).repeating()
/// ```
///
/// ## Observation
///
/// The observation data passed to
/// [`TriggerCriterion::emit_with`](super::TriggerCriterion::emit_with) is
/// [`PropertyValueCountTriggerEvent`]. It contains the entity ID whose creation or property write
/// caused the crossing, the tracked property value, the new count, the observed
/// [`Direction`](super::Direction), the configured direction filter as `Option<Direction>`, and the
/// selected [`TriggerMode`](super::TriggerMode):
///
/// ```rust,ignore
/// pub struct PropertyValueCountTriggerEvent<E, P>
/// where
///     E: Entity,
///     P: Property<E>,
/// {
///     pub entity_id: EntityId<E>,
///     pub value: P,
///     pub count: usize,
///     pub direction_filter: Option<Direction>,
///     pub direction: Direction,
///     pub mode: TriggerMode,
/// }
/// ```
///
/// ## Semantics
///
/// The initial count is measured when the trigger is registered. The criterion emits only on a
/// later threshold crossing. Since counts change one entity at a time, a crossing occurs when the
/// new count equals the threshold and differs from the previous count. [`Direction::Increasing`]
/// means the count increased to the threshold, while [`Direction::Decreasing`] means the count
/// decreased to the threshold. `changes_to` leaves the direction filter unset and emits for either
/// observed direction. `increases_to` and `decreases_to` set the direction filter to the
/// corresponding observed direction.
///
/// By default, the criterion uses [`TriggerMode::Repeating`](super::TriggerMode::Repeating) and
/// emits every time the count crosses the threshold and passes the configured direction filter. Call
/// [`PropertyValueCountTrigger::once`] to emit only for the first crossing, or
/// [`PropertyValueCountTrigger::repeating`] to return to the default repeating behavior.
///
/// Entity creation can cause a crossing if the new entity has the tracked value. Property writes
/// can cause a crossing when they move an entity into or out of the tracked value. A no-op write
/// where `previous == current` still emits a property-change event at the entity layer, but it does
/// not change this trigger's tracked count and therefore cannot by itself cross the threshold.
///
/// ## Example
///
/// ```rust
/// use ixa::{Context, ContextEntitiesExt, define_entity, define_property, IxaEvent};
/// use ixa::entity::EntityId;
/// use ixa::triggers::{
///     ContextTriggersExt, Direction, PropertyValueCountTrigger, TriggerCriterion, TriggerMode,
/// };
///
/// define_entity!(Person);
/// define_property!(
///     enum InfectionStatus {
///         Susceptible,
///         Infectious,
///     },
///     Person,
///     default_const = InfectionStatus::Susceptible
/// );
///
/// // The event records which person caused us to reach the threshold and
/// // the value of the threshold itself (as `count`).
/// #[derive(IxaEvent)]
/// struct InfectiousThresholdReached {
///     person: EntityId<Person>,
///     count: usize
/// }
///
/// let mut context = Context::new();
///
/// context.register_trigger(
///     PropertyValueCountTrigger::increases_to(
///         InfectionStatus::Infectious,
///         2,
///     ).emit_with(|observation| InfectiousThresholdReached {
///         person: observation.entity_id,
///         count: observation.count
///     }),
/// );
///
/// context.subscribe_to_event(|_context, _event: InfectiousThresholdReached| {
///     // respond when the infectious count crosses from below 2 to at least 2
/// });
/// ```
pub struct PropertyValueCountTrigger<E, P>
where
    E: Entity,
    P: Property<E>,
{
    value: P,
    threshold: usize,
    direction_filter: Option<Direction>,
    mode: TriggerMode,
    _entity: PhantomData<fn() -> E>,
}

#[derive(Clone, Copy, Debug)]
pub struct PropertyValueCountTriggerEvent<E, P>
where
    E: Entity,
    P: Property<E>,
{
    pub entity_id: EntityId<E>,
    pub value: P,
    pub count: usize,
    pub direction_filter: Option<Direction>,
    pub direction: Direction,
    pub mode: TriggerMode,
}

impl<E, P> PropertyValueCountTrigger<E, P>
where
    E: Entity,
    P: Property<E>,
{
    #[must_use]
    pub fn increases_to(value: P, threshold: usize) -> Self {
        Self {
            value,
            threshold,
            direction_filter: Some(Direction::Increasing),
            mode: TriggerMode::Repeating,
            _entity: PhantomData,
        }
    }

    #[must_use]
    pub fn decreases_to(value: P, threshold: usize) -> Self {
        Self {
            value,
            threshold,
            direction_filter: Some(Direction::Decreasing),
            mode: TriggerMode::Repeating,
            _entity: PhantomData,
        }
    }

    #[must_use]
    pub fn changes_to(value: P, threshold: usize) -> Self {
        Self {
            value,
            threshold,
            direction_filter: None,
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

impl<E, P> TriggerCriterion for PropertyValueCountTrigger<E, P>
where
    E: Entity,
    P: Property<E>,
{
    type Observation = PropertyValueCountTriggerEvent<E, P>;

    fn install<F>(self, context: &mut Context, on_match: F)
    where
        F: Fn(&mut Context, Self::Observation) + 'static,
    {
        match self.mode {
            TriggerMode::Once => {
                let state = Rc::new(Cell::new(CountTriggerState {
                    active: true,
                    count: context
                        .query_entity_count(EntityPropertyTuple::<E, _>::new((self.value,))),
                }));
                let on_match = Rc::new(on_match);

                context.subscribe_to_event({
                    let state = Rc::clone(&state);
                    let on_match = Rc::clone(&on_match);
                    move |context, event: EntityCreatedEvent<E>| {
                        let current = context.get_property::<E, P>(event.entity_id);
                        if current == self.value {
                            let mut state_value = state.get();
                            if !state_value.active {
                                return;
                            }
                            let previous_count = state_value.count;
                            state_value.count += 1;
                            let direction = Direction::Increasing;
                            if self.direction_filter != Some(Direction::Decreasing) {
                                let threshold_crossed = state_value.count == self.threshold
                                    && previous_count != state_value.count
                                    && self
                                        .direction_filter
                                        .is_none_or(|filter| filter == direction);
                                if threshold_crossed {
                                    on_match(
                                        context,
                                        PropertyValueCountTriggerEvent {
                                            entity_id: event.entity_id,
                                            value: self.value,
                                            count: state_value.count,
                                            direction_filter: self.direction_filter,
                                            direction,
                                            mode: self.mode,
                                        },
                                    );
                                    state_value.active = false;
                                }
                            }
                            state.set(state_value);
                        }
                    }
                });

                context.subscribe_to_event({
                    let state = Rc::clone(&state);
                    let on_match = Rc::clone(&on_match);
                    move |context, event: PropertyChangeEvent<E, P>| {
                        let mut state_value = state.get();
                        if !state_value.active {
                            return;
                        }
                        let previous_count = state_value.count;
                        state_value.count =
                            match (event.previous == self.value, event.current == self.value) {
                                (false, true) => state_value.count + 1,
                                (true, false) => state_value.count - 1,
                                _ => state_value.count,
                            };
                        let direction = if state_value.count > previous_count {
                            Some(Direction::Increasing)
                        } else if state_value.count < previous_count {
                            Some(Direction::Decreasing)
                        } else {
                            None
                        };
                        if let Some(direction) = direction {
                            let threshold_crossed = state_value.count == self.threshold
                                && self
                                    .direction_filter
                                    .is_none_or(|filter| filter == direction);
                            if threshold_crossed {
                                on_match(
                                    context,
                                    PropertyValueCountTriggerEvent {
                                        entity_id: event.entity_id,
                                        value: self.value,
                                        count: state_value.count,
                                        direction_filter: self.direction_filter,
                                        direction,
                                        mode: self.mode,
                                    },
                                );
                                state_value.active = false;
                            }
                        }
                        state.set(state_value);
                    }
                });
            }
            TriggerMode::Repeating => {
                let count = Rc::new(Cell::new(
                    context.query_entity_count(EntityPropertyTuple::<E, _>::new((self.value,))),
                ));
                let on_match = Rc::new(on_match);

                context.subscribe_to_event({
                    let count = Rc::clone(&count);
                    let on_match = Rc::clone(&on_match);
                    move |context, event: EntityCreatedEvent<E>| {
                        let current = context.get_property::<E, P>(event.entity_id);
                        if current == self.value {
                            let previous_count = count.get();
                            let current_count = previous_count + 1;
                            count.set(current_count);
                            let direction = Direction::Increasing;
                            if self.direction_filter != Some(Direction::Decreasing) {
                                let threshold_crossed = current_count == self.threshold
                                    && previous_count != current_count
                                    && self
                                        .direction_filter
                                        .is_none_or(|filter| filter == direction);
                                if threshold_crossed {
                                    on_match(
                                        context,
                                        PropertyValueCountTriggerEvent {
                                            entity_id: event.entity_id,
                                            value: self.value,
                                            count: current_count,
                                            direction_filter: self.direction_filter,
                                            direction,
                                            mode: self.mode,
                                        },
                                    );
                                }
                            }
                        }
                    }
                });

                context.subscribe_to_event({
                    let count = Rc::clone(&count);
                    let on_match = Rc::clone(&on_match);
                    move |context, event: PropertyChangeEvent<E, P>| {
                        let previous_count = count.get();
                        let current_count =
                            match (event.previous == self.value, event.current == self.value) {
                                (false, true) => previous_count + 1,
                                (true, false) => previous_count - 1,
                                _ => previous_count,
                            };
                        count.set(current_count);
                        let direction = if current_count > previous_count {
                            Some(Direction::Increasing)
                        } else if current_count < previous_count {
                            Some(Direction::Decreasing)
                        } else {
                            None
                        };
                        if let Some(direction) = direction {
                            let threshold_crossed = current_count == self.threshold
                                && self
                                    .direction_filter
                                    .is_none_or(|filter| filter == direction);
                            if threshold_crossed {
                                on_match(
                                    context,
                                    PropertyValueCountTriggerEvent {
                                        entity_id: event.entity_id,
                                        value: self.value,
                                        count: current_count,
                                        direction_filter: self.direction_filter,
                                        direction,
                                        mode: self.mode,
                                    },
                                );
                            }
                        }
                    }
                });
            }
        }
    }
}

#[derive(Clone, Copy)]
struct CountTriggerState {
    active: bool,
    count: usize,
}
