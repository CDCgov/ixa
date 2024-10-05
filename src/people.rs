use crate::{context::Context, define_data_plugin};
use std::{
    any::{Any, TypeId},
    cell::{RefCell, RefMut},
    collections::HashMap,
    fmt,
};

// PeopleData represents each unique person in the simulation with an id ranging
// from 0 to population - 1. Person properties are associated with a person
// via their id.
struct PeopleData {
    current_population: usize,
    properties_map: RefCell<HashMap<TypeId, Box<dyn Any>>>,
}

define_data_plugin!(
    PeoplePlugin,
    PeopleData,
    PeopleData {
        current_population: 0,
        properties_map: RefCell::new(HashMap::new())
    }
);

// Represents a unique person - the id refers to that person's index in the range
// 0 to population - 1 in the PeopleData container.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct PersonId {
    id: usize,
}

impl fmt::Debug for PersonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Person {}", self.id)
    }
}

// Individual characteristics or states related to a person, such as age or
// disease status, are represented as "person properties". These properties
// * are represented by a struct type that implements the PersonProperty trait,
// * specify a Value type to represent the data associated with the property,
// * specify an initializer, which returns the initial value
// They may be defined with the define_person_property! macro.
pub trait PersonProperty: Copy {
    type Value: Copy;
    fn initialize(context: &Context, person_id: PersonId) -> Self::Value;
}

/// Defines a person property with the following parameters:
/// * `$person_property`: A name for the identifier type of the property
/// * `$value`: The type of the property's value
/// * `$initialize`: (Optional) A function that takes a `Context` and `PersonId` and
///   returns the initial value. If it is not defined, calling `get_person_property`
///   on the property without explicitly setting a value first will panic.
#[macro_export]
macro_rules! define_person_property {
    ($person_property:ident, $value:ty, $initialize:expr) => {
        #[derive(Copy, Clone)]
        pub struct $person_property;
        impl $crate::people::PersonProperty for $person_property {
            type Value = $value;
            fn initialize(
                _context: &$crate::context::Context,
                _person: $crate::people::PersonId,
            ) -> Self::Value {
                $initialize(_context, _person)
            }
        }
    };
    ($person_property:ident, $value:ty) => {
        define_person_property!($person_property, $value, |_context, _person_id| {
            panic!("Property not initialized");
        });
    };
}

/// Defines a person property with the following parameters:
/// * `$person_property`: A name for the identifier type of the property
/// * `$value`: The type of the property's value
/// * `$default`: An initial value
#[macro_export]
macro_rules! define_person_property_with_default {
    ($person_property:ident, $value:ty, $default:expr) => {
        define_person_property!($person_property, $value, |_context, _person_id| {
            $default
        });
    };
}

pub use define_person_property;

impl PeopleData {
    /// Adds a person and returns a `PersonId` that can be used to reference them.
    /// This will increment the current population by 1.
    fn add_person(&mut self) -> PersonId {
        let id = self.current_population;
        self.current_population += 1;
        PersonId { id }
    }

    /// Retrieves a specific property of a person by their `PersonId`.
    ///
    /// Returns `RefMut<Option<T::Value>>`: `Some(value)` if the property exists for the given person,
    /// or `None` if it doesn't.
    #[allow(clippy::needless_pass_by_value)]
    fn get_person_property_ref<T: PersonProperty + 'static>(
        &self,
        person: PersonId,
        _property: T,
    ) -> RefMut<Option<T::Value>> {
        let properties_map = self.properties_map.borrow_mut();
        let index = person.id;
        RefMut::map(properties_map, |properties_map| {
            let properties = properties_map
                .entry(TypeId::of::<T>())
                .or_insert_with(|| Box::new(Vec::<Option<T::Value>>::with_capacity(index)));
            let values: &mut Vec<Option<T::Value>> = properties
                .downcast_mut()
                .expect("Type mismatch in properties_map");
            if index >= values.len() {
                values.resize(index + 1, None);
            }
            &mut values[index]
        })
    }

    /// Sets the value of a property for a person
    #[allow(clippy::needless_pass_by_value)]
    fn set_person_property<T: PersonProperty + 'static>(
        &self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        let mut property_ref = self.get_person_property_ref(person_id, property);
        *property_ref = Some(value);
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
    /// Returns the current population size
    fn get_current_population(&self) -> usize;

    /// Creates a new person with no assigned person properties
    fn add_person(&mut self) -> PersonId;

    /// Given a `PersonId` returns the value of a defined person property,
    /// initializing it if it hasn't been set yet. If no initializer is
    /// provided, and the property is not set this will panic
    fn get_person_property<T: PersonProperty + 'static>(
        &self,
        person_id: PersonId,
        _property: T,
    ) -> T::Value;

    /// Given a `PersonId`, initialize the value of a defined person property.
    /// Panics if the property is already initialized. Does not fire a change
    /// event.
    fn initialize_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        _property: T,
        value: T::Value,
    );

    /// Given a `PersonId`, sets the value of a defined person property
    /// Panics if the property is not initialized. Fires a change event.
    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        _property: T,
        value: T::Value,
    );
}

