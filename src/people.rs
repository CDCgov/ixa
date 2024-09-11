use crate::{context::Context, define_data_plugin};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

// PeopleData represents each unique person in the simulation with an id ranging
// from 0 to population - 1. Other data are associated with a person via their
// their id.
struct PeopleData {
    population: usize,
}

define_data_plugin!(PeoplePlugin, PeopleData, PeopleData { population: 0 });

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
    fn get_default() -> Self::Value;
}

#[macro_export]
macro_rules! define_person_property {
    ($person_property:ident, $value:ty, $default: expr) => {
        #[derive(Copy, Clone)]
        pub struct $person_property;

        impl $crate::people::PersonProperty for $person_property {
            type Value = $value;

            fn get_default() -> Self::Value {
                $default
            }
        }
    };
}
pub use define_person_property;

/// Person property values are stored in a `HashMap` by `TypeId` of the property,
/// with each value stored in a Vec indexed by the person's id.
struct PersonPropertiesDataContainer {
    values_map: HashMap<TypeId, Box<dyn Any>>,
}

impl PersonPropertiesDataContainer {
    /// Given a `PersonId`, returns the value of a person property of type `T`
    #[allow(clippy::needless_pass_by_value)]
    fn get_person_property<T: PersonProperty + 'static>(
        &self,
        person_id: PersonId,
        _property: T,
    ) -> T::Value {
        match self.values_map.get(&TypeId::of::<T>()) {
            Some(boxed_vec) => {
                let index = person_id.id;
                let vec = boxed_vec.downcast_ref::<Vec<T::Value>>().unwrap();
                if index >= vec.len() {
                    T::get_default()
                } else {
                    vec[index]
                }
            }
            None => T::get_default(),
        }
    }

    /// Given a `PersonId`, sets the value of a person property of type `T`
    #[allow(clippy::needless_pass_by_value)]
    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        _property: T,
        value: T::Value,
    ) {
        let index = person_id.id;
        let vec = self
            .values_map
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(Vec::<T::Value>::with_capacity(index)));
        let vec: &mut Vec<T::Value> = vec.downcast_mut().unwrap();
        if index >= vec.len() {
            vec.resize(index + 1, T::get_default());
        }
        vec[index] = value;
    }
}

define_data_plugin!(
    PersonPropertiesPlugin,
    PersonPropertiesDataContainer,
    PersonPropertiesDataContainer {
        values_map: HashMap::default()
    }
);

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

pub type PersonBuilderCallback = dyn FnOnce(&mut Context, PersonId);
pub struct PersonBuilder<'a> {
    /// Internal
    callbacks: Vec<Box<PersonBuilderCallback>>,
    /// Reference to the simulation context
    context: &'a mut Context,
    /// Id of the person being built
    person_id: PersonId,
}

impl<'a> PersonBuilder<'a> {
    pub fn new(context: &'a mut Context) -> PersonBuilder<'a> {
        let people_data_container = context.get_data_container_mut(PeoplePlugin);
        let person_id = PersonId {
            id: people_data_container.population,
        };
        PersonBuilder {
            context,
            person_id,
            callbacks: Vec::new(),
        }
    }

    /// Returns a reference to `Context` which might be needed to build the person
    pub fn get_context(&mut self) -> &mut Context {
        self.context
    }

    /// Returns the identifier struct for the person
    #[must_use]
    pub fn get_person_id(&self) -> PersonId {
        self.person_id
    }

    fn add_callback(&mut self, callback: impl FnOnce(&mut Context, PersonId) + 'static) {
        self.callbacks.push(Box::new(callback));
    }

    /// Sets the value a person property of type `T`
    ///
    /// Returns `self` to allow for chaining
    #[must_use]
    pub fn set_person_property<T: PersonProperty + 'static>(
        mut self,
        property: T,
        value: T::Value,
    ) -> PersonBuilder<'a> {
        self.add_callback(move |context, person_id| {
            context.set_person_property(person_id, property, value);
        });
        self
    }

    /// Inserts the finalized person into the data container after person properties
    /// have been set.
    #[must_use]
    pub fn insert(self) -> PersonId {
        let people_data_container = self.context.get_data_container_mut(PeoplePlugin);
        people_data_container.population += 1;
        let person_id = self.get_person_id();
        for callback in self.callbacks {
            callback(self.context, person_id);
        }
        self.context.emit_event(PersonCreatedEvent {
            person_id: self.person_id,
        });
        self.person_id
    }
}

pub trait ContextPeopleExt {
    /// Creates a person by instantiating a `PersonBuilder`, which can be used to
    /// set properties before inserting it into the data container.
    fn create_person(&mut self) -> PersonBuilder;

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
}

impl ContextPeopleExt for Context {
    fn create_person(&mut self) -> PersonBuilder {
        PersonBuilder::new(self)
    }

    fn get_person_property<T: PersonProperty + 'static>(
        &self,
        person_id: PersonId,
        property: T,
    ) -> T::Value {
        match self.get_data_container(PersonPropertiesPlugin) {
            None => T::get_default(),
            Some(data_container) => data_container.get_person_property(person_id, property),
        }
    }

    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        let data_container = self.get_data_container_mut(PersonPropertiesPlugin);

        let current_value = data_container.get_person_property(person_id, property);
        let change_event: PersonPropertyChangeEvent<T> = PersonPropertyChangeEvent {
            person_id,
            current: value,
            previous: current_value,
        };

        data_container.set_person_property(person_id, property, value);

        self.emit_event(change_event);
    }
}

#[cfg(test)]
mod test {

    use std::{cell::RefCell, rc::Rc};

    use crate::context::Context;

    use super::{ContextPeopleExt, PersonCreatedEvent, PersonProperty, PersonPropertyChangeEvent};

    define_person_property!(Age, u8, 0);

    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    pub enum SexType {
        Male,
        Female,
    }

    define_person_property!(Sex, SexType, SexType::Female);

    #[test]
    fn observe_person_addition() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(move |_context, event: PersonCreatedEvent| {
            *flag_clone.borrow_mut() = true;
            assert_eq!(event.person_id.id, 0);
        });

        let _ = context.create_person().insert();
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn add_person_default_properties() {
        let mut context = Context::new();
        let person_id = context.create_person().insert();
        assert_eq!(
            context.get_person_property(person_id, Age),
            Age::get_default()
        );
        assert_eq!(
            context.get_person_property(person_id, Sex),
            Sex::get_default()
        );
    }

    #[test]
    fn add_person_set_properties() {
        let mut context = Context::new();
        let person_id = context
            .create_person()
            .set_person_property(Age, 10)
            .set_person_property(Sex, SexType::Male)
            .insert();
        assert_eq!(context.get_person_property(person_id, Age), 10);
        assert_eq!(context.get_person_property(person_id, Sex), SexType::Male);

        context.set_person_property(person_id, Age, 11);
        assert_eq!(context.get_person_property(person_id, Age), 11);

        context.set_person_property(person_id, Sex, SexType::Female);
        assert_eq!(context.get_person_property(person_id, Sex), SexType::Female);
    }

    #[test]
    fn observe_person_property_change() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(move |_context, event: PersonPropertyChangeEvent<Age>| {
            *flag_clone.borrow_mut() = true;
            assert_eq!(event.person_id.id, 0);
            assert_eq!(event.previous, 0);
            assert_eq!(event.current, 1);
        });

        let person_id = context.create_person().insert();
        context.set_person_property(person_id, Age, 1);
        context.execute();
        assert!(*flag.borrow());
    }
}
