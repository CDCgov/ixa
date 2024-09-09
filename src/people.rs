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
    fn add_person(&mut self) -> PersonBuilder;
}

impl ContextPeopleExt for Context {
    fn add_person(&mut self) -> PersonBuilder {
        PersonBuilder::new(self)
    }
}

pub struct PersonBuilder<'a> {
    context: &'a mut Context,
    person_id: PersonId,
}

impl<'a> PersonBuilder<'a> {
    pub fn new(context: &'a mut Context) -> PersonBuilder<'a> {
        let people_data_container = context.get_data_container_mut(PeoplePlugin);
        let person_id = PersonId {
            id: people_data_container.population,
        };
        PersonBuilder { context, person_id }
    }

    pub fn get_context(&mut self) -> &mut Context {
        self.context
    }

    #[must_use]
    pub fn get_person_id(&self) -> PersonId {
        self.person_id
    }

    #[must_use]
    pub fn execute(self) -> PersonId {
        let people_data_container = self.context.get_data_container_mut(PeoplePlugin);
        people_data_container.population += 1;
        self.context.emit_event(PersonAdditionEvent {
            person_id: self.person_id,
            _private: (),
        });
        self.person_id
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

pub trait PersonPropertiesBuilderExt<'a> {
    fn set_person_property<T: PersonProperty + 'static>(
        self,
        _property: T,
        value: T::Value,
    ) -> PersonBuilder<'a>;
}

impl<'a> PersonPropertiesBuilderExt<'a> for PersonBuilder<'a> {
    fn set_person_property<T: PersonProperty + 'static>(
        mut self,
        property: T,
        value: T::Value,
    ) -> PersonBuilder<'a> {
        let person_id = self.get_person_id();
        let data_container = self
            .get_context()
            .get_data_container_mut(PersonPropertiesPlugin);
        data_container.set_person_property(person_id, property, value);
        self
    }
}

#[cfg(test)]
mod test {

    use std::{cell::RefCell, rc::Rc};

    use crate::{
        context::Context,
        people::{PersonPropertiesContextExt, PersonProperty},
    };

    use super::{
        ContextPeopleExt, PersonAdditionEvent, PersonPropertiesBuilderExt,
        PersonPropertyChangeEvent,
    };

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

        let _ = context.add_person().execute();
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn add_person_default_properties() {
        let mut context = Context::new();
        let person_id = context.add_person().execute();
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
            .add_person()
            .set_person_property(Age, 10)
            .set_person_property(Sex, SexType::Male)
            .execute();
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

        let person_id = context.add_person().execute();
        context.set_person_property(person_id, Age, 1);
        context.execute();
        assert!(*flag.borrow());
    }
}
