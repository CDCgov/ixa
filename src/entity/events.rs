/*!

Entity events report creation, explicitly supplied initial values, and later property changes:

- [`EntityCreatedEvent`] is emitted when an entity has been created.
- [`PropertyInitializedEvent`] is emitted for each non-default, non-derived property value
  explicitly supplied during creation.
- [`PropertyChangeEvent`] is emitted when a property is subsequently updated.

Client code can subscribe to these events with the `Context::subscribe_to_event<IxaEvent>(handler)` method:

```rust,ignore
// Suppose `InfectionStatus` is a property of the entity `Person`.
// A type alias for property change events makes code more concise and readable.
pub type InfectionStatusEvent = PropertyChangeEvent<Person, InfectionStatus>;
// Suppose we want to execute the following function whenever `InfectionStatus` changes.
fn handle_infection_status_change(context: &mut Context, event: InfectionStatusEvent){
    // ... handle the infection status change event ...
}
// We do so by subscribing to this event.
context.subscribe_to_event::<InfectionStatusEvent>(handle_infection_status_change);

// Observe a non-default value explicitly supplied when a person is created.
context.subscribe_to_event(
    |_context, event: PropertyInitializedEvent<Person, InfectionStatus>| {
        // ... use event.entity_id and event.value ...
    },
);
```


A non-derived property sits on the type-erased side of the boundary of its dependent's `PropertyValueStore`, so it
needs to somehow trigger the creation of and emit the change events for its dependents in a type-erased way.

Property change events are triggered and collected on the outside of the type-erased `PropertyValueStore` boundary,
because a non-derived p

*/

use smallbox::space::S4;
use smallbox::SmallBox;

use crate::entity::property::{Property, PropertyInitializationKind};
use crate::entity::{ContextEntitiesExt, Entity, EntityId};
use crate::{Context, IxaEvent};

// We choose the size parameter for `PartialPropertyChangeEventBox` based on the assumption that
// most properties are 64 bits or fewer. The concrete object behind `PartialPropertyChangeEventBox`
// (the alias for the `SmallBox`) is `PartialPropertyChangeEventCore<E, P>`, which is
// `#[repr(transparent)]` over `PropertyChangeEvent<E, P>`. That event stores
//
// - `EntityId<E>`, one `usize`, so 8 bytes.
// - `current`: a property value, typically <= 8 bytes
// - `current`: a property value, typically <= 8 bytes
//
// That puts the payload at 24 bytes, with 8-byte alignment. The `S4` size is 4 `usize`s of inline
// storage, i.e. 32 bytes, and inline storage is used when the payload size and alignment fit. So
// `S4` comfortably holds the common 24-byte case inline with 8 bytes of slack.
pub(crate) type PartialPropertyChangeEventBox = SmallBox<dyn PartialPropertyChangeEvent, S4>;

/// Type-erased interface to `PartialPropertyChangeEvent<E, P>`.
/// Interacts with the index on behalf of the erased type.
pub(crate) trait PartialPropertyChangeEvent {
    /// Updates the index with the current property value and emits a change event.
    fn emit_in_context(&mut self, context: &mut Context);
}

impl<E: Entity, P: Property<E>> PartialPropertyChangeEvent
    for PartialPropertyChangeEventCore<E, P>
{
    /// Updates the index with the current property value and emits a change event.
    fn emit_in_context(&mut self, context: &mut Context) {
        self.0.current = context.get_property(self.0.entity_id);

        {
            // Update value change counters
            let property_value_store = context.get_property_value_store::<E, P>();
            if self.0.current != self.0.previous {
                for counter in &property_value_store.value_change_counters {
                    counter
                        .borrow_mut()
                        .update(self.0.entity_id, self.0.current, context);
                }
            }
        }

        // Now update the indexes
        let property_value_store = context.get_property_value_store_mut::<E, P>();
        if let Some(index) = property_value_store.index.as_mut() {
            // Out with the old
            index.remove_entity(&self.0.previous, self.0.entity_id);
            // In with the new
            index.add_entity(&self.0.current, self.0.entity_id);
        }

        // We decided not to do the following check.
        // See `src/entity/context_extension::ContextEntitiesExt::set_property`.
        // if self.0.current != self.0.previous {
        //     context.emit_event(self.to_event());
        // }

        context.emit_event(self.to_event());
    }
}

