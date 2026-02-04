//! A `QueryResultIterator` encapsulates the execution of a query, presenting the results as
//! an iterator.
//!
//! You normally create a `QueryResultIterator` by calling `context.query_entity(some_query)`,
//! which returns a `QueryResultIterator` instance.
//!
//! A `QueryResultIterator` can be thought of abstractly as representing the intersection of
//! a set of `SourceSet`s. Internally, `QueryResultIterator` holds its state in a set of
//! `SourceSet` instances and a `SourceIterator`, which is an iterator created from a
//! `SourceSet` and represents the first of the sets in the intersection. To produce the "next"
//! `EntityId<E>` for a call to `QueryResultIterator::next()`, we need to iterate over the
//! `SourceIterator` until we find an entity ID that is contained in all of the `SourceSet`s
//! simultaneously.
//!
//! A `QueryResultIterator` holds an immutable reference to the `Context`, so
//! operations that mutate `Context` will be forbidden by the compiler statically. The results
//! can be collected into a `Vec` (or other container) with the `collect` idiom for use cases
//! where you want a mutable copy of the result set. If you don't need a mutable copy, use
//! `Context::with_query_results` instead, as it is much more efficient for indexed queries.

use std::cell::Ref;

use log::warn;
use rand::Rng;

use crate::entity::query::source_set::{SourceIterator, SourceSet};
use crate::entity::{Entity, EntityId, EntityIterator};
use crate::hashing::IndexSet;
use crate::random::{sample_multiple_l_reservoir, sample_single_l_reservoir};

/// An iterator over the results of a query, producing `EntityId<E>`s until exhausted.
pub struct QueryResultIterator<'c, E: Entity> {
    source: SourceIterator<'c, E>,
    sources: Vec<SourceSet<'c, E>>,
}

impl<'c, E: Entity> QueryResultIterator<'c, E> {
    /// Create a new empty `QueryResultIterator` for situations where you know
    /// there are no results but need a `QueryResultIterator`.
    pub fn empty() -> QueryResultIterator<'c, E> {
        QueryResultIterator {
            source: SourceIterator::Empty,
            sources: vec![],
        }
    }

    /// Create a new `QueryResultIterator` that iterates over the entire population if entities.
    /// This is used, for example, when the query is the empty query.
    pub(super) fn from_population_iterator(iter: EntityIterator<E>) -> Self {
        QueryResultIterator {
            source: SourceIterator::WholePopulation(iter),
            sources: vec![],
        }
    }

    /// Create a new `QueryResultIterator` from a provided list of sources.
    /// The sources need not be sorted.
    pub fn from_sources(mut sources: Vec<SourceSet<'c, E>>) -> Self {
        if sources.is_empty() {
            return Self::empty();
        }

        sources.sort_unstable_by_key(|x| x.upper_len());
        let source = sources.remove(0).into_iter();
        QueryResultIterator { source, sources }
    }

    pub fn from_index_set(set: Ref<'c, IndexSet<EntityId<E>>>) -> QueryResultIterator<'c, E> {
        QueryResultIterator {
            source: SourceSet::IndexSet(set).into_iter(),
            sources: vec![],
        }
    }

    /// Sample a single entity uniformly from the query results. Returns `None` if the
    /// query's result set is empty.
    pub fn sample_entity<R>(mut self, rng: &mut R) -> Option<EntityId<E>>
    where
        R: Rng,
    {
        // The known length case
        let (lower, upper) = self.size_hint();
        if Some(lower) == upper {
            if lower == 0 {
                warn!("Requested a sample entity from an empty population");
                return None;
            }
            // This little trick with `u32` makes this function 30% faster.
            let index = rng.random_range(0..lower as u32);
            return self.nth(index as usize);
        }

        // Slow path
        sample_single_l_reservoir(rng, self)
    }

    /// Sample up to `requested` entities uniformly from the query results. If the
    /// query's result set has fewer than `requested` entities, the entire result
    /// set is returned.
    pub fn sample_entities<R>(self, rng: &mut R, requested: usize) -> Vec<EntityId<E>>
    where
        R: Rng,
    {
        sample_multiple_l_reservoir(rng, self, requested)
    }
}

impl<'a, E: Entity> Iterator for QueryResultIterator<'a, E> {
    type Item = EntityId<E>;

