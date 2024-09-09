use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use crate::{context::Context, define_data_plugin};

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct PersonId {
    id: usize,
}

struct PeopleData {
    population: usize,
}

define_data_plugin!(PeoplePlugin, PeopleData, PeopleData { population: 0 });

#[derive(Clone, Copy)]
#[allow(clippy::manual_non_exhaustive)]
pub struct PersonAdditionEvent {
    pub person_id: PersonId,
    // Prevent instantiation outside of this module
    _private: (),
}

pub trait ContextPeopleExt {
    fn add_person(&mut self, data: PersonData) -> PersonId;
}

impl ContextPeopleExt for Context {
    fn add_person(&mut self, data: PersonData) -> PersonId {
        let people_data_container = self.get_data_container_mut(PeoplePlugin);
        let person_id = PersonId {
            id: people_data_container.population,
        };
        people_data_container.population += 1;
        for callback in data.callbacks {
            callback(self, person_id);
        }
        self.emit_event(PersonAdditionEvent {
            person_id,
            _private: (),
        });
        person_id
    }
}

pub type PersonAdditionCallback = dyn FnOnce(&mut Context, PersonId);
pub struct PersonData {
    callbacks: Vec<Box<PersonAdditionCallback>>,
}

impl PersonData {
    #[must_use]
    pub fn new() -> PersonData {
        PersonData {
            callbacks: Vec::default(),
        }
    }

    pub fn add_callback(&mut self, callback: impl FnOnce(&mut Context, PersonId) + 'static) {
        self.callbacks.push(Box::new(callback));
    }
}

impl Default for PersonData {
    fn default() -> Self {
        Self::new()
    }
}

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

#[derive(Copy, Clone)]
#[allow(clippy::manual_non_exhaustive)]
pub struct PersonPropertyChangeEvent<T: PersonProperty> {
    pub person_id: PersonId,
    pub new_value: T::Value,
    pub old_value: T::Value,
    // Prevent instantiation outside of this module
    _private: (),
}

struct PersonPropertiesDataContainer {
    values_map: HashMap<TypeId, Box<dyn Any>>,
}

impl PersonPropertiesDataContainer {
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

pub trait PersonPropertiesContextExt {
    fn get_person_property<T: PersonProperty + 'static>(
        &self,
        person_id: PersonId,
        _property: T,
    ) -> T::Value;

    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        _property: T,
        value: T::Value,
    );
}

impl PersonPropertiesContextExt for Context {
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

        // Build event signaling person property has changed
        let current_value = data_container.get_person_property(person_id, property);
        let change_event: PersonPropertyChangeEvent<T> = PersonPropertyChangeEvent {
            person_id,
            new_value: value,
            old_value: current_value,
            _private: (),
        };

        data_container.set_person_property(person_id, property, value);

        self.emit_event(change_event);
    }
}

pub trait PersonPropertiesDataExt {
    fn set_person_property<T: PersonProperty + 'static>(&mut self, _property: T, value: T::Value);
}

impl PersonPropertiesDataExt for PersonData {
    fn set_person_property<T: PersonProperty + 'static>(&mut self, property: T, value: T::Value) {
        self.add_callback(move |context, person_id| {
            let data_container = context.get_data_container_mut(PersonPropertiesPlugin);
            data_container.set_person_property(person_id, property, value);
        });
    }
}

#[cfg(test)]
mod test {

    use std::{cell::RefCell, rc::Rc};

    use crate::{
        context::Context,
        people::{PersonData, PersonPropertiesContextExt, PersonPropertiesDataExt, PersonProperty},
    };

    use super::{ContextPeopleExt, PersonAdditionEvent, PersonPropertyChangeEvent};

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
        context.subscribe_to_event(move |_context, event: PersonAdditionEvent| {
            *flag_clone.borrow_mut() = true;
            assert_eq!(event.person_id.id, 0);
        });

        let _ = context.add_person(PersonData::new());
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn add_person_default_properties() {
        let mut context = Context::new();
        let person_id = context.add_person(PersonData::new());
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
        let mut person_data = PersonData::new();
        person_data.set_person_property(Age, 10);
        person_data.set_person_property(Sex, SexType::Male);
        let person_id = context.add_person(person_data);
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
            assert_eq!(event.old_value, 0);
            assert_eq!(event.new_value, 1);
        });

        let person_id = context.add_person(PersonData::new());
        context.set_person_property(person_id, Age, 1);
        context.execute();
        assert!(*flag.borrow());
    }
}
