use crate::{context::Context, define_data_plugin};
use std::{
    any::{Any, TypeId},
    cell::{RefCell, RefMut},
    collections::HashMap,
    fmt,
};

type DerivedSetter = dyn Fn(&mut Context, PersonId);

// PeopleData represents each unique person in the simulation with an id ranging
// from 0 to population - 1. Person properties are associated with a person
// via their id.
struct PeopleData {
    current_population: usize,
    properties_map: RefCell<HashMap<TypeId, Box<dyn Any>>>,
    property_dependencies: HashMap<TypeId, Vec<Box<DerivedSetter>>>,
}

define_data_plugin!(
    PeoplePlugin,
    PeopleData,
    PeopleData {
        current_population: 0,
        properties_map: RefCell::new(HashMap::new()),
        property_dependencies: HashMap::new()
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
    #[must_use]
    fn is_derived() -> bool {
        false
    }
    fn initialize(context: &Context, person_id: PersonId) -> Self::Value;
    #[must_use]
    fn calculate(_context: &Context, _person_id: PersonId) -> Self::Value {
        panic!("Property not derived");
    }
    #[must_use]
    fn dependencies() -> Vec<TypeId> {
        panic!("Property not derived");
    }
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

/// Defines a derived person property with the following parameters:
/// * `$person_property`: A name for the identifier type of the property
/// * `$value`: The type of the property's value
/// * `[$($dependency),+]`: A list of person properties the derived property depends on
/// * $calculate: A closure that takes the values of each dependency and returns the derived value
#[macro_export]
macro_rules! define_derived_person_property {
    ($derived_property:ident, $value:ty, [$($dependency:ident),+], |$($param:ident),+| $derive_fn:expr) => {
        #[derive(Copy, Clone)]
        pub struct $derived_property;

        impl $crate::people::PersonProperty for $derived_property {
            type Value = $value;

            fn calculate(context: &$crate::context::Context, person_id: $crate::people::PersonId) -> Self::Value {
                #[allow(unused_parens)]
                let ($($param),+) = (
                    $(context.get_person_property(person_id, $dependency)),+
                );
                (|$($param),+| $derive_fn)($($param),+)
            }
            fn is_derived() -> bool { true }
            fn dependencies() -> Vec<std::any::TypeId> {
                vec![$(std::any::TypeId::of::<$dependency>()),+]
             }
            fn initialize(
                context: &$crate::context::Context,
                person: $crate::people::PersonId,
            ) -> Self::Value {
                Self::calculate(context, person)
            }
        }
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

    fn add_dependency_callback(
        &mut self,
        dependency: TypeId,
        callback: impl Fn(&mut Context, PersonId) + 'static,
    ) {
        self.property_dependencies
            .entry(dependency)
            .or_default()
            .push(Box::new(callback));
    }
}

// Emitted when a new person is created
// These are internal to this module; use subscribe_to_person_created
#[derive(Clone, Copy)]
#[allow(clippy::manual_non_exhaustive)]
struct PersonCreatedEvent {
    pub person_id: PersonId,
}

// Emitted when a person property is updated
// These are internal to this module; use subscribe_to_person_property_changed
#[derive(Copy, Clone)]
#[allow(clippy::manual_non_exhaustive)]
struct PersonPropertyChangeEvent<T: PersonProperty> {
    pub person_id: PersonId,
    pub current: T::Value,
    pub previous: T::Value,
}

trait PrivateContextPeopleExt {
    fn set_person_property_internal<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    );
}

impl PrivateContextPeopleExt for Context {
    fn set_person_property_internal<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        let data_container = self.get_data_container(PeoplePlugin).expect(
            "PeoplePlugin is not initialized; make sure you add a person before setting properties",
        );
        let current_value = *data_container.get_person_property_ref(person_id, property);

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

                let data_container = self.get_data_container_mut(PeoplePlugin);
                if let Some(callbacks) = data_container
                    .property_dependencies
                    .get_mut(&TypeId::of::<T>())
                {
                    // Temporarily move the callbacks out for ownership reasons
                    let mut collector: Vec<Box<DerivedSetter>> = std::mem::take(callbacks);

                    for callback in &mut collector {
                        callback(self, person_id);
                    }

                    // Insert the callbacks back into the map
                    self.get_data_container_mut(PeoplePlugin)
                        .property_dependencies
                        .insert(TypeId::of::<T>(), collector);
                }
            }
            // The person property is not yet initialized, so we don't emit any events.
            None => {
                data_container.set_person_property(person_id, property, value);
            }
        }
    }
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

    /// Given a `PersonId`, sets the value of a defined person property
    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        _property: T,
        value: T::Value,
    );

    fn register_derived_property<T: PersonProperty + 'static>(&mut self, property: T);

    fn subscribe_to_person_created(&mut self, handler: impl Fn(&mut Context, PersonId) + 'static);

    fn subscribe_to_person_property_changed<T: PersonProperty + 'static>(
        &mut self,
        _property: T,
        handler: impl Fn(&mut Context, PersonId, T::Value, T::Value) + 'static,
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

    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        assert!(!T::is_derived(), "Cannot set a derived property directly");
        self.set_person_property_internal(person_id, property, value);
    }

    fn register_derived_property<T: PersonProperty + 'static>(&mut self, property: T) {
        let data_container = self.get_data_container_mut(PeoplePlugin);
        let dependencies = T::dependencies();
        // For each dependency, create a callback that recalculates the derived property
        // and sets it. This should be called whenever the dependency is set.
        for dependency in dependencies {
            data_container.add_dependency_callback(
                dependency,
                move |context: &mut Context, person_id: PersonId| {
                    let new_value = T::calculate(context, person_id);
                    context.set_person_property_internal(person_id, property, new_value);
                },
            );
        }
    }

    fn subscribe_to_person_created(&mut self, handler: impl Fn(&mut Context, PersonId) + 'static) {
        self.subscribe_to_event(move |context, event: PersonCreatedEvent| {
            handler(context, event.person_id);
        });
    }

    fn subscribe_to_person_property_changed<T: PersonProperty + 'static>(
        &mut self,
        property: T,
        handler: impl Fn(&mut Context, PersonId, T::Value, T::Value) + 'static,
    ) {
        if T::is_derived() {
            self.register_derived_property(property);
        }
        self.subscribe_to_event(move |context, event: PersonPropertyChangeEvent<T>| {
            handler(context, event.person_id, event.current, event.previous);
        });
    }
}

