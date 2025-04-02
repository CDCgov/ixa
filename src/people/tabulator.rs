/*!

A `Tabulator` is a particular kind of query, in particular, it is a list of properties. Executing
a `Tabulator` finds for every existing combinations of values of the properties in the property list
the set of people having that combination.

- Instead of the requirement that the property be a specific value, we only require the property to
  exist (to have any value) for a `PersonId` to match.
- A `Tabulator` automatically enables indexing for its properties and refreshes the indexes.
- The (text) names of the properties are returned with the results.
*/

use crate::people::external_api::ContextPeopleExtCrate;
use crate::people::index::IndexValue;
use crate::people::{PeoplePlugin, PersonProperty};
use crate::{Context, ContextPeopleExt, HashMap, HashSet, PersonId, TypeId};
use seq_macro::seq;

pub trait Tabulator {
    #[allow(clippy::missing_errors_doc)]
    fn setup(&self, context: &mut Context);
    fn get_columns(&self) -> Vec<String>;
    fn tabulate_person_properties<F>(&self, context: &mut Context, print_fn: F)
    where
        F: Fn(&Context, &[String], usize);
}

impl<T1: PersonProperty> Tabulator for (T1,) {
    fn setup(&self, context: &mut Context) {
        // Mirror's `Query::setup()`. Note: `Context::index_property()` automatically registers the
        // property.
        context.index_property::<T1>(T1::get_instance());

        // 1. Refresh the indexes for each property in the query.
        let data_container = context.get_data_container(PeoplePlugin).unwrap();
        data_container.index_unindexed_people::<T1>(context);
    }

    fn get_columns(&self) -> Vec<String> {
        vec![String::from(T1::name())]
    }

    fn tabulate_person_properties<F>(&self, context: &mut Context, print_fn: F)
    where
        F: Fn(&Context, &[String], usize),
    {
        self.setup(context);

        let data_container = context.get_data_container(PeoplePlugin).unwrap();
        let indexes = data_container.property_indexes.borrow();
        let mut index_list: Vec<&HashMap<IndexValue, (String, HashSet<PersonId>)>> = vec![];
        // Both unwraps guaranteed to succeed because we called `Tabulator::setup`.
        index_list.push(
            indexes
                .get_container_ref::<T1>()
                .unwrap()
                .lookup
                .as_ref()
                .unwrap(),
        );

        let mut property_value_names = vec![];
        let current_matches: HashSet<PersonId> = HashSet::default(); //dummy
        process_indices(
            context,
            index_list.as_slice(),
            &mut property_value_names,
            &current_matches,
            &print_fn,
        );
    }
}

macro_rules! impl_tabulator {
    ($ct:expr) => {
        seq!(N in 0..$ct {
            impl<
                #(
                    T~N : PersonProperty,
                )*
            > Tabulator for (
                #(
                    T~N,
                )*
            )
            {
                fn setup(&self, context: &mut $crate::Context) {
                    #(
                        <T~N>::register(context);
                        // ToDo: Decide what to do if this returns an error.
                        context.index_property_by_id(std::any::TypeId::of::<T~N>())
                               .unwrap_or_else(|e| panic!("type not found: {:?}", e));
                    )*
                    // 1. Refresh the indexes for each property in the query.
                    let data_container = context.get_data_container(PeoplePlugin).unwrap();
                    #(
                        data_container.index_unindexed_people::<T~N>(context);
                    )*
                }

                fn get_columns(&self) -> Vec<String> {
                    vec![
                    #(
                        String::from(T~N::name()),
                    )*
                    ]
                }

                fn tabulate_person_properties<F>(&self, context: &mut $crate::Context, print_fn: F)
                    where
                        F: Fn(&$crate::Context, &[String], usize)
                {
                    self.setup(context);

                    let data_container = context.get_data_container($crate::people::PeoplePlugin).unwrap();
                    let indexes = data_container.property_indexes.borrow();
                    let mut index_list: Vec<&HashMap<$crate::people::index::IndexValue, (String, HashSet<PersonId>)>> = vec![];
                    #(
                        // Both unwraps guaranteed to succeed because we called `Tabulator::setup`.
                        index_list.push(
                            indexes
                                .get_container_ref::<T~N>()
                                .unwrap()
                                .lookup
                                .as_ref()
                                .unwrap(),
                        );
                    )*

                    let mut property_value_names = vec![];
                    let current_matches: HashSet<$crate::PersonId> = HashSet::default(); //dummy
                    $crate::people::tabulator::process_indices(
                        context,
                        index_list.as_slice(),
                        &mut property_value_names,
                        &current_matches,
                        &print_fn,
                    );
                }

            }
        });
    }
}

seq!(Z in 2..20 {
    impl_tabulator!(Z);
});

