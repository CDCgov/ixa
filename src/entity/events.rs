/*!

`EntityCreatedEvent` and `EntityPropertyChangeEvent` types in analogy to `PersonCreatedEvent` and `PersonPropertyChangeEvent`.

*/

use ixa_derive::IxaEvent;

use crate::entity::property::Property;
use crate::entity::{Entity, EntityId};
use crate::IxaEvent;

/// Emitted when a new entity is created.
/// These should not be emitted outside this module.
#[derive(Clone, Copy, IxaEvent)]
#[allow(clippy::manual_non_exhaustive)]
pub struct EntityCreatedEvent<E: Entity> {
    /// The [`EntityId<E>`] of the new entity.
    pub entity_id: EntityId<E>,
}

impl<E: Entity> EntityCreatedEvent<E> {
    pub fn new(entity_id: EntityId<E>) -> Self {
        Self { entity_id }
    }
}

/// Emitted when a property is updated.
/// These should not be emitted outside this module.
#[derive(Copy, Clone, IxaEvent)]
#[allow(clippy::manual_non_exhaustive)]
pub struct PropertyChangeEvent<E: Entity, P: Property<E>> {
    /// The [`EntityId<E>`] that changed
    pub entity_id: EntityId<E>,
    /// The new value
    pub current: P,
    /// The old value
    pub previous: Option<P>,
}

// impl<E: Entity, P: Property<E>> IxaEvent for PropertyChangeEvent<E, P> {
//     fn on_subscribe(context: &mut Context) {
//         if P::is_derived() {
//             context.register_property::<T>();
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use serde_derive::Serialize;

    use super::*;
    use crate::{define_entity, define_property, Context};

    define_entity!(Person);

    define_property!(struct Age(u8), Person);

    #[derive(Serialize, Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub enum AgeGroupValue {
        Child,
        Adult,
    }
    // define_global_property!(Threshold, u8);

    // define_derived_property!(AgeGroup, AgeGroupValue, [Age], |age| {
    //     if age < 18 {
    //         AgeGroupValue::Child
    //     } else {
    //         AgeGroupValue::Adult
    //     }
    // });

    define_property!(
        enum RiskCategory {
            High,
            Low,
        },
        Person
    );

    define_property!(struct IsRunner(bool), Person, default_const = IsRunner(false));

    define_property!(struct RunningShoes(u8), Person);

    #[test]
    fn observe_entity_addition() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(move |_context, event: EntityCreatedEvent<Person>| {
            *flag_clone.borrow_mut() = true;
            assert_eq!(event.entity_id.0, 0);
        });

        let _ = context.add_entity::<Person, _>(()).unwrap();
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
                    Some(RiskCategory::Low),
                    "Previous value is correct"
                );
                assert_eq!(
                    event.current,
                    RiskCategory::High,
                    "Current value is correct"
                );
            },
        );

        let person_id = context.add_entity((RiskCategory::Low,)).unwrap();

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
        let person_id = context.add_entity(()).unwrap();
        // Initializer called as a side effect of set, so event fires.
        context.set_property(person_id, RunningShoes(42));
        context.execute();
        assert!(*flag.borrow());
    }

    /*
    #[test]
    fn get_entity_property_change_event() {
        let mut context = Context::new();
        let person = context.add_entity((Age(17),)).unwrap();

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
    */

    #[test]
    fn test_person_property_change_event_no_people() {
        let mut context = Context::new();
        // Non derived person property -- no problems
        context.subscribe_to_event(|_context, _event: PropertyChangeEvent<Person, IsRunner>| {
            unreachable!();
        });

        // Derived person property -- can't add an event without people being present
        // context.subscribe_to_event(|_context, _event: PropertyChangeEvent<Person, AgeGroup>| {
        //     unreachable!();
        // });
    }
}