#[cfg(test)]
mod test {
    use super::{ContextPeopleExt, PersonId};
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
        context.subscribe_to_person_created(move |_context, person| {
            *flag_clone.borrow_mut() = true;
            assert_eq!(person.id, 0);
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
        context.set_person_property(person, Age, 8);

        // Create a bunch of people and don't initialize Age
        for _ in 1..9 {
            let _person = context.add_person();
        }
        let tenth_person = context.add_person();

        // Set a person property for a person > index 0
        context.set_person_property(tenth_person, Age, 42);
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
        context.set_person_property(person_id, Age, 42);
        context.set_person_property(person_id, RiskCategoryType, RiskCategory::Low);
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
        context.subscribe_to_person_property_changed(
            RiskCategoryType,
            move |_context, _person, _current, _prev| {
                *flag_clone.borrow_mut() = true;
            },
        );

        let person = context.add_person();
        // This should not emit a change event
        context.set_person_property(person, Age, 42);
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

        context.set_person_property(person, IsRunner, true);

        // This should initialize it
        let value = context.get_person_property(person, RunningShoes);
        assert_eq!(value, 4);
    }

    #[test]
    fn observe_person_property_change() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_person_property_changed(
            RiskCategoryType,
            move |_context, person, current, previous| {
                *flag_clone.borrow_mut() = true;
                assert_eq!(person.id, 0, "Person id is correct");
                assert_eq!(previous, RiskCategory::Low, "Previous value is correct");
                assert_eq!(current, RiskCategory::High, "Current value is correct");
            },
        );
        let person_id = context.add_person();
        context.set_person_property(person_id, RiskCategoryType, RiskCategory::Low);
        context.set_person_property(person_id, RiskCategoryType, RiskCategory::High);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn observe_person_property_change_with_initializer() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_person_property_changed(
            RunningShoes,
            move |_context, _person, _current, _prev| {
                *flag_clone.borrow_mut() = true;
            },
        );
        let person_id = context.add_person();
        // Initializer wasn't called, so don't fire an event
        context.set_person_property(person_id, RunningShoes, 42);
        context.execute();
        assert!(!*flag.borrow());
    }

