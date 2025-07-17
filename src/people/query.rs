use crate::people::index::IndexValue;
use crate::{Context, ContextPeopleExt, PersonProperty};
use seq_macro::seq;
use std::any::TypeId;

/// Encapsulates a person query.
///
/// [`Context::query_people`] actually takes an instance of [`Query`], but because
/// we implement Query for tuples of up to size 20, that's invisible
/// to the caller. Do not use this trait directly.
pub trait Query: Copy {
    fn setup(&self, context: &Context);
    fn get_query(&self) -> Vec<(TypeId, IndexValue)>;
}

impl Query for () {
    fn setup(&self, _: &Context) {}

    fn get_query(&self) -> Vec<(TypeId, IndexValue)> {
        vec![]
    }
}

// Implement the query version with one parameter.
impl<T1: PersonProperty> Query for (T1, T1::Value) {
    fn setup(&self, context: &Context) {
        context.register_property::<T1>();
    }

    fn get_query(&self) -> Vec<(TypeId, IndexValue)> {
        vec![(std::any::TypeId::of::<T1>(), IndexValue::compute(&self.1))]
    }
}

// Implement the versions with 1..20 parameters.
macro_rules! impl_query {
    ($ct:expr) => {
        seq!(N in 0..$ct {
            impl<
                #(
                    T~N : PersonProperty,
                )*
            > Query for (
                #(
                    (T~N, T~N::Value),
                )*
            )
            {
                fn setup(&self, context: &Context) {
                    #(
                        context.register_property::<T~N>();
                    )*
                }

                fn get_query(&self) -> Vec<(TypeId, IndexValue)> {
                    let mut ordered_items = vec![
                    #(
                        (std::any::TypeId::of::<T~N>(), IndexValue::compute(&self.N.1)),
                    )*
                    ];
                    ordered_items.sort_by(|a, b| a.0.cmp(&b.0));
                    ordered_items
                }
            }
        });
    }
}

seq!(Z in 1..20 {
    impl_query!(Z);
});

/// Helper utility for combining two queries, useful if you want
/// to iteratively construct a query in multiple parts.
///
/// Example:
/// ```
/// use ixa::{define_person_property, people::QueryAnd, Context, ContextPeopleExt};
/// define_person_property!(Age, u8);
/// define_person_property!(Alive, bool);
/// let context = Context::new();
/// let q1 = (Age, 42);
/// let q2 = (Alive, true);
/// context.query_people(QueryAnd::new(q1, q2));
/// ```

#[derive(Copy, Clone)]
pub struct QueryAnd<Q1, Q2>
where
    Q1: Query,
    Q2: Query,
{
    queries: (Q1, Q2),
}

impl<Q1, Q2> QueryAnd<Q1, Q2>
where
    Q1: Query,
    Q2: Query,
{
    pub fn new(q1: Q1, q2: Q2) -> Self {
        Self { queries: (q1, q2) }
    }
}

impl<Q1, Q2> Query for QueryAnd<Q1, Q2>
where
    Q1: Query,
    Q2: Query,
{
    fn setup(&self, context: &Context) {
        Q1::setup(&self.queries.0, context);
        Q2::setup(&self.queries.1, context);
    }

    fn get_query(&self) -> Vec<(TypeId, IndexValue)> {
        let mut query = Vec::new();
        query.extend_from_slice(&self.queries.0.get_query());
        query.extend_from_slice(&self.queries.1.get_query());
        query.sort_by(|a, b| a.0.cmp(&b.0));
        query
    }
}

#[cfg(test)]
mod tests {
    use crate::people::PeoplePlugin;
    use crate::people::{Query, QueryAnd};
    use crate::{
        define_derived_property, define_multi_property_index, define_person_property, Context,
        ContextPeopleExt,
    };
    use serde_derive::Serialize;
    use std::any::TypeId;

    define_person_property!(Age, u8);
    define_person_property!(County, u32);
    define_person_property!(Height, u32);
    define_derived_property!(AgeGroup, u8, [Age], |age| (age / 5));

    #[derive(Serialize, Copy, Clone, PartialEq, Eq, Debug)]
    pub enum RiskCategoryValue {
        High,
        Low,
    }

    define_person_property!(RiskCategory, RiskCategoryValue);

