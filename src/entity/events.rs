/*!

`EntityCreatedEvent` and `EntityPropertyChangeEvent` types are emitted when an entity is created or an entity's
property value is changed.

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
```


A non-derived property sits on the type-erased side of the boundary of its dependent's `PropertyValueStore`, so it
needs to somehow trigger the creation of and emit the change events for its dependents in a type-erased way.

Property change events are triggered and collected on the outside of the type-erased `PropertyValueStore` boundary,
because a non-derived p

*/

use ixa_derive::IxaEvent;

use crate::entity::property::Property;
use crate::entity::{ContextEntitiesExt, Entity, EntityId};
use crate::{Context, IxaEvent};

/// Type-erased interface to `PartialPropertyChangeEvent<E, P>`.
pub(crate) trait PartialPropertyChangeEvent {
    fn emit_in_context(self: Box<Self>, context: &mut Context);
}

impl<E: Entity, P: Property<E>> PartialPropertyChangeEvent
    for PartialPropertyChangeEventCore<E, P>
{
    /// Updates the index with the current property value and emits a change event.
    fn emit_in_context(self: Box<Self>, context: &mut Context) {
        let current_value: P = context.get_property(self.0.entity_id);
        let property_value_store = context.get_property_value_store_mut::<E, P>();

        property_value_store
            .index
            .add_entity(&current_value.make_canonical(), self.0.entity_id);

        // We decided not to do the following check.
        // See `src/entity/context_extension::ContextEntitiesExt::set_property`.
        // if current_value != self.0.previous {
        //     context.emit_event(self.to_event(current_value));
        // }

        context.emit_event(self.to_event(current_value));
    }
}

/// Represents a partially created `PropertyChangeEvent` of a derived property during the computation of property
/// changes during the update of one of its non-derived property dependencies.
///
/// A `Box<PartialPropertyChangeEventCore<E, P>>` can be transformed into a `Box<PropertyChangeEvent<E, P>>` in place,
/// avoiding an allocation.
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

    pub fn to_event(mut self, current_value: P) -> PropertyChangeEvent<E, P> {
        self.0.current = current_value;
        self.0
    }
}

/// Emitted when a new entity is created.
/// These should not be emitted outside this module.
#[derive(IxaEvent)]
#[allow(clippy::manual_non_exhaustive)]
pub struct EntityCreatedEvent<E: Entity> {
    /// The [`EntityId<E>`] of the new entity.
    pub entity_id: EntityId<E>,
}
// We provide blanket impls for these because the compiler isn't smart enough to know
// this type is always `Copy`/`Clone` if we derive them.
impl<E: Entity> Copy for EntityCreatedEvent<E> {}
impl<E: Entity> Clone for EntityCreatedEvent<E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<E: Entity> EntityCreatedEvent<E> {
    pub fn new(entity_id: EntityId<E>) -> Self {
        Self { entity_id }
    }
}