/// Represents a partially created `PropertyChangeEvent` of a derived property during the computation of property
/// changes during the update of one of its non-derived property dependencies.
///
/// A `PartialPropertyChangeEventCore<E, P>` is layout-compatible with
/// `PropertyChangeEvent<E, P>`, so converting via `to_event()` does not require an extra heap
/// allocation.
#[repr(transparent)]
pub(crate) struct PartialPropertyChangeEventCore<E: Entity, P: Property<E>>(
    PropertyChangeEvent<E, P>,
);
// We provide blanket impls for these because the compiler isn't smart enough to know
// `PartialPropertyChangeEvent<E, P>` is always `Copy`/`Clone` if we derive them.
impl<E: Entity, P: Property<E>> Clone for PartialPropertyChangeEventCore<E, P> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<E: Entity, P: Property<E>> Copy for PartialPropertyChangeEventCore<E, P> {}

impl<E: Entity, P: Property<E>> PartialPropertyChangeEventCore<E, P> {
    pub fn new(entity_id: EntityId<E>, previous_value: P) -> Self {
        Self(PropertyChangeEvent {
            entity_id,
            current: previous_value,
            previous: previous_value,
        })
    }

    pub fn to_event(self) -> PropertyChangeEvent<E, P> {
        self.0
    }
}

/// Emitted when a new entity is created.
/// These should not be emitted outside this module.
pub struct EntityCreatedEvent<E: Entity> {
    /// The [`EntityId<E>`] of the new entity.
    pub entity_id: EntityId<E>,
}

// Implemented manually to maintain the subscription flag through the event hooks.
impl<E: Entity> Clone for EntityCreatedEvent<E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<E: Entity> Copy for EntityCreatedEvent<E> {}

impl<E: Entity> IxaEvent for EntityCreatedEvent<E> {
    fn on_subscribe(context: &mut Context) {
        context.entity_store.items[E::id()].entity_created_event_subscribed = true;
    }

    fn on_unsubscribe(context: &mut Context) {
        if !context.has_event_handlers::<Self>() {
            context.entity_store.items[E::id()].entity_created_event_subscribed = false;
        }
    }
}

impl<E: Entity> EntityCreatedEvent<E> {
    #[must_use]
    pub fn new(entity_id: EntityId<E>) -> Self {
        Self { entity_id }
    }
}

/// Emitted for a non-default, non-derived property value supplied when an entity is created.
pub struct PropertyInitializedEvent<E: Entity, P: Property<E>> {
    /// The newly created entity.
    pub entity_id: EntityId<E>,
    /// The initial property value supplied by the caller.
    pub value: P,
}

// Implemented manually to maintain the subscription bit set through the event hooks.
impl<E: Entity, P: Property<E>> Clone for PropertyInitializedEvent<E, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<E: Entity, P: Property<E>> Copy for PropertyInitializedEvent<E, P> {}

impl<E: Entity, P: Property<E>> IxaEvent for PropertyInitializedEvent<E, P> {
    fn on_subscribe(context: &mut Context) {
        match P::initialization_kind() {
            PropertyInitializationKind::Explicit | PropertyInitializationKind::Constant => {
                context
                    .entity_store
                    .get_property_store_mut::<E>()
                    .property_initialized_event_subscriptions
                    .set(P::id());
            }
            PropertyInitializationKind::Derived => {}
        }
    }