    fn next(&mut self) -> Option<Self::Item> {
        // Walk over the iterator and return an entity iff:
        //    (1) they exist in all the indexes
        //    (2) they match the unindexed properties
        'outer: for entity_id in self.source.by_ref() {
            // (1) check all the indexes
            for source in &self.sources {
                if !source.contains(entity_id) {
                    continue 'outer;
                }
            }

            // This entity matches.
            return Some(entity_id);
        }

        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.source.size_hint();
        if self.sources.is_empty() {
            (lower, upper)
        } else {
            // The intersection may be empty but cannot have more than the
            // upper bound of the source set.
            (0, upper)
        }
    }

    fn count(self) -> usize {
        if self.sources.is_empty() {
            // Fast path, as some source types have fast count impls.
            self.source.count()
        } else {
            self.fold(0, |n, _| n + 1)
        }
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if self.sources.is_empty() {
            // Fast path: delegate to the underlying source iterator,
            // which may have an optimized `nth` implementation.
            self.source.nth(n)
        } else {
            // General path: advance through the filtered iterator.
            // `nth(n)` is equivalent to skipping `n` items and returning the next.
            for _ in 0..n {
                self.next()?;
            }
            self.next()
        }
    }
}
impl<'c, E: Entity> std::iter::FusedIterator for QueryResultIterator<'c, E> {}

#[cfg(test)]
mod tests {
    #![allow(unused_macros)]
    /*!
    ## Test Matrix

    | Test # | PropertyInitializationKind | is_default_value | Indexed | Initial Source Position |
    | ------ | -------------------------- | ---------------- | ------- | ----------------------- |
    | 1      | Explicit                   | N/A              | No      | Yes                     |
    | 2      | Explicit                   | N/A              | Yes     | Yes                     |
    | 3      | Constant                   | true             | No      | Yes                     |
    | 4      | Constant                   | true             | Yes     | Yes                     |
    | 5      | Constant                   | false            | No      | Yes                     |
    | 6      | Constant                   | false            | Yes     | Yes                     |
    | 7      | Derived                    | N/A              | No      | Yes                     |
    | 8      | Derived                    | N/A              | Yes     | Yes                     |
    | 9      | Explicit                   | N/A              | No      | No                      |
    | 10     | Explicit                   | N/A              | Yes     | No                      |
    | 11     | Constant                   | true             | No      | No                      |
    | 12     | Constant                   | false            | Yes     | No                      |
    | 13     | Derived                    | N/A              | No      | No                      |
    | 14     | Derived                    | N/A              | Yes     | No                      |

    The tests use multi-property queries (tests 9-14) where one property
    is indexed with fewer results to ensure it becomes the "smallest source
    set", making the tested property NOT the initial source position.
    */

    use indexmap::IndexSet;

    use crate::prelude::*;
    use crate::{all, define_derived_property, define_property};

    define_entity!(Person);

    // Test properties covering different initialization kinds

    // Explicit (Normal) property - no default, requires explicit initialization
    define_property!(struct ExplicitProp(u8), Person);

    // Constant property - has a constant default value
    define_property!(struct ConstantProp(u8), Person, default_const = ConstantProp(42));

    // Derived property - computed from other properties
    define_derived_property!(struct DerivedProp(bool), Person, [ExplicitProp], |explicit| {
        DerivedProp(explicit.0 % 2 == 0)
    });

    // Additional properties for multi-property queries
    define_property!(struct ConstantProp2(u16), Person, default_const = ConstantProp2(100));
    define_property!(struct ExplicitProp2(bool), Person);

    define_property!(struct Age(u8), Person, default_const = Age(0));
    define_property!(struct Alive(bool), Person, default_const = Alive(true));

    define_derived_property!(
        enum AgeGroupRisk {
            NewBorn,
            General,
            OldAdult,
        },
        Person,
        [Age],
        [],
        |age| {
            if age.0 <= 1 {
                AgeGroupRisk::NewBorn
            } else if age.0 <= 65 {
                AgeGroupRisk::General
            } else {
                AgeGroupRisk::OldAdult
            }
        }
    );

    // Helper function to create a population for testing
    fn setup_test_population(context: &mut Context, size: usize) -> Vec<EntityId<Person>> {
        let mut people = Vec::new();
        for i in 0..size {
            let person = context
                .add_entity(all!(
                    Person,
                    ExplicitProp((i % 20) as u8),
                    ExplicitProp2(i % 2 == 0)
                ))
                .unwrap();
            people.push(person);
        }
        people
    }

