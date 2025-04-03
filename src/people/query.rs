use crate::people::index::IndexValue;
use crate::people::PeoplePlugin;
use crate::{type_of, Context, ContextPeopleExt, HashSet, PersonId, PersonProperty};
use seq_macro::seq;
use std::any::TypeId;

/// Encapsulates a person query.
///
/// [`Context::query_people`] actually takes an instance of [`Query`], but because
/// we implement Query for tuples of up to size 20, that's invisible
/// to the caller. Do not use this trait directly.
pub trait Query: Copy {
    /// Registers each property in the query with the context and refreshes the indexes. Any work that requires
    /// a mutable reference to the context should be done here.
    fn setup(&self, context: &Context);
    /// Executes the query, accumulating the results with `accumulator`.
    fn execute_query(&self, context: &Context, accumulator: impl FnMut(PersonId));
    /// Checks that the given entity matches the query.
    fn match_entity(&self, context: &Context, entity: PersonId) -> bool;
    fn get_query(&self) -> Vec<(TypeId, IndexValue)>;
}

impl Query for () {
    fn setup(&self, _: &Context) {}
    fn execute_query(&self, _context: &Context, _accumulator: impl FnMut(PersonId)) {}
    fn match_entity(&self, _context: &Context, _entity: PersonId) -> bool {
        true
    }
    fn get_query(&self) -> Vec<(TypeId, IndexValue)> {
        vec![]
    }
}

// Implement the query version with one parameter.
impl<T1: PersonProperty> Query for (T1, T1::Value) {
    fn setup(&self, context: &Context) {
        T1::register(context);

        // 1. Refresh the indexes for each property in the query.
        let data_container = context.get_data_container(PeoplePlugin).unwrap();
        data_container.index_unindexed_people::<T1>(context);
    }

    fn execute_query(&self, context: &Context, mut accumulator: impl FnMut(PersonId)) {
        let people_data = context.get_data_container(PeoplePlugin).unwrap();
        let index_map = people_data.property_indexes.borrow();
        let mut indexes: Vec<&HashSet<PersonId>> = Vec::new();
        // A vector of closures that look up a property for an `people_id`
        let mut unindexed: Vec<Box<dyn Fn(PersonId) -> bool>> = Vec::new();

        {
            // 1. Refresh the indexes for each property in the query.
            //    Done in setup.

            // 2. Collect the index entry corresponding to the value.
            let index = index_map.get_container_ref::<T1>().unwrap();
            let hash_value = IndexValue::compute(&self.1);
            if let Some(lookup) = &index.lookup {
                if let Some((_, people)) = lookup.get(&hash_value) {
                    indexes.push(people);
                } else {
                    // This is empty and so the intersection will also be empty.
                    return;
                }
            } else {
                // No index, so we'll get to this after.
                unindexed.push(Box::new(move |people_id: PersonId| {
                    match T1::compute(context, people_id) {
                        Some(value) => hash_value == IndexValue::compute(&value),
                        _ => false,
                    }
                }));
            }
        }

        // 3. Create an iterator over entities, based on either:
        //    (1) the smallest index if there is one.
        //    (2) the overall entity count if there are no indices.
        // let people_data = context.get_data_container::<PeopleData>().unwrap();
        let to_check: Box<dyn Iterator<Item = PersonId>> = if indexes.is_empty() {
            Box::new(people_data.people_iterator())
        } else {
            let mut min_len: usize = usize::MAX;
            let mut shortest_idx: usize = 0;
            for (idx, index_iter) in indexes.iter().enumerate() {
                if index_iter.len() < min_len {
                    shortest_idx = idx;
                    min_len = index_iter.len();
                }
            }
            Box::new(indexes.remove(shortest_idx).iter().copied())
        };

        // 4. Walk over the iterator and add entities to the result iff:
        //    (1) they exist in all the indexes
        //    (2) they match the unindexed properties
        'outer: for people_id in to_check {
            // (1) check all the indexes
            for &index in &indexes {
                if !index.contains(&people_id) {
                    continue 'outer;
                }
            }

            // (2) check the unindexed properties
            for hash_lookup in &unindexed {
                if !hash_lookup(people_id) {
                    continue 'outer;
                }
            }

            // This matches.
            accumulator(people_id);
        }
    }

    fn match_entity(&self, context: &Context, entity: PersonId) -> bool {
        context.get_person_property(entity, T1::get_instance()) == self.1
    }

    fn get_query(&self) -> Vec<(TypeId, IndexValue)> {
        vec![(type_of::<T1>(), IndexValue::compute(&self.1))]
    }
}