/// Emitted when a property is updated.
/// These should not be emitted outside this module.
#[derive(IxaEvent)]
#[allow(clippy::manual_non_exhaustive)]
pub struct PropertyChangeEvent<E: Entity, P: Property<E>> {
    /// The [`EntityId<E>`] that changed
    pub entity_id: EntityId<E>,
    /// The new value
    pub current: P,
    /// The old value
    pub previous: P,
}
// We provide blanket impls for these because the compiler isn't smart enough to know
// this type is always `Copy`/`Clone` if we derive them.
impl<E: Entity, P: Property<E>> Clone for PropertyChangeEvent<E, P> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<E: Entity, P: Property<E>> Copy for PropertyChangeEvent<E, P> {}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::{define_derived_property, define_entity, define_property, Context};

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

    #[test]
    fn observe_entity_addition() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(move |_context, event: EntityCreatedEvent<Person>| {
            *flag_clone.borrow_mut() = true;
            assert_eq!(event.entity_id.0, 0);
        });

        let _ = context
            .add_entity::<Person, _>((Age(18), RunningShoes(33), RiskCategory::Low))
            .unwrap();
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn add_entity_with_subscriber_fires_exactly_one_event() {
        let mut context = Context::new();

        let event_count = Rc::new(RefCell::new(0usize));
        let event_count_clone = event_count.clone();
        context.subscribe_to_event(move |ctx, event: EntityCreatedEvent<Person>| {
            *event_count_clone.borrow_mut() += 1;
            // Properties are accessible in the handler because events are deferred until execute().
            assert_eq!(ctx.get_property::<Person, Age>(event.entity_id), Age(42));
            assert_eq!(
                ctx.get_property::<Person, RiskCategory>(event.entity_id),
                RiskCategory::High
            );
        });

        let id = context
            .add_entity::<Person, _>((Age(42), RunningShoes(7), RiskCategory::High))
            .unwrap();
        assert_eq!(id, PersonId::new(0));
        assert_eq!(
            *event_count.borrow(),
            0,
            "event must not fire before execute()"
        );

        context.execute();
        assert_eq!(*event_count.borrow(), 1, "exactly one event must fire");
    }

    #[test]
    fn add_entity_without_subscriber_creates_entity_correctly() {
        // Verifies the has_event_handlers fast path: no subscriber means no event emission,
        // but the entity must still be created with correct properties and count.
        let mut context = Context::new();

        let id = context
            .add_entity::<Person, _>((Age(30), RunningShoes(10), RiskCategory::Low))
            .unwrap();

        context.execute();

        assert_eq!(id, PersonId::new(0));
        assert_eq!(context.get_entity_count::<Person>(), 1);
        assert_eq!(context.get_property::<Person, Age>(id), Age(30));
        assert_eq!(
            context.get_property::<Person, RiskCategory>(id),
            RiskCategory::Low
        );
    }

    #[test]
    fn observe_bulk_entity_addition_order() {
        let mut context = Context::new();

        let seen_ids = Rc::new(RefCell::new(Vec::<usize>::new()));
        let seen_ids_clone = seen_ids.clone();
        context.subscribe_to_event(move |_context, event: EntityCreatedEvent<Person>| {
            seen_ids_clone.borrow_mut().push(event.entity_id.0);
        });

        let ids = context
            .add_entities::<Person, _, _>([
                (Age(18), RunningShoes(33), RiskCategory::Low),
                (Age(19), RunningShoes(34), RiskCategory::High),
                (Age(20), RunningShoes(35), RiskCategory::Low),
            ])
            .unwrap();

        context.execute();

        assert_eq!(
            ids,
            vec![PersonId::new(0), PersonId::new(1), PersonId::new(2)]
        );
        assert_eq!(*seen_ids.borrow(), vec![0, 1, 2]);
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
            .add_entity((Age(9), RunningShoes(33), RiskCategory::Low))
            .unwrap();

        context.set_property(person_id, RiskCategory::High);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn observe_entity_property_change_with_set() {
        let mut context = Context::new();

        let event_count = Rc::new(RefCell::new(0usize));
        let event_count_clone = event_count.clone();
        context.subscribe_to_event(
            move |_context, _event: PropertyChangeEvent<Person, RunningShoes>| {
                *event_count_clone.borrow_mut() += 1;
            },
        );
        // Entity creation does not emit property-change events.
        let person_id = context
            .add_entity((Age(9), RunningShoes(33), RiskCategory::Low))
            .unwrap();
        context.execute();
        assert_eq!(*event_count.borrow(), 0);

        // Emits a change event.
        context.set_property(person_id, RunningShoes(42));
        context.execute();
        assert_eq!(*event_count.borrow(), 1);
    }

    #[test]
    fn get_entity_property_change_event() {
        let mut context = Context::new();
        let person = context
            .add_entity((Age(17), RunningShoes(33), RiskCategory::Low))
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