    // region: Test Matrix Tests

    // Test 1: Explicit property, non-default value, not indexed, initial source position = Yes
    #[test]
    fn test_explicit_non_default_not_indexed_initial_source_yes() {
        let mut context = Context::new();
        setup_test_population(&mut context, 100);

        let results = context
            .query_result_iterator(all!(Person, ExplicitProp(5)))
            .collect::<Vec<_>>();

        assert_eq!(results.len(), 5); // 5, 25, 45, 65, 85
        for person in results {
            assert_eq!(
                context.get_property::<_, ExplicitProp>(person),
                ExplicitProp(5)
            );
        }
    }

    // Test 2: Explicit property, non-default value, indexed, initial source position = Yes
    #[test]
    fn test_explicit_non_default_indexed_initial_source_yes() {
        let mut context = Context::new();
        context.index_property::<Person, ExplicitProp>();
        setup_test_population(&mut context, 100);

        let results = context
            .query_result_iterator(all!(Person, ExplicitProp(7)))
            .collect::<Vec<_>>();

        assert_eq!(results.len(), 5); // 7, 27, 47, 67, 87
        for person in results {
            assert_eq!(
                context.get_property::<_, ExplicitProp>(person),
                ExplicitProp(7)
            );
        }
    }

    // Test 3: Constant property, default value, not indexed, initial source position = Yes
    #[test]
    fn test_constant_default_not_indexed_initial_source_yes() {
        let mut context = Context::new();
        // Create people without setting ConstantProp - they'll use default value 42
        for _ in 0..50 {
            context
                .add_entity(all!(Person, ExplicitProp(1), ExplicitProp2(false)))
                .unwrap();
        }

        let results = context
            .query_result_iterator(all!(Person, ConstantProp(42), ExplicitProp2(false)))
            .collect::<Vec<_>>();

        assert_eq!(results.len(), 50);
        for person in results {
            assert_eq!(
                context.get_property::<_, ConstantProp>(person),
                ConstantProp(42)
            );
        }
    }

    // Test 4: Constant property, default value, indexed, initial source position = Yes
    #[test]
    fn test_constant_default_indexed_initial_source_yes() {
        let mut context = Context::new();
        context.index_property::<Person, ConstantProp>();

        for _ in 0..50 {
            context
                .add_entity(all!(Person, ExplicitProp(1), ExplicitProp2(false)))
                .unwrap();
        }

        let results = context
            .query_result_iterator(all!(Person, ConstantProp(42)))
            .collect::<Vec<_>>();
        assert_eq!(results.len(), 50);
    }

    // Test 5: Constant property, non-default value, not indexed, initial source position = Yes
    #[test]
    fn test_constant_non_default_not_indexed_initial_source_yes() {
        let mut context = Context::new();

        for i in 0..50 {
            if i < 10 {
                context
                    .add_entity(all!(
                        Person,
                        ExplicitProp(1),
                        ExplicitProp2(false),
                        ConstantProp(99)
                    ))
                    .unwrap();
            } else {
                context
                    .add_entity(all!(Person, ExplicitProp(1), ExplicitProp2(false)))
                    .unwrap();
            }
        }

        let results = context
            .query_result_iterator(all!(Person, ConstantProp(99)))
            .collect::<Vec<_>>();

        assert_eq!(results.len(), 10);
        for person in results {
            assert_eq!(
                context.get_property::<_, ConstantProp>(person),
                ConstantProp(99)
            );
        }
    }

    // Test 6: Constant property, non-default value, indexed, initial source position = Yes
    #[test]
    fn test_constant_non_default_indexed_initial_source_yes() {
        let mut context = Context::new();
        context.index_property::<Person, ConstantProp>();

        for i in 0..50 {
            if i < 10 {
                context
                    .add_entity(all!(
                        Person,
                        ExplicitProp(1),
                        ExplicitProp2(false),
                        ConstantProp(99)
                    ))
                    .unwrap();
            } else {
                context
                    .add_entity(all!(Person, ExplicitProp(1), ExplicitProp2(false)))
                    .unwrap();
            }
        }

        let results = context
            .query_result_iterator(all!(Person, ConstantProp(99)))
            .collect::<Vec<_>>();

        assert_eq!(results.len(), 10);
    }

