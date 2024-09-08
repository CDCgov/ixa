use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct PersonId {
    id: usize,
}

struct PeopleData {
    population: usize,
}

define_data_plugin!(PeoplePlugin, PeopleData, PeopleData { population: 0 });

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
        self.person_id
    }
}

pub trait PersonProperty {
    type Value: Copy;
    fn get_default() -> Self::Value;
}

#[macro_export]
macro_rules! define_person_property {
    ($person_property:ident, $value:ty, $default: expr) => {
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

use crate::{context::Context, define_data_plugin};

// Wish this was still possible:
// #[macro_export]
// macro_rules! define_person_property_from_enum {
//     ($person_property:ty, $default: expr) => {
//         impl $crate::people::PersonProperty for $person_property {
//             type Value = $person_property;

//             fn get_default() -> Self::Value {
//                 $default
//             }
//         }

//         impl Copy for $person_property {}

//         impl Clone for $person_property {
//             fn clone(&self) -> Self {
//                 *self
//             }
//         }
//     };
// }
// pub use define_person_property_from_enum;

struct PersonPropertiesDataContainer {
    values_map: HashMap<TypeId, Box<dyn Any>>,
}

impl PersonPropertiesDataContainer {
    #[allow(clippy::needless_pass_by_value)]
    fn get_person_property<T: PersonProperty + 'static>(
        &self,
        _property: T,
        person_id: PersonId,
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
            Some(data_container) => data_container.get_person_property(property, person_id),
        }
    }

    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        let data_container = self.get_data_container_mut(PersonPropertiesPlugin);

        // TODO: emit events upon property changes

        data_container.set_person_property(person_id, property, value);
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

    use crate::{
        context::Context,
        people::{PersonPropertiesContextExt, PersonProperty},
    };

    use super::{ContextPeopleExt, PersonPropertiesBuilderExt};

    define_person_property!(Age, u8, 0);

    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    pub enum Sex {
        Male,
        Female,
    }
    // Wish we could use this kind of macro
    // define_person_property_from_enum!(Sex, Sex::Female);
    define_person_property!(PersonSex, Sex, Sex::Female);

    #[test]
    fn add_person_default_properties() {
        let mut context = Context::new();
        let person_id = context.add_person().execute();
        assert_eq!(
            context.get_person_property(person_id, Age),
            Age::get_default()
        );
        assert_eq!(
            // Wish this worked:
            // context.get_person_property(Gender, person_id),
            context.get_person_property(person_id, PersonSex),
            PersonSex::get_default()
        );
    }

    #[test]
    fn add_person_set_properties() {
        let mut context = Context::new();
        let person_id = context
            .add_person()
            .set_person_property(Age, 10)
            .set_person_property(PersonSex, Sex::Male)
            .execute();
        assert_eq!(context.get_person_property(person_id, Age), 10);
        assert_eq!(context.get_person_property(person_id, PersonSex), Sex::Male);

        context.set_person_property(person_id, Age, 11);
        assert_eq!(context.get_person_property(person_id, Age), 11);

        context.set_person_property(person_id, PersonSex, Sex::Female);
        assert_eq!(
            context.get_person_property(person_id, PersonSex),
            Sex::Female
        );
    }
}
