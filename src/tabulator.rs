use std::any::TypeId;

use seq_macro::seq;

use crate::people::external_api::ContextPeopleExtCrate;
use crate::people::PersonProperty;
use crate::{Context, ContextPeopleExt, IxaError};
pub trait Tabulator {
    #[allow(clippy::missing_errors_doc)]
    fn setup(&self, context: &Context) -> Result<(), IxaError>;
    fn get_typelist(&self) -> Vec<TypeId>;
    fn get_columns(&self) -> Vec<String>;
}

impl<T: PersonProperty> Tabulator for (T,) {
    fn setup(&self, context: &Context) -> Result<(), IxaError> {
        context.register_property::<T>();
        context.index_property_by_id(T::type_id());
        Ok(())
    }
    fn get_typelist(&self) -> Vec<TypeId> {
        vec![T::type_id()]
    }
    fn get_columns(&self) -> Vec<String> {
        vec![String::from(T::name())]
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
                fn setup(&self, context: &Context) -> Result<(), IxaError> {
                    #(
                        context.register_property::<T~N>();
                        context.index_property_by_id(T~N::type_id());
                    )*
                    Ok(())
                }

                fn get_typelist(&self) -> Vec<TypeId> {
                    vec![
                    #(
                        T~N::type_id(),
                    )*
                    ]
                }

                fn get_columns(&self) -> Vec<String> {
                    vec![
                    #(
                        String::from(T~N::name()),
                    )*
                    ]
                }
            }
        });
    }
}

seq!(Z in 2..20 {
    impl_tabulator!(Z);
});

// Implement Tabulator for the special case where we have type ids and not
// types. Note that we can't register properties here, so this may fail.
impl Tabulator for Vec<(TypeId, String)> {
    #[allow(clippy::missing_errors_doc)]
    fn setup(&self, context: &Context) -> Result<(), IxaError> {
        for (t, _) in self {
            context.index_property_by_id(*t);
        }
        Ok(())
    }

    fn get_typelist(&self) -> Vec<TypeId> {
        self.iter().map(|a| a.0).collect()
    }

    fn get_columns(&self) -> Vec<String> {
        self.iter().map(|a| a.1.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::Tabulator;
    use crate::{
        define_person_property, define_person_property_with_default, Context, ContextPeopleExt,
        HashSet, HashSetExt, PersonProperty,
    };

    define_person_property!(Age, u8);
    type RiskCategoryValue = u8;
    define_person_property!(RiskCategory, RiskCategoryValue);
    define_person_property_with_default!(IsRunner, bool, false);
    define_person_property_with_default!(IsSwimmer, bool, false);

    #[test]
    fn test_tabulator() {
        let cols = (Age, RiskCategory);
        assert_eq!(
            cols.get_typelist(),
            vec![Age::type_id(), RiskCategory::type_id()]
        );
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
