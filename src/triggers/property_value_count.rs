use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

use super::{Direction, TriggerCriterion, TriggerMode};
use crate::entity::events::{PropertyChangeEvent, PropertyInitializedEvent};
use crate::entity::property::Property;
use crate::entity::{ContextEntitiesExt, Entity, EntityId};
use crate::{define_data_plugin, Context, EntityPropertyTuple, EventListenerId, HashMap};

/// Trigger criterion for the count of entities with a particular property value.
///
/// [`PropertyValueCountTrigger`] observes
/// [`PropertyInitializedEvent`](crate::entity::events::PropertyInitializedEvent) and
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
/// [`PropertyValueCountTrigger::repeating`] to return to the default repeating behavior. A `once`
/// criterion stops observing events after it emits.
///
/// Entity creation can cause a crossing if the new entity's initial value is the tracked value.
/// The initial value is the one at creation (supplied, default, or computed for a derived
/// property), even if it is changed before the initialization event is handled. Property writes
/// can cause a crossing when they move an entity into or out
/// of the tracked value. A no-op write where `previous == current` still emits a property-change
/// event at the entity layer, but it does not change this trigger's tracked count and therefore
/// cannot by itself cross the threshold.
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
        let count = context.query_entity_count(EntityPropertyTuple::<E, _>::new((self.value,)));
        let (dispatcher, is_new) = Dispatcher::<E, P>::get_or_subscribe(context);
        let slot = Rc::new(Slot {
            value: self.value,
            threshold: self.threshold,
            direction_filter: self.direction_filter,
            mode: self.mode,
            count: Cell::new(count),
            state: Cell::new(if is_new {
                SlotState::Active
            } else {
                SlotState::Pending
            }),
            on_match: Box::new(on_match),
        });
        if !is_new {
            // Events already queued were emitted before `count` was measured, so the slot must
            // skip them. They are all ahead of this callback in the FIFO queue.
            let slot = Rc::clone(&slot);
            context.queue_callback(move |_context| {
                if slot.state.get() == SlotState::Pending {
                    slot.state.set(SlotState::Active);
                }
            });
        }
        dispatcher.slots.borrow_mut().push(slot);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SlotState {
    Pending,
    Active,
    Retired,
}

type OnMatch<E, P> = Box<dyn Fn(&mut Context, PropertyValueCountTriggerEvent<E, P>)>;

struct Slot<E: Entity, P: Property<E>> {
    value: P,
    threshold: usize,
    direction_filter: Option<Direction>,
    mode: TriggerMode,
    count: Cell<usize>,
    state: Cell<SlotState>,
    on_match: OnMatch<E, P>,
}

impl<E: Entity, P: Property<E>> Slot<E, P> {
    /// Applies one entity's transition and returns whether the slot retired.
    fn observe(
        &self,
        context: &mut Context,
        entity_id: EntityId<E>,
        previous: Option<P>,
        current: P,
    ) -> bool {
        if self.state.get() != SlotState::Active {
            return false;
        }
        let previous_count = self.count.get();
        let (current_count, direction) = match (previous == Some(self.value), current == self.value)
        {
            (false, true) => (previous_count + 1, Direction::Increasing),
            (true, false) => {
                debug_assert!(
                    previous_count > 0,
                    "Ixa internal error: property value count trigger observed a decrease below zero"
                );
                (previous_count.saturating_sub(1), Direction::Decreasing)
            }
            _ => return false,
        };
        self.count.set(current_count);
        if current_count != self.threshold
            || !self
                .direction_filter
                .is_none_or(|filter| filter == direction)
        {
            return false;
        }
        (self.on_match)(
            context,
            PropertyValueCountTriggerEvent {
                entity_id,
                value: self.value,
                count: current_count,
                direction_filter: self.direction_filter,
                direction,
                mode: self.mode,
            },
        );
        if self.mode == TriggerMode::Once {
            self.state.set(SlotState::Retired);
            return true;
        }
        false
    }
}

struct Listeners<E: Entity, P: Property<E>> {
    initialized: EventListenerId<PropertyInitializedEvent<E, P>>,
    changed: EventListenerId<PropertyChangeEvent<E, P>>,
}

/// Shared event subscriptions for every `PropertyValueCountTrigger<E, P>`, so each event costs
/// one callback rather than one per trigger.
struct Dispatcher<E: Entity, P: Property<E>> {
    slots: RefCell<Vec<Rc<Slot<E, P>>>>,
    listeners: RefCell<Option<Listeners<E, P>>>,
}

define_data_plugin!(
    DispatcherPlugin,
    HashMap<TypeId, Rc<dyn Any>>,
    HashMap::default()
);

impl<E: Entity, P: Property<E>> Dispatcher<E, P> {
    fn get_or_subscribe(context: &mut Context) -> (Rc<Self>, bool) {
        let key = TypeId::of::<Self>();
        if let Some(existing) = context.get_data(DispatcherPlugin).get(&key) {
            let existing = Rc::clone(existing)
                .downcast::<Self>()
                .expect("Ixa internal error: trigger dispatcher type mismatch");
            return (existing, false);
        }

        let dispatcher = Rc::new(Self {
            slots: RefCell::new(Vec::new()),
            listeners: RefCell::new(None),
        });
        let initialized = context.subscribe_to_event({
            let dispatcher = Rc::clone(&dispatcher);
            move |context, event: PropertyInitializedEvent<E, P>| {
                dispatcher.dispatch(context, event.entity_id, None, event.value);
            }
        });
        let changed = context.subscribe_to_event({
            let dispatcher = Rc::clone(&dispatcher);
            move |context, event: PropertyChangeEvent<E, P>| {
                dispatcher.dispatch(
                    context,
                    event.entity_id,
                    Some(event.previous),
                    event.current,
                );
            }
        });
        *dispatcher.listeners.borrow_mut() = Some(Listeners {
            initialized,
            changed,
        });
        context
            .get_data_mut(DispatcherPlugin)
            .insert(key, Rc::clone(&dispatcher) as Rc<dyn Any>);
        (dispatcher, true)
    }

    fn dispatch(
        &self,
        context: &mut Context,
        entity_id: EntityId<E>,
        previous: Option<P>,
        current: P,
    ) {
        // Slots registered while dispatching are pending, so the length can be fixed up front.
        // The vector is not borrowed across `on_match`, which may register more triggers.
        let len = self.slots.borrow().len();
        let mut retired = false;
        for index in 0..len {
            let slot = Rc::clone(&self.slots.borrow()[index]);
            retired |= slot.observe(context, entity_id, previous, current);
        }
        if retired {
            self.remove_retired(context);
        }
    }

    fn remove_retired(&self, context: &mut Context) {
        let is_empty = {
            let mut slots = self.slots.borrow_mut();
            slots.retain(|slot| slot.state.get() != SlotState::Retired);
            slots.is_empty()
        };
        if !is_empty {
            return;
        }
        // Callbacks already queued still hold this dispatcher, but it has no slots left. A later
        // trigger registration creates a fresh dispatcher.
        let listeners = self.listeners.borrow_mut().take();
        if let Some(listeners) = listeners {
            context.unsubscribe_from_event(&listeners.initialized);
            context.unsubscribe_from_event(&listeners.changed);
            context
                .get_data_mut(DispatcherPlugin)
                .remove(&TypeId::of::<Self>());
        }
    }
}
