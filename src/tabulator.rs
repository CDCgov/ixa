/*!

A `Tabulator` is a mechanism by which reports know their columns.

*/

use crate::{Context, ContextPeopleExt, PersonProperty};
use seq_macro::seq;
use std::any::TypeId;

pub trait Tabulator {
    fn setup(&self, context: &mut Context);
    fn get_typelist(&self) -> Vec<TypeId>;
    fn get_columns(&self) -> Vec<String>;
}

impl<T: PersonProperty + 'static> Tabulator for (T,) {
    fn setup(&self, context: &mut Context) {
        context.index_property(T::get_instance());
    }
    fn get_typelist(&self) -> Vec<TypeId> {
        vec![std::any::TypeId::of::<T>()]
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
                    T~N : PersonProperty + 'static,
                )*
            > Tabulator for (
                #(
                    T~N,
                )*
            )
            {
                fn setup(&self, context: &mut Context) {
                    #(
                        context.index_property(T~N::get_instance());
                    )*
                }
                fn get_typelist(&self) -> Vec<TypeId> {
                    vec![
                    #(
                        std::any::TypeId::of::<T~N>(),
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

#[cfg(test)]
mod tests {
    use super::Tabulator;
    use crate::{
        define_derived_property, define_person_property, define_person_property_with_default,
        Context, ContextPeopleExt,
    };
    use std::any::TypeId;
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
        assert_eq!(
            cols.get_typelist(),
            vec![TypeId::of::<Age>(), TypeId::of::<RiskCategory>()]
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
        tabulator.setup(&mut context);

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