// Implement Tabulator for the special case where we have type ids and not types, which
// occurs in the external API. Note that we can't register properties here, so this may fail.
// ToDo: Determine what to do in the event of failure.
impl Tabulator for Vec<(TypeId, String)> {
    fn setup(&self, context: &mut Context) {
        // Only called from contexts in which `PeopleData` is initialized?
        let data_container = context.get_data_container(PeoplePlugin).unwrap();

        // 1. Refresh the indexes for each property in the query.
        for (type_id, _) in self {
            data_container
                .index_property_by_id(*type_id)
                .unwrap_or_else(|e| panic!("{}", e));
            data_container
                .index_unindexed_people_by_id(context, *type_id)
                .unwrap_or_else(|e| panic!("{}", e));
        }
    }

    fn get_columns(&self) -> Vec<String> {
        self.iter().map(|a| a.1.clone()).collect()
    }

    fn tabulate_person_properties<F>(&self, context: &mut Context, print_fn: F)
    where
        F: Fn(&Context, &[String], usize),
    {
        self.setup(context);

        let data_container = context.get_data_container(PeoplePlugin).unwrap();
        let indexes = data_container.property_indexes.borrow();

        let mut index_list: Vec<&HashMap<IndexValue, (String, HashSet<PersonId>)>> = vec![];

        for (type_id, _) in self {
            // Both unwraps guaranteed to succeed because we called `Tabulator::setup`.
            index_list.push(
                indexes
                    .get_container_by_id_ref(*type_id)
                    .unwrap()
                    .lookup
                    .as_ref()
                    .unwrap(),
            );
        }

        let mut property_value_names = vec![];
        let current_matches: HashSet<PersonId> = HashSet::default(); //dummy
        process_indices(
            context,
            index_list.as_slice(),
            &mut property_value_names,
            &current_matches,
            &print_fn,
        );
    }
}

pub fn process_indices(
    context: &Context,
    remaining_indices: &[&HashMap<IndexValue, (String, HashSet<PersonId>)>],
    property_value_names: &mut Vec<String>,
    current_matches: &HashSet<PersonId>,
    print_fn: &dyn Fn(&Context, &[String], usize),
) {
    if remaining_indices.is_empty() {
        print_fn(
            context,
            property_value_names.as_slice(),
            current_matches.len(),
        );
        return;
    }

    let (&next_index, rest_indices) = remaining_indices.split_first().unwrap();

    // If there is nothing in the index, we don't need to process it
    if next_index.is_empty() {
        return;
    }

    for (value_name, people) in next_index.values() {
        let intersect = !property_value_names.is_empty();

        property_value_names.push(value_name.clone());

        let matches = if intersect {
            &current_matches.intersection(people).cloned().collect()
        } else {
            people
        };

        process_indices(
            context,
            rest_indices,
            property_value_names,
            matches,
            print_fn,
        );
        property_value_names.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::Tabulator;
    use crate::{
        define_derived_property, define_person_property, define_person_property_with_default,
        Context, ContextPeopleExt,
    };
    use std::cell::RefCell;
    use std::collections::HashSet;

    define_person_property!(Age, u8);
    type RiskCategoryValue = u8;
    define_person_property!(RiskCategory, RiskCategoryValue);
    define_person_property_with_default!(IsRunner, bool, false);
    define_person_property_with_default!(IsSwimmer, bool, false);
    define_derived_property!(AdultSwimmer, bool, [IsSwimmer, Age], |is_swimmer, age| {
        is_swimmer && age >= 18
    });

    #[test]
    fn test_tabulator() {
        let cols = (Age, RiskCategory);
        assert_eq!(cols.get_columns(), vec!["Age", "RiskCategory"]);
    }

    fn tabulate_properties_test_setup<T: Tabulator>(
        tabulator: &T,
        setup: impl FnOnce(&mut Context),
        expected_values: &HashSet<(Vec<String>, usize)>,
    ) {
        let mut context = Context::new();
        setup(&mut context);

        let results: RefCell<HashSet<(Vec<String>, usize)>> = RefCell::new(HashSet::new());
        context.tabulate_person_properties(tabulator, |_context, values, count| {
            results.borrow_mut().insert((values.to_vec(), count));
        });

        let results = &*results.borrow();
        assert_eq!(results, expected_values);
    }

    #[test]
    fn test_periodic_report() {
        let tabulator = (IsRunner,);
        let mut expected = HashSet::new();
        expected.insert((vec!["true".to_string()], 1));
        expected.insert((vec!["false".to_string()], 1));
        tabulate_properties_test_setup(
            &tabulator,
            |context| {
                let bob = context.add_person(()).unwrap();
                context.add_person(()).unwrap();
                context.set_person_property(bob, IsRunner, true);
            },
            &expected,
        );
    }

    #[test]
    fn test_get_counts_multi() {
        let tabulator = (IsRunner, IsSwimmer);
        let mut expected = HashSet::new();
        expected.insert((vec!["false".to_string(), "false".to_string()], 3));
        expected.insert((vec!["false".to_string(), "true".to_string()], 1));
        expected.insert((vec!["true".to_string(), "false".to_string()], 1));
        expected.insert((vec!["true".to_string(), "true".to_string()], 1));

        tabulate_properties_test_setup(
            &tabulator,
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
