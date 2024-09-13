use crate::{context::Context, define_data_plugin};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

// PeopleData represents each unique person in the simulation with an id ranging
// from 0 to population - 1. Person properties are associated with a person
// via their id.
struct PeopleData {
    population: usize,
    properties_map: HashMap<TypeId, Box<dyn Any>>,
}

define_data_plugin!(
    PeoplePlugin,
    PeopleData,
    PeopleData {
        population: 0,
        properties_map: HashMap::new()
    }
);

// Represents a unique person - the id refers to that person's index in the range
// 0 to population - 1 in the PeopleData container.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct PersonId {
    pub id: usize,
}

// Individual characteristics or states related to a person, such as age or
// disease status, are represented as "person properties". These properties
// * are represented by a struct type that implements the PersonProperty trait,
// * specify a Value type to represent the data associated with the property,
// * specify a default value
// They should be defined with the define_person_property! macro.
pub trait PersonProperty: Copy {
    type Value: Copy;
}

#[macro_export]
macro_rules! define_person_property {
    ($person_property:ident, $value:ty) => {
        #[derive(Copy, Clone)]
        pub struct $person_property;

        impl $crate::people::PersonProperty for $person_property {
            type Value = $value;
        }
    };
}
pub use define_person_property;

impl PeopleData {
    /// Adds a person and returns a `PersonId` that can be used to reference them.
    fn add_person(&mut self) -> PersonId {
        let id = self.population;
        self.population += 1;
        PersonId { id }
    }

    /// Retrieves a specific property of a person by their `PersonId`.
    ///
    /// Returns `Option<T::Value>`: `Some(value)` if the property exists for the given person,
    /// or `None` if it doesn't.
    #[allow(clippy::needless_pass_by_value)]
    fn get_person_property<T: PersonProperty + 'static>(
        &self,
        person_id: PersonId,
        _property: T,
    ) -> Option<T::Value> {
        self.properties_map
            .get(&TypeId::of::<T>())
            .and_then(|boxed_vec| {
                boxed_vec
                    .downcast_ref::<Vec<Option<T::Value>>>()
                    .expect("Type mismatch in properties_map")
                    .get(person_id.id)
                    .and_then(|value| value.as_ref().copied())
            })
    }

    /// Sets the value of a property for a person
    #[allow(clippy::needless_pass_by_value)]
    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        _property: T,
        value: T::Value,
    ) {
        let index = person_id.id;
        let vec = self
            .properties_map
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(Vec::<Option<T::Value>>::with_capacity(index)));
        let vec: &mut Vec<Option<T::Value>> = vec.downcast_mut().unwrap();
        if index >= vec.len() {
            vec.resize(index + 1, Option::None);
        }
        vec[index] = Option::Some(value);
    }
}

// Emitted when a new person is created
// These should not be emitted outside this module
#[derive(Clone, Copy)]
#[allow(clippy::manual_non_exhaustive)]
pub struct PersonCreatedEvent {
    pub person_id: PersonId,
}

// Emitted when a person property is updated
// These should not be emitted outside this module
#[derive(Copy, Clone)]
#[allow(clippy::manual_non_exhaustive)]
pub struct PersonPropertyChangeEvent<T: PersonProperty> {
    pub person_id: PersonId,
    pub current: T::Value,
    pub previous: T::Value,
}

pub trait ContextPeopleExt {
    /// Creates a new person with no assigned person properties
    fn add_person(&mut self) -> PersonId;

    /// Given a Persionid, returns the value of a defined person property
    fn get_person_property<T: PersonProperty + 'static>(
        &self,
        person_id: PersonId,
        _property: T,
    ) -> T::Value;

    // Given a `PersonId`, sets the value of a defined person property
    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        _property: T,
        value: T::Value,
    );

    fn set_person_property_default_value<T: PersonProperty + 'static>(
        &mut self,
        property: T,
        value: T::Value,
    ) {
        self.before_person_added(move |context, person_id| {
            context.set_person_property(person_id, property, value);
        });
    }

    fn before_person_added(&mut self, callback: impl Fn(&mut Context, PersonId) + 'static);
}

impl ContextPeopleExt for Context {
    fn add_person(&mut self) -> PersonId {
        let person_id = self.get_data_container_mut(PeoplePlugin).add_person();

        self.emit_event(PersonCreatedEvent { person_id });
        person_id
    }