    fn on_unsubscribe(context: &mut Context) {
        match P::initialization_kind() {
            PropertyInitializationKind::Explicit | PropertyInitializationKind::Constant
                if !context.has_event_handlers::<Self>() =>
            {
                context
                    .entity_store
                    .get_property_store_mut::<E>()
                    .property_initialized_event_subscriptions
                    .reset(P::id());
            }
            PropertyInitializationKind::Explicit
            | PropertyInitializationKind::Constant
            | PropertyInitializationKind::Derived => {}
        }
    }
}

impl<E: Entity, P: Property<E>> PropertyInitializedEvent<E, P> {
    #[must_use]
    pub fn new(entity_id: EntityId<E>, value: P) -> Self {
        Self { entity_id, value }
    }
}

/// Emitted when a property is updated.
/// These should not be emitted outside this module.
#[derive(IxaEvent)]
pub struct PropertyChangeEvent<E: Entity, P: Property<E>> {
    /// The [`EntityId<E>`] that changed
    pub entity_id: EntityId<E>,
    /// The new value
    pub current: P,
    /// The old value
    pub previous: P,
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use super::*;
    use crate::{define_derived_property, define_entity, define_property, with, Context};

    define_entity!(Person);

    define_property!(struct Age(u8), Person );

    // define_global_property!(Threshold, u8);

    // An enum
    define_derived_property!(
        enum AgeGroup {
            Child,
            Adult,
        },
        Person,
        [Age], // Depends only on age
        [],    // No global dependencies
        |age| {
            let age: Age = age;
            if age.0 < 18 {
                AgeGroup::Child
            } else {
                AgeGroup::Adult
            }
        }
    );

    define_property!(
        enum RiskCategory {
            High,
            Low,
        },
        Person
    );

    define_property!(struct IsRunner(bool), Person, default_const = IsRunner(false));

    define_property!(struct RunningShoes(u8), Person );

    define_entity!(InitializationPerson);

    define_property!(
        struct InitConstant(u8),
        InitializationPerson,
        default_const = InitConstant(0)
    );

    define_property!(
        struct InitSecond(u8),
        InitializationPerson,
        default_const = InitSecond(0)
    );

    define_property!(struct InitRequired(u8), InitializationPerson);

    define_entity!(OtherInitializationPerson);
    crate::impl_property!(
        InitConstant,
        OtherInitializationPerson,
        default_const = InitConstant(0)
    );

    define_derived_property!(
        struct InitDerived(u8),
        InitializationPerson,
        [InitConstant],
        [],
        |value| {
            let value: InitConstant = value;
            InitDerived(value.0 + 1)
        }
    );

    #[test]
    fn initialization_event_contains_nondefault_constant_value() {
        let mut context = Context::new();
        let received = Rc::new(RefCell::new(None));
        let received_clone = received.clone();
        context.subscribe_to_event(
            move |context, event: PropertyInitializedEvent<InitializationPerson, InitConstant>| {
                assert_eq!(
                    context.get_property::<InitializationPerson, InitConstant>(event.entity_id),
                    event.value
                );
                *received_clone.borrow_mut() = Some((event.entity_id, event.value));
            },
        );

        let entity_id = context
            .add_entity(with!(
                InitializationPerson,
                InitConstant(7),
                InitRequired(11)
            ))
            .unwrap();
        context.execute();

        assert_eq!(*received.borrow(), Some((entity_id, InitConstant(7))));
    }

    #[test]
    fn required_property_initialization_emits_without_default() {
        let mut context = Context::new();
        let received = Rc::new(RefCell::new(None));
        let received_clone = received.clone();
        context.subscribe_to_event(
            move |_context, event: PropertyInitializedEvent<InitializationPerson, InitRequired>| {
                *received_clone.borrow_mut() = Some(event.value);
            },
        );

        context
            .add_entity(with!(InitializationPerson, InitRequired(23)))
            .unwrap();
        context.execute();

        assert_eq!(*received.borrow(), Some(InitRequired(23)));
    }