impl ContextPeopleExt for Context {
    fn get_current_population(&self) -> usize {
        self.get_data_container(PeoplePlugin)
            .map_or(0, |data_container| data_container.current_population)
    }

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
        let data_container = self.get_data_container(PeoplePlugin)
            .expect("PeoplePlugin is not initialized; make sure you add a person before accessing properties");

        // Attempt to retrieve the existing value
        if let Some(value) = *data_container.get_person_property_ref(person_id, property) {
            return value;
        }

        // Initialize the property. This does not fire a change event
        let initialized_value = T::initialize(self, person_id);
        data_container.set_person_property(person_id, property, initialized_value);

        initialized_value
    }

    fn initialize_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        let data_container = self.get_data_container(PeoplePlugin)
            .expect("PeoplePlugin is not initialized; make sure you add a person before accessing properties");

        let current_value = *data_container.get_person_property_ref(person_id, property);
        assert!(current_value.is_none(), "Property already initialized");
        data_container.set_person_property(person_id, property, value);
    }

    #[allow(clippy::single_match_else)]
    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        let data_container = self.get_data_container(PeoplePlugin)
            .expect("PeoplePlugin is not initialized; make sure you add a person before accessing properties");

        let current_value = *data_container.get_person_property_ref(person_id, property);
        let previous_value = match current_value {
            Some(current_value) => current_value,
            None => {
                let initialize_value = T::initialize(self, person_id);
                data_container.set_person_property(person_id, property, initialize_value);
                initialize_value
            }
        };

        let change_event: PersonPropertyChangeEvent<T> = PersonPropertyChangeEvent {
            person_id,
            current: value,
            previous: previous_value,
        };
        data_container.set_person_property(person_id, property, value);
        self.emit_event(change_event);
    }
}

#[cfg(test)]
mod test {
    use super::{ContextPeopleExt, PersonCreatedEvent, PersonId, PersonPropertyChangeEvent};
    use crate::{context::Context, people::PeoplePlugin};
    use std::{cell::RefCell, rc::Rc};