    #[test]
    fn derived_property_returns_correct_values() {
        let mut context = Context::new();
        define_derived_person_property!(MastersRunner, bool, [Age, IsRunner], |age, is_runner| age
            >= 40
            && is_runner);

        let paula = context.add_person();
        context.set_person_property(paula, Age, 50);
        context.set_person_property(paula, IsRunner, true);

        let colleen = context.add_person();
        context.set_person_property(colleen, Age, 31);
        context.set_person_property(colleen, IsRunner, true);

        assert!(context.get_person_property(paula, MastersRunner),);
        assert!(!context.get_person_property(colleen, MastersRunner),);
    }


    #[test]
    fn derived_property_changes_correctly() {
        let mut context = Context::new();
        define_derived_person_property!(MastersRunner, bool, [Age, IsRunner], |age, is_runner| age
            >= 40
            && is_runner);

        let paula = context.add_person();
        context.set_person_property(paula, Age, 50);
        context.set_person_property(paula, IsRunner, true);

        let colleen = context.add_person();
        context.set_person_property(colleen, Age, 31);
        context.set_person_property(colleen, IsRunner, true);

        assert!(context.get_person_property(paula, MastersRunner),);
        assert!(!context.get_person_property(colleen, MastersRunner),);

        context.set_person_property(colleen, Age, 50);
        assert!(context.get_person_property(colleen, MastersRunner),);        
    }
        
    #[test]
    #[should_panic(expected = "Cannot set a derived property directly")]
    fn setting_derived_property_explicitly_panics() {
        let mut context = Context::new();
        define_derived_person_property!(Senior, bool, [Age], |age| age >= 65);

        let person = context.add_person();
        context.set_person_property(person, Senior, true);
    }

    #[test]
    fn derived_property_initializes() {
        let mut context = Context::new();
        define_person_property!(Friendly, bool, |_context, _person_id| true);
        define_person_property!(IsHuman, bool, |_context, _person_id| true);
        define_derived_person_property!(
            Likeable,
            bool,
            [Friendly, IsHuman],
            |friendly, is_human| { friendly && is_human }
        );

        let person = context.add_person();

        assert!(context.get_person_property(person, Likeable));
    }

    #[test]
    fn derived_property_change_event() {
        let mut context = Context::new();
        define_derived_person_property!(MastersRunner, bool, [Age, IsRunner], |age, is_runner| age
            >= 40
            && is_runner);

        let person = context.add_person();
        context.set_person_property(person, Age, 50);
        context.set_person_property(person, IsRunner, false);

        // Initialize it so we actually get change events
        assert!(!context.get_person_property(person, MastersRunner));

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_person_property_changed(
            MastersRunner,
            move |_context, person_id, current, prev| {
                assert_eq!(person_id, person);
                assert!(!prev, "Correct previous value");
                assert!(current, "Correct current value");
                *flag_clone.borrow_mut() = true;
            },
        );
        context.set_person_property(person, IsRunner, true);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn derived_property_change_event_multiple_changes() {
        let mut context = Context::new();
        define_derived_person_property!(MastersRunner, bool, [Age, IsRunner], |age, is_runner| age
            >= 40
            && is_runner);

        let person = context.add_person();
        context.set_person_property(person, Age, 50);
        context.set_person_property(person, IsRunner, false);

        // Initialize it so we actually get change events
        assert!(!context.get_person_property(person, MastersRunner));

        let flag = Rc::new(RefCell::new(0));
        let flag_clone = flag.clone();
        context.subscribe_to_person_property_changed(
            MastersRunner,
            move |_context, _person_id, _current, _prev| {
                *flag_clone.borrow_mut() += 1;
            },
        );
        context.set_person_property(person, IsRunner, true);
        context.set_person_property(person, Age, 30);
        context.execute();
        assert_eq!(*flag.borrow(), 2);
    }
}