// Implement the versions with 1..20 parameters.
macro_rules! impl_query {
    ($ct:expr) => {
        $crate::seq!(N in 0..$ct {
            impl<
                #(
                    T~N : $crate::PersonProperty,
                )*
            > Query for (
                #(
                    (T~N, T~N::Value),
                )*
            )
            {
                fn setup(&self, context: &$crate::Context) {
                #(
                    <T~N>::register(context);
                )*
                    // 1. Refresh the indexes for each property in the query.
                    let data_container = context.get_data_container($crate::people::PeoplePlugin).unwrap();
                #(
                    data_container.index_unindexed_people::<T~N>(context);
                )*
                }


                fn execute_query(&self, context: &$crate::Context, mut accumulator: impl FnMut($crate::PersonId)) {
                    let people_data = context.get_data_container($crate::people::PeoplePlugin).unwrap();
                    let index_map = people_data.property_indexes.borrow();
                    let mut indexes: Vec<&HashSet<$crate::PersonId>> = Vec::new();
                    // A vector of closures that look up a property for an `people_id`
                    let mut unindexed: Vec<Box<dyn Fn(PersonId) -> bool>> = Vec::new();

                    // 1. Refresh the indexes for each property in the query.
                    //    Done in setup.
                #(
                    {
                        // 2. Collect the index entry corresponding to the value.
                        let index = index_map.get_container_ref::<T~N>().unwrap();
                        let hash_value = $crate::people::index::IndexValue::compute(&self.N.1);
                        if let Some(lookup) = &index.lookup {
                            if let Some((_, people)) = lookup.get(&hash_value) {
                                indexes.push(people);
                            } else {
                                // This is empty and so the intersection will also be empty.
                                return;
                            }
                        } else {
                            // No index, so we'll get to this after.
                            unindexed.push(
                                Box::new(
                                    move
                                    |people_id: $crate::PersonId| {
                                        match <T~N>::compute(context, people_id) {
                                            Some(value) => {
                                                hash_value == $crate::people::index::IndexValue::compute(&value)
                                            }
                                            _ => { false }
                                        }
                                    }
                                )
                            );
                        }
                    }
                )*
                    // 3. Create an iterator over entities, based on either:
                    //    (1) the smallest index if there is one.
                    //    (2) the overall population if there are no indices.
                    let to_check: Box<dyn Iterator<Item = $crate::PersonId>> =
                        if indexes.is_empty() {
                            Box::new(people_data.people_iterator())
                        } else {
                            let mut min_len: usize = usize::MAX;
                            let mut shortest_idx: usize = 0;
                            for (idx, index_iter) in indexes.iter().enumerate() {
                                if index_iter.len() < min_len {
                                    shortest_idx = idx;
                                    min_len = index_iter.len();
                                }
                            }
                            Box::new(indexes.remove(shortest_idx).iter().cloned())
                        };

                    // 4. Walk over the iterator and add entity to the result iff:
                    //    (1) they exist in all the indexes
                    //    (2) they match the unindexed properties
                    'outer: for people_id in to_check {
                        // (1) check all the indexes
                        for &index in &indexes {
                            if !index.contains(&people_id) {
                                continue 'outer;
                            }
                        }

                        // (2) check the unindexed properties
                        for hash_lookup in &unindexed {
                            if !hash_lookup(people_id) {
                                continue 'outer;
                            }
                        }

                        // This matches.
                        accumulator(people_id);
                    }
                }

                fn match_entity(&self, context: &$crate::Context, person_id: $crate::PersonId) -> bool {
                    #(
                        if context.get_person_property(person_id, <T~N>::get_instance()) != self.N.1
                        {
                            return false;
                        }
                    )*
                    // Matches every property in the query
                    true
                }

                fn get_query(&self) -> Vec<($crate::TypeId, $crate::people::index::IndexValue)> {
                    vec![
                    #(
                        ($crate::type_of::<T~N>(), $crate::people::index::IndexValue::compute(&self.N.1)),
                    )*
                    ]
                }
            }
        });
    }
}