    #[test]
    fn explicit_constant_default_does_not_emit_initialization_event() {
        let mut context = Context::new();
        context.subscribe_to_event(
            |_context, _event: PropertyInitializedEvent<InitializationPerson, InitConstant>| {
                panic!("the explicit default must not emit");
            },
        );

        context
            .add_entity(with!(
                InitializationPerson,
                InitConstant(0),
                InitRequired(1)
            ))
            .unwrap();
        context.execute();
    }

    #[test]
    fn omitted_constant_default_does_not_emit_initialization_event() {
        let mut context = Context::new();
        context.subscribe_to_event(
            |_context, _event: PropertyInitializedEvent<InitializationPerson, InitConstant>| {
                panic!("an omitted property must not emit");
            },
        );

        context
            .add_entity(with!(InitializationPerson, InitRequired(1)))
            .unwrap();
        context.execute();
    }

    #[test]
    fn derived_property_does_not_emit_initialization_event() {
        let mut context = Context::new();
        context.subscribe_to_event(
            |_context, _event: PropertyInitializedEvent<InitializationPerson, InitDerived>| {
                panic!("a dependent derived property must not emit");
            },
        );

        context
            .add_entity(with!(
                InitializationPerson,
                InitConstant(9),
                InitRequired(1)
            ))
            .unwrap();
        context.execute();
    }

    #[test]
    fn initialization_events_are_queued_before_creation() {
        let mut context = Context::new();
        context.index_property::<InitializationPerson, InitConstant>();
        context.index_property_counts::<InitializationPerson, InitSecond>();

        let order = Rc::new(RefCell::new(Vec::new()));
        let order_clone = order.clone();
        context.subscribe_to_event(
            move |context, event: PropertyInitializedEvent<InitializationPerson, InitConstant>| {
                assert_eq!(
                    context.query_entity_count(with!(InitializationPerson, event.value)),
                    1
                );
                assert_eq!(
                    context.query_entity_count(with!(InitializationPerson, InitSecond(4))),
                    1
                );
                order_clone.borrow_mut().push("constant");
            },
        );

        let order_clone = order.clone();
        context.subscribe_to_event(
            move |_context, _event: PropertyInitializedEvent<InitializationPerson, InitSecond>| {
                order_clone.borrow_mut().push("second");
            },
        );

        let order_clone = order.clone();
        context.subscribe_to_event(
            move |_context,
                  _event: PropertyInitializedEvent<InitializationPerson, InitRequired>| {
                order_clone.borrow_mut().push("required");
            },
        );

        let order_clone = order.clone();
        context.subscribe_to_event(
            move |_context, _event: EntityCreatedEvent<InitializationPerson>| {
                order_clone.borrow_mut().push("created");
            },
        );

        context
            .add_entity(with!(
                InitializationPerson,
                InitConstant(3),
                InitSecond(4),
                InitRequired(5)
            ))
            .unwrap();
        assert!(order.borrow().is_empty());

        context.execute();
        let order = order.borrow();
        assert_eq!(order.len(), 4);
        assert_eq!(order[3], "created");
        assert!(order[..3].contains(&"constant"));
        assert!(order[..3].contains(&"second"));
        assert!(order[..3].contains(&"required"));
    }

    #[test]
    fn property_initialized_subscription_cache_tracks_listener_lifecycle() {
        let mut context = Context::new();
        let property_id = <InitConstant as Property<InitializationPerson>>::id();

        let first = context.subscribe_to_event(
            |_context, _event: PropertyInitializedEvent<InitializationPerson, InitConstant>| {},
        );
        assert!(context
            .entity_store
            .get_property_store::<InitializationPerson>()
            .property_initialized_event_subscriptions
            .get(property_id));

        let second = context.subscribe_to_event(
            |_context, _event: PropertyInitializedEvent<InitializationPerson, InitConstant>| {},
        );
        assert!(context.unsubscribe_from_event(&first));
        assert!(context
            .entity_store
            .get_property_store::<InitializationPerson>()
            .property_initialized_event_subscriptions
            .get(property_id));

        assert!(context.unsubscribe_from_event(&second));
        assert!(!context
            .entity_store
            .get_property_store::<InitializationPerson>()
            .property_initialized_event_subscriptions
            .get(property_id));
    }