    #[test]
    fn query_people() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();

        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_empty() {
        let context = Context::new();

        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 0);
    }

    #[test]
    fn query_people_count() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();

        assert_eq!(
            context.query_people_count((RiskCategory, RiskCategoryValue::High)),
            1
        );
    }

    #[test]
    fn query_people_count_empty() {
        let context = Context::new();

        assert_eq!(
            context.query_people_count((RiskCategory, RiskCategoryValue::High)),
            0
        );
    }

    #[test]
    fn query_people_macro_index_first() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();
        context.index_property(RiskCategory);
        assert!(property_is_indexed::<RiskCategory>(&context));
        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 1);
    }

    fn property_is_indexed<T: 'static>(context: &Context) -> bool {
        context
            .get_data(PeoplePlugin)
            .get_index_ref(TypeId::of::<T>())
            .unwrap()
            .lookup
            .is_some()
    }

    #[test]
    fn query_people_macro_index_second() {
        let mut context = Context::new();
        let _ = context.add_person((RiskCategory, RiskCategoryValue::High));
        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert!(!property_is_indexed::<RiskCategory>(&context));
        assert_eq!(people.len(), 1);
        context.index_property(RiskCategory);
        assert!(property_is_indexed::<RiskCategory>(&context));
        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_macro_change() {
        let mut context = Context::new();
        let person1 = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();

        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 1);
        let people = context.query_people((RiskCategory, RiskCategoryValue::Low));
        assert_eq!(people.len(), 0);

        context.set_person_property(person1, RiskCategory, RiskCategoryValue::Low);
        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 0);
        let people = context.query_people((RiskCategory, RiskCategoryValue::Low));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_index_after_add() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();
        context.index_property(RiskCategory);
        assert!(property_is_indexed::<RiskCategory>(&context));
        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_add_after_index() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();
        context.index_property(RiskCategory);
        assert!(property_is_indexed::<RiskCategory>(&context));
        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 1);

        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();
        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 2);
    }

    #[test]
    // This is safe because we reindex only when someone queries.
    fn query_people_add_after_index_without_query() {
        let mut context = Context::new();
        let _ = context.add_person(()).unwrap();
        context.index_property(RiskCategory);
    }

    #[test]
    #[should_panic(expected = "Property not initialized")]
    // This will panic when we query.
    fn query_people_add_after_index_panic() {
        let mut context = Context::new();
        context.add_person(()).unwrap();
        context.index_property(RiskCategory);
        context.query_people((RiskCategory, RiskCategoryValue::High));
    }

    #[test]
    fn query_people_cast_value() {
        let mut context = Context::new();
        let _ = context.add_person((Age, 42)).unwrap();

        // Age is a u8, by default integer literals are i32; the macro should cast it.
        let people = context.query_people((Age, 42));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_intersection() {
        let mut context = Context::new();
        let _ = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::High)))
            .unwrap();
        let _ = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();
        let _ = context
            .add_person(((Age, 40), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();

        let people = context.query_people(((Age, 42), (RiskCategory, RiskCategoryValue::High)));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_intersection_non_macro() {
        let mut context = Context::new();
        let _ = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::High)))
            .unwrap();
        let _ = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();
        let _ = context
            .add_person(((Age, 40), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();

        let people = context.query_people(((Age, 42), (RiskCategory, RiskCategoryValue::High)));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_intersection_one_indexed() {
        let mut context = Context::new();
        let _ = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::High)))
            .unwrap();
        let _ = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();
        let _ = context
            .add_person(((Age, 40), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();

        context.index_property(Age);
        let people = context.query_people(((Age, 42), (RiskCategory, RiskCategoryValue::High)));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_derived_prop() {
        let mut context = Context::new();
        define_derived_property!(Senior, bool, [Age], |age| age >= 65);

        let person = context.add_person((Age, 64)).unwrap();
        let _ = context.add_person((Age, 88)).unwrap();

        // Age is a u8, by default integer literals are i32; the macro should cast it.
        let not_seniors = context.query_people((Senior, false));
        let seniors = context.query_people((Senior, true));
        assert_eq!(seniors.len(), 1, "One senior");
        assert_eq!(not_seniors.len(), 1, "One non-senior");

        context.set_person_property(person, Age, 65);

        let not_seniors = context.query_people((Senior, false));
        let seniors = context.query_people((Senior, true));

        assert_eq!(seniors.len(), 2, "Two seniors");
        assert_eq!(not_seniors.len(), 0, "No non-seniors");
    }

    #[test]
    fn query_derived_prop_with_index() {
        let mut context = Context::new();
        define_derived_property!(Senior, bool, [Age], |age| age >= 65);

        context.index_property(Senior);
        let person = context.add_person((Age, 64)).unwrap();
        let _ = context.add_person((Age, 88)).unwrap();

        // Age is a u8, by default integer literals are i32; the macro should cast it.
        let not_seniors = context.query_people((Senior, false));
        let seniors = context.query_people((Senior, true));
        assert_eq!(seniors.len(), 1, "One senior");
        assert_eq!(not_seniors.len(), 1, "One non-senior");

        context.set_person_property(person, Age, 65);

        let not_seniors = context.query_people((Senior, false));
        let seniors = context.query_people((Senior, true));

        assert_eq!(seniors.len(), 2, "Two seniors");
        assert_eq!(not_seniors.len(), 0, "No non-seniors");
    }

    #[test]
    fn query_derived_prop_with_optimized_index() {
        let mut context = Context::new();
        // create a 'regular' derived property
        define_derived_property!(
            Ach,
            (u8, u32, u32),
            [Age, County, Height],
            |age, county, height| { (age, county, height) }
        );

        // create a multi-property index
        define_multi_property_index!(Age, County, Height);
        define_multi_property_index!(County, Height);

        // add some people
        let _person = context
            .add_person(((Age, 64), (County, 2), (Height, 120)))
            .unwrap();
        let _ = context
            .add_person(((Age, 88), (County, 2), (Height, 130)))
            .unwrap();
        let p2 = context
            .add_person(((Age, 8), (County, 1), (Height, 140)))
            .unwrap();
        let p3 = context
            .add_person(((Age, 28), (County, 1), (Height, 140)))
            .unwrap();
        let p4 = context
            .add_person(((Age, 28), (County, 2), (Height, 160)))
            .unwrap();
        let p5 = context
            .add_person(((Age, 28), (County, 2), (Height, 160)))
            .unwrap();

        // 'regular' derived property
        let ach_people = context.query_people((Ach, (28, 2, 160)));
        assert_eq!(ach_people.len(), 2, "Should have 2 matches");
        assert!(ach_people.contains(&p4));
        assert!(ach_people.contains(&p5));

        // multi-property index
        let age_county_height2 = context.query_people(((Age, 28), (County, 2), (Height, 160)));
        assert_eq!(age_county_height2.len(), 2, "Should have 2 matches");
        assert!(age_county_height2.contains(&p4));
        assert!(age_county_height2.contains(&p5));

        // multi-property index with different order
        let age_county_height3 = context.query_people(((County, 2), (Height, 160), (Age, 28)));
        assert_eq!(age_county_height3.len(), 2, "Should have 2 matches");
        assert!(age_county_height3.contains(&p4));
        assert!(age_county_height3.contains(&p5));

        // multi-property index with different order
        let age_county_height4 = context.query_people(((Height, 160), (County, 2), (Age, 28)));
        assert_eq!(age_county_height4.len(), 2, "Should have 2 matches");
        assert!(age_county_height4.contains(&p4));
        assert!(age_county_height4.contains(&p5));

        // multi-property index with different order and different value
        let age_county_height5 = context.query_people(((Height, 140), (County, 1), (Age, 28)));
        assert_eq!(age_county_height5.len(), 1, "Should have 1 matches");
        assert!(age_county_height5.contains(&p3));

        context.set_person_property(p2, Age, 28);
        // multi-property index again after changing the value
        let age_county_height5 = context.query_people(((Height, 140), (County, 1), (Age, 28)));
        assert_eq!(age_county_height5.len(), 2, "Should have 2 matches");
        assert!(age_county_height5.contains(&p2));
        assert!(age_county_height5.contains(&p3));

        let age_county_height5 = context.query_people(((Height, 140), (County, 1)));
        assert_eq!(age_county_height5.len(), 2, "Should have 2 matches");
        assert!(age_county_height5.contains(&p2));
        assert!(age_county_height5.contains(&p3));
    }

    #[test]
    fn query_and_returns_people() {
        let mut context = Context::new();
        context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::High)))
            .unwrap();

        let q1 = (Age, 42);
        let q2 = (RiskCategory, RiskCategoryValue::High);

        let people = context.query_people(QueryAnd::new(q1, q2));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_and_conflicting() {
        let mut context = Context::new();
        context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::High)))
            .unwrap();

        let q1 = (Age, 42);
        let q2 = (Age, 64);

        let people = context.query_people(QueryAnd::new(q1, q2));
        assert_eq!(people.len(), 0);
    }

    fn query_and_copy_impl<Q: Query>(context: &Context, q: Q) {
        for _ in 0..2 {
            context.query_people(q);
        }
    }
    #[test]
    fn test_query_and_copy() {
        let mut context = Context::new();
        context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::High)))
            .unwrap();
        query_and_copy_impl(
            &context,
            QueryAnd::new((Age, 42), (RiskCategory, RiskCategoryValue::High)),
        );
    }
}