    // Test 7: Derived property, not indexed, initial source position = Yes
    #[test]
    fn test_derived_not_indexed_initial_source_yes() {
        let mut context = Context::new();

        for i in 0..100 {
            context
                .add_entity(all!(Person, ExplicitProp(i as u8), ExplicitProp2(false)))
                .unwrap();
        }

        let results = context
            .query_result_iterator(all!(Person, DerivedProp(true)))
            .collect::<Vec<_>>();

        // DerivedProp is true when ExplicitProp is even
        assert_eq!(results.len(), 50);
        for person in results {
            assert_eq!(
                context.get_property::<Person, DerivedProp>(person),
                DerivedProp(true)
            );
        }
    }

    // Test 8: Derived property, indexed, initial source position = Yes
    #[test]
    fn test_derived_indexed_initial_source_yes() {
        let mut context = Context::new();
        context.index_property::<Person, DerivedProp>();

        for i in 0..100 {
            context
                .add_entity(all!(Person, ExplicitProp(i as u8), ExplicitProp2(false)))
                .unwrap();
        }

        let results = context
            .query_result_iterator(all!(Person, DerivedProp(false)))
            .collect::<Vec<_>>();

        // DerivedProp is false when ExplicitProp is odd
        assert_eq!(results.len(), 50);
        for person in results {
            assert_eq!(
                context.get_property::<Person, DerivedProp>(person),
                DerivedProp(false)
            );
        }
    }

    // Test 9-14: Initial source position = No (multi-property queries where property is NOT the smallest source)

    // Test 9: Explicit property, non-default, not indexed, initial source = No
    #[test]
    fn test_explicit_non_default_not_indexed_initial_source_no() {
        let mut context = Context::new();
        context.index_property::<Person, ExplicitProp2>(); // Index the other property so it's the smallest

        for i in 0..100 {
            context
                .add_entity(all!(
                    Person,
                    ExplicitProp((i % 20) as u8),
                    ExplicitProp2(i % 2 == 0)
                ))
                .unwrap();
        }

        let results = context
            .query_result_iterator(all!(Person))
            .collect::<Vec<_>>();
        for person in results {
            let explicit_prop = context.get_property::<Person, ExplicitProp>(person);
            let explicit_prop2 = context.get_property::<Person, ExplicitProp2>(person);
            println!("({:?} {:?} {:?})", person, explicit_prop, explicit_prop2);
        }

        // ExplicitProp2 has only 2 values, so it will be the smaller source
        let results = context
            .query_result_iterator(all!(Person, ExplicitProp(5), ExplicitProp2(false)))
            .collect::<Vec<_>>();

        // Looking for ExplicitProp=5 AND ExplicitProp2=true
        // ExplicitProp cycles 0-19, ExplicitProp2 alternates
        // Matches: 4 (since 5,25,45,65,85 but only even indices)
        let expected = results.len();
        assert!(expected > 0);
        for person in results {
            assert_eq!(
                context.get_property::<Person, ExplicitProp>(person),
                ExplicitProp(5)
            );
            assert_eq!(
                context.get_property::<Person, ExplicitProp2>(person),
                ExplicitProp2(false)
            );
        }
    }

    // Test 10: Explicit property, non-default, indexed, initial source = No
    #[test]
    fn test_explicit_non_default_indexed_initial_source_no() {
        let mut context = Context::new();
        context.index_property::<Person, ExplicitProp>();
        context.index_property::<Person, ConstantProp2>(); // ConstantProp2 will likely be the smaller source

        for i in 0..100 {
            if i < 10 {
                context
                    .add_entity(all!(
                        Person,
                        ExplicitProp(7),
                        ExplicitProp2(false),
                        ConstantProp2(200) // Non-default for smaller source
                    ))
                    .unwrap();
            } else {
                context
                    .add_entity(all!(
                        Person,
                        ExplicitProp((i % 20) as u8),
                        ExplicitProp2(false)
                    ))
                    .unwrap();
            }
        }

        let results = context
            .query_result_iterator(all!(Person, ExplicitProp(7), ConstantProp2(200)))
            .collect::<Vec<_>>();

        assert_eq!(results.len(), 10);
    }