    #[test]
    fn property_initialized_subscription_cache_ignores_failed_unsubscription() {
        let mut context = Context::new();
        let listener = context.subscribe_to_event(
            |_context, _event: PropertyInitializedEvent<InitializationPerson, InitConstant>| {},
        );
        let property_id = <InitConstant as Property<InitializationPerson>>::id();

        assert!(context.unsubscribe_from_event(&listener));
        assert!(!context.unsubscribe_from_event(&listener));
        assert!(!context
            .entity_store
            .get_property_store::<InitializationPerson>()
            .property_initialized_event_subscriptions
            .get(property_id));
    }

    #[test]
    fn property_initialized_subscription_cache_ignores_derived_property() {
        let mut context = Context::new();
        context.subscribe_to_event(
            |_context, _event: PropertyInitializedEvent<InitializationPerson, InitDerived>| {},
        );

        assert!(context
            .entity_store
            .get_property_store::<InitializationPerson>()
            .property_initialized_event_subscriptions
            .is_empty());
    }

    #[test]
    fn property_initialized_subscription_cache_tracks_distinct_properties() {
        let mut context = Context::new();
        context.subscribe_to_event(
            |_context, _event: PropertyInitializedEvent<InitializationPerson, InitConstant>| {},
        );
        context.subscribe_to_event(
            |_context, _event: PropertyInitializedEvent<InitializationPerson, InitRequired>| {},
        );

        let subscriptions = &context
            .entity_store
            .get_property_store::<InitializationPerson>()
            .property_initialized_event_subscriptions;
        assert!(subscriptions.get(<InitConstant as Property<InitializationPerson>>::id()));
        assert!(subscriptions.get(InitRequired::id()));
    }

    #[test]
    fn property_initialized_subscription_cache_is_scoped_to_entity_type() {
        let mut context = Context::new();
        context.subscribe_to_event(
            |_context, _event: PropertyInitializedEvent<InitializationPerson, InitConstant>| {},
        );

        assert!(!context
            .entity_store
            .get_property_store::<InitializationPerson>()
            .property_initialized_event_subscriptions
            .is_empty());
        assert!(context
            .entity_store
            .get_property_store::<OtherInitializationPerson>()
            .property_initialized_event_subscriptions
            .is_empty());
    }

    #[test]
    fn property_initialized_subscription_cache_ignores_unrelated_event() {
        let mut context = Context::new();
        context.subscribe_to_event(|_context, _event: EntityCreatedEvent<InitializationPerson>| {});

        assert!(context
            .entity_store
            .get_property_store::<InitializationPerson>()
            .property_initialized_event_subscriptions
            .is_empty());
    }