    define_person_property!(Age, u8);
    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    pub enum RiskCategory {
        High,
        Low,
    }
    define_person_property!(RiskCategoryType, RiskCategory);
    define_person_property_with_default!(IsRunner, bool, false);
    define_person_property!(RunningShoes, u8, |context: &Context, person: PersonId| {
        let is_runner = context.get_person_property(person, IsRunner);
        if is_runner {
            4
        } else {
            0
        }
    });

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
        context.initialize_person_property(person, Age, 42);
        assert_eq!(context.get_person_property(person, Age), 42);
    }

    #[allow(clippy::should_panic_without_expect)]
    #[test]
    #[should_panic]
    fn get_uninitialized_property_panics() {
        let mut context = Context::new();
        let person = context.add_person();
        context.get_person_property(person, Age);
    }

    // Tests that if we try to set or access a property for an index greater than
    // the current size of the property Vec, the vector will be resized.
    #[test]
    fn set_property_resize() {
        let mut context = Context::new();

        // Add a person and set a property, instantiating the Vec
        let person = context.add_person();
        context.initialize_person_property(person, Age, 8);

        // Create a bunch of people and don't initialize Age
        for _ in 1..9 {
            let _person = context.add_person();
        }
        let tenth_person = context.add_person();

        // Set a person property for a person > index 0
        context.initialize_person_property(tenth_person, Age, 42);
        // Call an initializer
        assert!(!context.get_person_property(tenth_person, IsRunner));
    }

    #[test]
    fn get_current_population() {
        let mut context = Context::new();
        assert_eq!(context.get_current_population(), 0);
        for _ in 0..3 {
            context.add_person();
        }
        assert_eq!(context.get_current_population(), 3);
    }

    #[test]
    fn add_person() {
        let mut context = Context::new();

        let person_id = context.add_person();
        context.initialize_person_property(person_id, Age, 42);
        context.initialize_person_property(person_id, RiskCategoryType, RiskCategory::Low);
        assert_eq!(context.get_person_property(person_id, Age), 42);
        assert_eq!(
            context.get_person_property(person_id, RiskCategoryType),
            RiskCategory::Low
        );
    }

    #[test]
    fn add_person_initializers() {
        let mut context = Context::new();
        let person_id = context.add_person();

        assert_eq!(context.get_person_property(person_id, RunningShoes), 0);
        assert!(!context.get_person_property(person_id, IsRunner));
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

        let person = context.add_person();
        // This should not emit a change event
        context.initialize_person_property(person, Age, 42);
        context.get_person_property(person, IsRunner);
        context.get_person_property(person, RunningShoes);

        context.execute();

        assert!(!*flag.borrow());
    }

    #[test]
    fn property_initialization_is_lazy() {
        let mut context = Context::new();
        let person = context.add_person();
        let people_data = context.get_data_container_mut(PeoplePlugin);

        // Verify we haven't initialized the property yet
        let has_value = *people_data.get_person_property_ref(person, RunningShoes);
        assert!(has_value.is_none());

        context.initialize_person_property(person, IsRunner, true);

        // This should initialize it
        let value = context.get_person_property(person, RunningShoes);
        assert_eq!(value, 4);
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
        let person_id = context.add_person();
        context.initialize_person_property(person_id, RiskCategoryType, RiskCategory::Low);
        context.set_person_property(person_id, RiskCategoryType, RiskCategory::High);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn observe_person_property_change_with_initializer() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, _event: PersonPropertyChangeEvent<RunningShoes>| {
                *flag_clone.borrow_mut() = true;
            },
        );
        let person_id = context.add_person();
        // Initializer wasn't called, so don't fire an event
        context.initialize_person_property(person_id, RunningShoes, 42);
        context.execute();
        assert!(!*flag.borrow());
    }

    #[test]
    fn observe_person_property_change_with_set() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, _event: PersonPropertyChangeEvent<RunningShoes>| {
                *flag_clone.borrow_mut() = true;
            },
        );
        let person_id = context.add_person();
        // Initializer called as a side effect of set, so event fires.
        context.set_person_property(person_id, RunningShoes, 42);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    #[should_panic(expected = "Property already initialized")]
    fn calling_initialize_twice_panics() {
        let mut context = Context::new();
        let person_id = context.add_person();
        context.initialize_person_property(person_id, IsRunner, true);
        context.initialize_person_property(person_id, IsRunner, true);
    }

    #[test]
    #[should_panic(expected = "Property already initialized")]
    fn calling_initialize_after_get_panics() {
        let mut context = Context::new();
        let person_id = context.add_person();
        let _ = context.get_person_property(person_id, IsRunner);
        context.initialize_person_property(person_id, IsRunner, true);
    }

    #[test]
    fn initialize_without_initializer_succeeds() {
        let mut context = Context::new();
        let person_id = context.add_person();
        context.initialize_person_property(person_id, RiskCategoryType, RiskCategory::High);
    }

    #[test]
    #[should_panic(expected = "Property not initialized")]
    fn set_without_initializer_panics() {
        let mut context = Context::new();
        let person_id = context.add_person();
        context.set_person_property(person_id, RiskCategoryType, RiskCategory::High);
    }
}
