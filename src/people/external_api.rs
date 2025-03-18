use crate::people::ContextPeopleExt;
use crate::people::PeoplePlugin;
use crate::Context;
use crate::IxaError;
use crate::PersonId;
use std::any::TypeId;

pub(crate) trait ContextPeopleExtCrate {
    fn get_person_property_by_name(
        &self,
        name: &str,
        person_id: PersonId,
    ) -> Result<String, IxaError>;

    fn tabulate_person_properties_by_name<F>(
        &self,
        properties: Vec<String>,
        print_fn: F,
    ) -> Result<(), IxaError>
    where
        F: Fn(&Context, &[String], usize);

    fn index_property_by_id(&self, type_id: TypeId);

    fn get_person_property_names(&self) -> Vec<String>;
}

impl ContextPeopleExtCrate for Context {
    fn get_person_property_by_name(
        &self,
        name: &str,
        person_id: PersonId,
    ) -> Result<String, IxaError> {
        let data_container = self.get_data_container(PeoplePlugin);
        if data_container.is_none() {
            return Err(IxaError::IxaError(String::from("No people exist")));
        }
        let data_container = data_container.unwrap();
        let type_id = *data_container
            .people_types
            .borrow()
            .get(name)
            .ok_or(IxaError::IxaError(format!("No property '{name}'")))?;
        let methods = data_container.get_methods(type_id);
        Ok((methods.get_display)(self, person_id))
    }

    fn tabulate_person_properties_by_name<F>(
        &self,
        properties: Vec<String>,
        print_fn: F,
    ) -> Result<(), IxaError>
    where
        F: Fn(&Context, &[String], usize),
    {
        let data_container = self.get_data_container(PeoplePlugin);
        if data_container.is_none() {
            return Ok(());
        }
        let data_container = data_container.unwrap();
        let people_types = data_container.people_types.borrow();

        let mut query = Vec::new();
        for name in properties {
            let type_id = people_types
                .get(&name.to_string())
                .ok_or(IxaError::IxaError(format!("No property '{name}'")))?;

            query.push((*type_id, name.to_string()));
        }

        self.tabulate_person_properties(&query, print_fn);
        Ok(())
    }

    fn index_property_by_id(&self, type_id: TypeId) {
        let data_container = self.get_data_container(PeoplePlugin).unwrap();

        let mut index = data_container.get_index_ref_mut(type_id).unwrap();
        index.enable_indexing();
    }

    fn get_person_property_names(&self) -> Vec<String> {
        let data_container = self.get_data_container(PeoplePlugin);
        if data_container.is_none() {
            return Vec::new();
        }
        let data_container = data_container.unwrap();
        let people_types = data_container.people_types.borrow();

        people_types
            .keys()
            .map(std::string::ToString::to_string)
            .collect()
    }
}

#[cfg(test)]
mod test {
    use std::cell::RefCell;
    use std::collections::HashSet;

    use super::ContextPeopleExtCrate;
    use crate::people::{define_person_property, ContextPeopleExt};
    use crate::ContextRandomExt;
    use crate::{define_person_property_with_default, Context};

    define_person_property!(Age, u8);

    #[test]
    fn get_property_string() {
        let mut context = Context::new();
        context.init_random(42);

        let person1 = context.add_person((Age, 10)).unwrap();
        let age = context.get_person_property_by_name("Age", person1).unwrap();
        assert_eq!(age, "10");
    }

    #[test]
    fn get_unknown_property_string() {
        let mut context = Context::new();
        context.init_random(42);

        let person1 = context.add_person((Age, 10)).unwrap();
        let age = context.get_person_property_by_name("Unknown", person1);
        assert!(age.is_err());
    }

    define_person_property_with_default!(IsRunner, bool, false);
    define_person_property_with_default!(IsSwimmer, bool, false);

    fn tabulate_properties_test_setup(
        tabulator: Vec<String>,
        setup: impl FnOnce(&mut Context),
        expected_values: &HashSet<(Vec<String>, usize)>,
    ) {
        let mut context = Context::new();
        setup(&mut context);

        let results: RefCell<HashSet<(Vec<String>, usize)>> = RefCell::new(HashSet::new());
        context
            .tabulate_person_properties_by_name(tabulator, |_context, values, count| {
                results.borrow_mut().insert((values.to_vec(), count));
            })
            .unwrap();

        let results = &*results.borrow();
        assert_eq!(results, expected_values);
    }

    #[test]
    fn test_get_counts_multi_by_name() {
        let tabulator = vec![String::from("IsRunner"), String::from("IsSwimmer")];
        let mut expected = HashSet::new();
        expected.insert((vec!["false".to_string(), "false".to_string()], 3));
        expected.insert((vec!["false".to_string(), "true".to_string()], 1));
        expected.insert((vec!["true".to_string(), "false".to_string()], 1));
        expected.insert((vec!["true".to_string(), "true".to_string()], 1));

        tabulate_properties_test_setup(
            tabulator,
            |context| {
                context.add_person(()).unwrap();
                context.add_person(()).unwrap();
                context.add_person(()).unwrap();
                let bob = context.add_person(()).unwrap();
                let anne = context.add_person(()).unwrap();
                let charlie = context.add_person(()).unwrap();

                context.set_person_property(bob, IsRunner, true);
                context.set_person_property(charlie, IsRunner, true);
                context.set_person_property(anne, IsSwimmer, true);
                context.set_person_property(charlie, IsSwimmer, true);
            },
            &expected,
        );
    }
}