    #[test]
    fn property_initialized_subscription_cache_controls_delivery() {
        let mut context = Context::new();
        let count = Rc::new(Cell::new(0));
        let count_clone = count.clone();
        let listener = context.subscribe_to_event(
            move |_context,
                  _event: PropertyInitializedEvent<InitializationPerson, InitRequired>| {
                count_clone.set(count_clone.get() + 1);
            },
        );

        context
            .add_entity(with!(
                InitializationPerson,
                InitConstant(1),
                InitRequired(1)
            ))
            .unwrap();
        context.execute();
        assert_eq!(count.get(), 1);

        assert!(context.unsubscribe_from_event(&listener));
        context
            .add_entity(with!(
                InitializationPerson,
                InitConstant(2),
                InitRequired(2)
            ))
            .unwrap();
        context.execute();
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn entity_created_subscription_cache_tracks_listener_lifecycle() {
        let mut context = Context::new();
        assert!(
            !context
                .entity_store
                .new_entity_id::<InitializationPerson>()
                .1
        );

        let first = context
            .subscribe_to_event(|_context, _event: EntityCreatedEvent<InitializationPerson>| {});
        assert!(
            context
                .entity_store
                .new_entity_id::<InitializationPerson>()
                .1
        );

        let second = context
            .subscribe_to_event(|_context, _event: EntityCreatedEvent<InitializationPerson>| {});
        assert!(context.unsubscribe_from_event(&first));
        assert!(
            context
                .entity_store
                .new_entity_id::<InitializationPerson>()
                .1
        );

        assert!(context.unsubscribe_from_event(&second));
        assert!(
            !context
                .entity_store
                .new_entity_id::<InitializationPerson>()
                .1
        );
    }

    #[test]
    fn entity_created_subscription_cache_ignores_failed_unsubscription() {
        let mut context = Context::new();
        let listener = context
            .subscribe_to_event(|_context, _event: EntityCreatedEvent<InitializationPerson>| {});

        assert!(context.unsubscribe_from_event(&listener));
        assert!(!context.unsubscribe_from_event(&listener));
        assert!(
            !context
                .entity_store
                .new_entity_id::<InitializationPerson>()
                .1
        );
    }

    #[test]
    fn entity_created_subscription_cache_is_scoped_to_entity_type() {
        let mut context = Context::new();
        context.subscribe_to_event(|_context, _event: EntityCreatedEvent<InitializationPerson>| {});

        assert!(
            context
                .entity_store
                .new_entity_id::<InitializationPerson>()
                .1
        );
        assert!(
            !context
                .entity_store
                .new_entity_id::<OtherInitializationPerson>()
                .1
        );
    }

    #[test]
    fn entity_created_subscription_cache_ignores_unrelated_event() {
        let mut context = Context::new();
        context.subscribe_to_event(
            |_context, _event: PropertyInitializedEvent<InitializationPerson, InitConstant>| {},
        );

        assert!(
            !context
                .entity_store
                .new_entity_id::<InitializationPerson>()
                .1
        );
    }

    #[test]
    fn entity_created_subscription_cache_controls_delivery() {
        let mut context = Context::new();
        let count = Rc::new(Cell::new(0));
        let count_clone = count.clone();
        let listener = context.subscribe_to_event(
            move |_context, _event: EntityCreatedEvent<InitializationPerson>| {
                count_clone.set(count_clone.get() + 1);
            },
        );

        context
            .add_entity(with!(InitializationPerson, InitRequired(1)))
            .unwrap();
        context.execute();
        assert_eq!(count.get(), 1);

        assert!(context.unsubscribe_from_event(&listener));
        context
            .add_entity(with!(InitializationPerson, InitRequired(2)))
            .unwrap();
        context.execute();
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn entity_created_subscription_cache_preserves_listener_order() {
        let mut context = Context::new();
        let order = Rc::new(RefCell::new(Vec::new()));

        let order_clone = order.clone();
        context.subscribe_to_event(
            move |_context, _event: EntityCreatedEvent<InitializationPerson>| {
                order_clone.borrow_mut().push(1);
            },
        );
        let order_clone = order.clone();
        context.subscribe_to_event(
            move |_context, _event: EntityCreatedEvent<InitializationPerson>| {
                order_clone.borrow_mut().push(2);
            },
        );

        context
            .add_entity(with!(InitializationPerson, InitRequired(1)))
            .unwrap();
        context.execute();

        assert_eq!(*order.borrow(), vec![1, 2]);
    }

    #[test]
    fn initialization_event_runs_once_per_subscriber() {
        let mut context = Context::new();
        let count = Rc::new(Cell::new(0));
        for _ in 0..2 {
            let count_clone = count.clone();
            context.subscribe_to_event(
                move |_context,
                      _event: PropertyInitializedEvent<InitializationPerson, InitConstant>| {
                    count_clone.set(count_clone.get() + 1);
                },
            );
        }

        context
            .add_entity(with!(
                InitializationPerson,
                InitConstant(2),
                InitRequired(1)
            ))
            .unwrap();
        context.execute();

        assert_eq!(count.get(), 2);
    }

    #[test]
    fn repeated_creation_emits_matching_entity_ids_and_values() {
        let mut context = Context::new();
        let received = Rc::new(RefCell::new(Vec::new()));
        let received_clone = received.clone();
        context.subscribe_to_event(
            move |_context, event: PropertyInitializedEvent<InitializationPerson, InitConstant>| {
                received_clone
                    .borrow_mut()
                    .push((event.entity_id, event.value));
            },
        );

        let first = context
            .add_entity(with!(
                InitializationPerson,
                InitConstant(5),
                InitRequired(1)
            ))
            .unwrap();
        let second = context
            .add_entity(with!(
                InitializationPerson,
                InitConstant(8),
                InitRequired(2)
            ))
            .unwrap();
        context.execute();

        assert_eq!(
            *received.borrow(),
            vec![(first, InitConstant(5)), (second, InitConstant(8))]
        );
    }

    #[test]
    fn observe_entity_addition() {
        let mut context = Context::new();
        context.index_property::<Person, Age>();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(move |context, event: EntityCreatedEvent<Person>| {
            let matching = context.query(with!(Person, Age(18)));
            assert!(matching.contains(event.entity_id));
            *flag_clone.borrow_mut() = true;
            assert_eq!(event.entity_id.0, 0);
        });

        let _ = context
            .add_entity::<Person, _>(with!(Person, Age(18), RunningShoes(33), RiskCategory::Low))
            .unwrap();
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn observe_entity_property_change() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, event: PropertyChangeEvent<Person, RiskCategory>| {
                *flag_clone.borrow_mut() = true;
                assert_eq!(event.entity_id.0, 0, "Entity id is correct");
                assert_eq!(
                    event.previous,
                    RiskCategory::Low,
                    "Previous value is correct"
                );
                assert_eq!(
                    event.current,
                    RiskCategory::High,
                    "Current value is correct"
                );
            },
        );

        let person_id = context
            .add_entity(with!(Person, Age(9), RunningShoes(33), RiskCategory::Low))
            .unwrap();

        context.set_property(person_id, RiskCategory::High);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn observe_entity_property_change_with_set() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, _event: PropertyChangeEvent<Person, RunningShoes>| {
                *flag_clone.borrow_mut() = true;
            },
        );
        // Does not emit a change event.
        let person_id = context
            .add_entity(with!(Person, Age(9), RunningShoes(33), RiskCategory::Low))
            .unwrap();
        // Emits a change event.
        context.set_property(person_id, RunningShoes(42));
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn get_entity_property_change_event() {
        let mut context = Context::new();
        let person = context
            .add_entity(with!(Person, Age(17), RunningShoes(33), RiskCategory::Low))
            .unwrap();

        let flag = Rc::new(RefCell::new(false));

        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, event: PropertyChangeEvent<Person, AgeGroup>| {
                assert_eq!(event.entity_id.0, 0);
                assert_eq!(event.previous, AgeGroup::Child);
                assert_eq!(event.current, AgeGroup::Adult);
                *flag_clone.borrow_mut() = true;
            },
        );
        context.set_property(person, Age(18));
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn test_person_property_change_event_no_people() {
        let mut context = Context::new();
        // Non derived person property -- no problems
        context.subscribe_to_event(|_context, _event: PropertyChangeEvent<Person, IsRunner>| {
            unreachable!();
        });

        // Derived person property -- can't add an event without people being present
        context.subscribe_to_event(|_context, _event: PropertyChangeEvent<Person, AgeGroup>| {
            unreachable!();
        });
    }
}