    fn get_person_property<T: PersonProperty + 'static>(
        &self,
        person_id: PersonId,
        property: T,
    ) -> T::Value {
        self.get_data_container(PeoplePlugin)
            .expect("PeoplePlguin is not initialized")
            .get_person_property(person_id, property)
            .expect("Property not initialized")
    }

    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        let data_container = self.get_data_container_mut(PeoplePlugin);

        let current_value = data_container.get_person_property(person_id, property);
        match current_value {
            // The person property is already set, so we emit a change event
            Some(current_value) => {
                let change_event: PersonPropertyChangeEvent<T> = PersonPropertyChangeEvent {
                    person_id,
                    current: value,
                    previous: current_value,
                };
                data_container.set_person_property(person_id, property, value);
                self.emit_event(change_event);
            }
            // The person property is not yet initialized, so we don't emit
            // any events.
            None => {
                data_container.set_person_property(person_id, property, value);
            }
        }
    }

    fn before_person_added(&mut self, callback: impl Fn(&mut Context, PersonId) + 'static) {
        self.subscribe_immediately_to_event(move |context, event: PersonCreatedEvent| {
            let person_id = event.person_id;
            callback(context, person_id);
        });
    }
}

#[cfg(test)]
mod test {
    use super::{ContextPeopleExt, PersonCreatedEvent, PersonPropertyChangeEvent};
    use crate::context::Context;
    use std::{cell::RefCell, rc::Rc};

    define_person_property!(Age, u8);
    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    pub enum RiskCategory {
        High,
        Low,
    }
    define_person_property!(RiskCategoryType, RiskCategory);

    #[test]
    fn observe_person_addition() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(move |_context, event: PersonCreatedEvent| {
            *flag_clone.borrow_mut() = true;
            assert_eq!(event.person_id.id, 0);
        });

        let _ = context.add_person();
        context.execute();
        assert!(*flag.borrow());
    }
    #[test]
    fn set_get_properties() {
        let mut context = Context::new();

        let person = context.add_person();
        context.set_person_property(person, Age, 42);
        assert_eq!(context.get_person_property(person, Age), 42);
    }

    #[test]
    #[should_panic = "Property not initialized"]
    fn get_uninitialized_property() {
        let mut context = Context::new();
        let person = context.add_person();
        context.get_person_property(person, Age);
    }

    #[test]
    fn set_property_resize() {
        let mut context = Context::new();

        // Create a bunch of people and don't initialize Age
        let first_person = context.add_person();
        for _ in 1..9 {
            let _person = context.add_person();
        }
        let tenth_person = context.add_person();

        // This will fill up the earlier people in the Age vec with None values
        context.set_person_property(tenth_person, Age, 42);

        // Now we set up a listener for change events
        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(move |_context, _event: PersonPropertyChangeEvent<Age>| {
            *flag_clone.borrow_mut() = true;
        });

        // This is the first time we're setting the Age property for the first person,
        // so it shouldn't emit a change event.
        context.set_person_property(first_person, Age, 42);
        context.execute();
        assert!(!*flag.borrow());

        // Now the change event should fire
        context.set_person_property(first_person, Age, 43);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn add_person_set_creation_stage() {
        let mut context = Context::new();

        context.before_person_added(move |context, person_id| {
            context.set_person_property(person_id, Age, 42);
            context.set_person_property(person_id, RiskCategoryType, RiskCategory::Low);
        });
        let person_id = context.add_person();
        assert_eq!(context.get_person_property(person_id, Age), 42);
        assert_eq!(
            context.get_person_property(person_id, RiskCategoryType),
            RiskCategory::Low
        );
    }

    #[test]
    fn add_person_set_default_properties() {
        let mut context = Context::new();

        context.set_person_property_default_value(Age, 42);
        let person_id = context.add_person();
        assert_eq!(context.get_person_property(person_id, Age), 42);
    }

    #[test]
    fn property_initialization_should_not_emit_events() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, _event: PersonPropertyChangeEvent<RiskCategoryType>| {
                *flag_clone.borrow_mut() = true;
            },
        );
        // Neither of these initialization patterns should emit a change event
        context.before_person_added(move |context, person_id| {
            context.set_person_property(person_id, Age, 42);
        });
        context.set_person_property_default_value(RiskCategoryType, RiskCategory::Low);

        let _person = context.add_person();
        context.execute();
        assert!(!*flag.borrow());
    }

    #[test]
    fn observe_person_property_change() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, event: PersonPropertyChangeEvent<RiskCategoryType>| {
                *flag_clone.borrow_mut() = true;
                assert_eq!(event.person_id.id, 0, "Person id is correct");
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
        context.set_person_property_default_value(RiskCategoryType, RiskCategory::Low);
        let person_id = context.add_person();
        context.set_person_property(person_id, RiskCategoryType, RiskCategory::High);
        context.execute();
        assert!(*flag.borrow());
    }
}