seq!(Z in 1..20 {
    impl_query!(Z);
});

/*
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
        query
    }
}
*/

#[cfg(test)]
mod tests {
    use crate::{
        define_derived_property, define_person_property, define_person_property_with_default,
        define_rng, Context, ContextPeopleExt, ContextRandomExt, HashSet, PersonId, PersonProperty,
    };
    use serde_derive::{Deserialize, Serialize};

    define_person_property!(Age, u8);

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

    fn property_is_indexed<T: PersonProperty>(context: &Context) -> bool {
        context.property_is_indexed::<T>()
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

    define_rng!(QueryTestRng);
    #[derive(Deserialize, Serialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
    pub enum QueryTestAgeGroupRisk {
        NewBorn,
        General,
        OldAdult,
    }
    define_person_property_with_default!(QueryTestAlive, bool, true);
    define_derived_property!(QueryTestAgeGroupFoi, QueryTestAgeGroupRisk, [Age], |age| {
        if age <= 1 {
            QueryTestAgeGroupRisk::NewBorn
        } else if age <= 65 {
            QueryTestAgeGroupRisk::General
        } else {
            QueryTestAgeGroupRisk::OldAdult
        }
    });

    #[test]
    fn test_derived_nonderived_query() {
        let mut context = Context::new();
        context.init_random(42);

        for _ in 0..100 {
            let age: u8 = context.sample_range(QueryTestRng, 0..100);
            let person = context.add_person((Age, age)).unwrap();

            // Demonstrate that people exist with the properties we expect
            let _ = context.get_person_property(person, Age);
            let checked_alive = context.get_person_property(person, QueryTestAlive);
            assert!(checked_alive);
        }
        // Make sure both single property and tuple singelton queries retrieve the same people.
        // Tuple with single item
        let alive_query_result_tuple = context.query_people(((QueryTestAlive, true),));
        assert_eq!(alive_query_result_tuple.len(), 100);
        // Nontuple query.
        let alive_query_result = context.query_people((QueryTestAlive, true));
        assert_eq!(alive_query_result.len(), 100);
        assert_eq!(alive_query_result_tuple, alive_query_result);

        // Do the same as previous but with the other property.
        // Tuple with single item
        let age_group_tuple_query_result = context.query_people((
            // (QueryTestAlive, true),
            (QueryTestAgeGroupFoi, QueryTestAgeGroupRisk::General),
        ));
        assert!(!age_group_tuple_query_result.is_empty());
        // Nontuple query.
        let age_group_query_result =
            context.query_people((QueryTestAgeGroupFoi, QueryTestAgeGroupRisk::General));
        assert!(!age_group_query_result.is_empty());
        assert_eq!(age_group_tuple_query_result, age_group_query_result);

        // Now do multi-property query
        let multi_property_query = context.query_people((
            (QueryTestAlive, true),
            (QueryTestAgeGroupFoi, QueryTestAgeGroupRisk::General),
        ));
        assert!(!multi_property_query.is_empty());

        // For good measure, check that the intersection of the first two single-property queries
        // gives the third query.
        let intersection: HashSet<PersonId> = alive_query_result
            .into_iter()
            .collect::<HashSet<_>>()
            .intersection(&age_group_query_result.into_iter().collect::<HashSet<_>>())
            .copied()
            .collect();
        let expected: HashSet<PersonId> = multi_property_query.into_iter().collect();
        assert_eq!(
            intersection, expected,
            "The intersection of the first two vectors does not match the third vector"
        );
    }
    /*
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

    fn query_and_copy_impl<Q: Query>(context: &mut Context, q: Q) {
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
            &mut context,
            QueryAnd::new((Age, 42), (RiskCategory, RiskCategoryValue::High)),
        );
    }
    */
}