    // Test 11: Constant property, default value, not indexed, initial source = No
    #[test]
    fn test_constant_default_not_indexed_initial_source_no() {
        let mut context = Context::new();
        context.index_property::<Person, ExplicitProp>(); // Make ExplicitProp the smaller source

        for i in 0..100 {
            if i < 5 {
                context
                    .add_entity(all!(Person, ExplicitProp(99), ExplicitProp2(false)))
                    .unwrap(); // ConstantProp uses default
            } else {
                context
                    .add_entity(all!(
                        Person,
                        ExplicitProp((i % 20) as u8),
                        ExplicitProp2(false)
                    ))
                    .unwrap();
            }
        }

        let results = context
            .query_result_iterator(all!(Person, ExplicitProp(99), ConstantProp(42)))
            .collect::<Vec<_>>();

        assert_eq!(results.len(), 5);
        for person in results {
            assert_eq!(
                context.get_property::<Person, ConstantProp>(person),
                ConstantProp(42)
            );
        }
    }

    // Test 12: Constant property, non-default, indexed, initial source = No
    #[test]
    fn test_constant_non_default_indexed_initial_source_no() {
        let mut context = Context::new();
        context.index_property::<Person, ConstantProp>();
        context.index_property::<Person, ExplicitProp2>();

        for i in 0..100 {
            if i < 10 {
                context
                    .add_entity(all!(
                        Person,
                        ConstantProp(99),
                        ExplicitProp(0),
                        ExplicitProp2(true)
                    ))
                    .unwrap();
            } else {
                context
                    .add_entity(all!(Person, ExplicitProp(0), ExplicitProp2(false)))
                    .unwrap();
            }
        }

        let results = context
            .query_result_iterator(all!(Person, ConstantProp(99), ExplicitProp2(true)))
            .collect::<Vec<_>>();

        assert_eq!(results.len(), 10);
    }

    // Test 13: Derived property, not indexed, initial source = No
    #[test]
    fn test_derived_not_indexed_initial_source_no() {
        let mut context = Context::new();
        context.index_property::<Person, ExplicitProp2>();

        for i in 0..100 {
            context
                .add_entity(all!(Person, ExplicitProp(i as u8), ExplicitProp2(i < 50)))
                .unwrap();
        }

        let results = context
            .query_result_iterator(all!(Person, ExplicitProp2(true), DerivedProp(true)))
            .collect::<Vec<_>>();

        // ExplicitProp2=true for i<50, DerivedProp=true when ExplicitProp is even
        // So we want i<50 AND i is even: 0,2,4,...,48 = 25 people
        assert_eq!(results.len(), 25);
    }

    // Test 14: Derived property, indexed, initial source = No
    #[test]
    fn test_derived_indexed_initial_source_no() {
        let mut context = Context::new();
        context.index_property::<Person, DerivedProp>();
        context.index_property::<Person, ExplicitProp2>();

        for i in 0..100 {
            context
                .add_entity(all!(Person, ExplicitProp(i as u8), ExplicitProp2(i < 30)))
                .unwrap();
        }

        let results = context
            .query_result_iterator(all!(Person, ExplicitProp2(true), DerivedProp(false)))
            .collect::<Vec<_>>();

        // ExplicitProp2=true for i<30, DerivedProp=false when ExplicitProp is odd
        // So we want i < 30 AND i is odd: 1,3,5,...,29 = 15 people
        assert_eq!(results.len(), 15);
    }

    // endregion Test Matrix Tests

    #[test]
    fn test_multiple_query_result_iterators() {
        let mut context = Context::new();
        context.index_property::<Person, Age>();

        for age in 0..100 {
            context
                .add_entity(all!(
                    Person,
                    Age(age),
                    ExplicitProp(age.wrapping_mul(7) % 100),
                    ExplicitProp2(false)
                ))
                .unwrap();
        }
        for age in 0..100 {
            context
                .add_entity(all!(
                    Person,
                    Age(age),
                    ExplicitProp(age.wrapping_mul(14) % 100),
                    ExplicitProp2(false)
                ))
                .unwrap();
        }

        // Since both queries include `Age`, both will attempt to index unindexed entities. This tests that there is
        // no double borrow error.
        let results = context.query_result_iterator(all!(Person, Age(25)));
        let more_results = context.query_result_iterator(all!(Person, Age(25), ExplicitProp(75)));

        let collected_results = results.collect::<IndexSet<_>>();
        let other_collected_results = more_results.collect::<IndexSet<_>>();
        let intersection_count = collected_results
            .intersection(&other_collected_results)
            .count();
        assert_eq!(intersection_count, 1);
    }
}
