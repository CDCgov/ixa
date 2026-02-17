//! Iterator implementation for [`EntitySet`].
//!
//! `EntitySetIterator` mirrors an `EntitySet` expression tree and evaluates nodes lazily.
//! - `Source` iterates a concrete backing source (`Population`, singleton `Entity`, index set,
//!   or property-backed source).
//! - `Intersection` and `Difference` drive iteration from one branch and filter candidates using
//!   membership checks.
//! - `Union` yields the left branch first, then lazily activates the right branch (`Pending` to
//!   `Active`) and filters out any IDs already present in the left branch via membership checks.
//!
//! The iterator is created through `EntitySet::into_iter()`.

use std::cell::Ref;

use log::warn;
use rand::Rng;

use crate::entity::entity_set::entity_set::{EntitySet, EntitySetInner};
use crate::entity::entity_set::source_iterator::SourceIterator;
use crate::entity::entity_set::source_set::SourceSet;
use crate::entity::{Entity, EntityId, PopulationIterator};
use crate::hashing::IndexSet;
use crate::random::{
    count_and_sample_single_l_reservoir, sample_multiple_from_known_length,
    sample_multiple_l_reservoir, sample_single_l_reservoir,
};

enum EntitySetIteratorInner<'a, E: Entity> {
    Empty,
    Source(SourceIterator<'a, E>),
    // The `IntersectionSources` variant is a micro-optimization to avoid recursive
    // `EntitySet` membership checks in the most common case, improving tight-loop
    // benchmark performance by 5%-15%.
    IntersectionSources {
        driver: SourceIterator<'a, E>,
        filters: Vec<SourceSet<'a, E>>,
    },
    Intersection {
        driver: Box<EntitySetIteratorInner<'a, E>>,
        filters: Vec<EntitySet<'a, E>>,
    },
    Difference {
        left: Box<EntitySetIteratorInner<'a, E>>,
        right: EntitySet<'a, E>,
    },
    Union {
        left: Box<EntitySetIteratorInner<'a, E>>,
        right: UnionRightState<'a, E>,
    },
}

enum UnionRightState<'a, E: Entity> {
    Pending(Option<EntitySet<'a, E>>),
    Active(Box<EntitySetIteratorInner<'a, E>>),
}

impl<'a, E: Entity> EntitySetIteratorInner<'a, E> {
    fn from_entity_set(set: EntitySet<'a, E>) -> Self {
        match set.into_inner() {
            EntitySetInner::Source(source) => {
                if matches!(source, SourceSet::Empty) {
                    Self::Empty
                } else {
                    Self::Source(source.into_iter())
                }
            }
            EntitySetInner::Intersection(mut sets) => {
                if sets.is_empty() {
                    return Self::Empty;
                }
                if sets.len() == 1 {
                    return Self::from_entity_set(sets.pop().unwrap());
                }

                if sets.iter().all(EntitySet::is_source_leaf) {
                    // `EntitySet::Intersection` stores operands sorted ascending by `cost_hint`.
                    // Keep that order so membership checks short-circuit early on small filters.
                    let mut set_iter = sets.into_iter();

                    let first = set_iter.next().unwrap().into_source_leaf().unwrap();
                    let driver = first.into_iter();

                    let filters = set_iter
                        .map(|set| set.into_source_leaf().unwrap())
                        .collect();

                    return Self::IntersectionSources { driver, filters };
                }

                // `EntitySet::Intersection` stores operands sorted ascending by `cost_hint`.
                // Use the first set as the iteration driver and keep the remaining filters in
                // ascending order for short-circuit-friendly `contains` checks.
                let mut set_iter = sets.into_iter();
                let driver = Box::new(Self::from_entity_set(set_iter.next().unwrap()));
                Self::Intersection {
                    driver,
                    filters: set_iter.collect(),
                }
            }
            EntitySetInner::Difference(left, right) => Self::Difference {
                left: Box::new(Self::from_entity_set(*left)),
                right: *right,
            },
            EntitySetInner::Union(left, right) => Self::Union {
                left: Box::new(Self::from_entity_set(*left)),
                right: UnionRightState::Pending(Some(*right)),
            },
        }
    }

    fn contains(&self, entity_id: EntityId<E>) -> bool {
        match self {
            Self::Empty => false,
            Self::Source(iter) => iter.contains(entity_id),
            Self::IntersectionSources { driver, filters } => {
                driver.contains(entity_id) && filters.iter().all(|set| set.contains(entity_id))
            }
            Self::Intersection { driver, filters } => {
                driver.contains(entity_id) && filters.iter().all(|set| set.contains(entity_id))
            }
            Self::Difference { left, right } => {
                left.contains(entity_id) && !right.contains(entity_id)
            }
            Self::Union { left, right } => left.contains(entity_id) || right.contains(entity_id),
        }
    }
}

impl<'a, E: Entity> UnionRightState<'a, E> {
    fn contains(&self, entity_id: EntityId<E>) -> bool {
        match self {
            Self::Pending(Some(set)) => set.contains(entity_id),
            Self::Pending(None) => false,
            Self::Active(iter) => iter.contains(entity_id),
        }
    }
}

impl<'a, E: Entity> EntitySetIteratorInner<'a, E> {
    #[inline]
    fn next_inner(&mut self) -> Option<EntityId<E>> {
        match self {
            Self::Empty => None,
            Self::Source(source) => source.next(),
            Self::IntersectionSources { driver, filters } => driver
                .by_ref()
                .find(|&entity_id| filters.iter().all(|filter| filter.contains(entity_id))),
            Self::Intersection { driver, filters } => {
                while let Some(entity_id) = driver.next_inner() {
                    if filters.iter().all(|filter| filter.contains(entity_id)) {
                        return Some(entity_id);
                    }
                }
                None
            }
            Self::Difference { left, right } => {
                while let Some(entity_id) = left.next_inner() {
                    if !right.contains(entity_id) {
                        return Some(entity_id);
                    }
                }
                None
            }
            Self::Union { left, right } => loop {
                if let Some(entity_id) = left.next_inner() {
                    return Some(entity_id);
                }

                match right {
                    UnionRightState::Pending(maybe_set) => {
                        if let Some(set) = maybe_set.take() {
                            *right = UnionRightState::Active(Box::new(Self::from_entity_set(set)));
                        }
                        continue;
                    }
                    UnionRightState::Active(right_iter) => {
                        while let Some(entity_id) = right_iter.next_inner() {
                            if !left.contains(entity_id) {
                                return Some(entity_id);
                            }
                        }
                        return None;
                    }
                }
            },
        }
    }

    #[inline]
    fn size_hint_inner(&self) -> (usize, Option<usize>) {
        match self {
            Self::Empty => (0, Some(0)),
            Self::Source(source) => source.size_hint(),
            Self::IntersectionSources { driver, .. } => {
                let (_, upper) = driver.size_hint();
                (0, upper)
            }
            Self::Intersection { driver, .. } => {
                let (_, upper) = driver.size_hint_inner();
                (0, upper)
            }
            Self::Difference { left, .. } => {
                let (_, upper) = left.size_hint_inner();
                (0, upper)
            }
            Self::Union { left, right } => {
                let (_, left_upper) = left.size_hint_inner();
                let right_upper = match right {
                    UnionRightState::Pending(_) => None,
                    UnionRightState::Active(right_iter) => right_iter.size_hint_inner().1,
                };
                let upper = match (left_upper, right_upper) {
                    (Some(a), Some(b)) => Some(a.saturating_add(b)),
                    _ => None,
                };
                (0, upper)
            }
        }
    }
}

/// An iterator over the IDs in an entity set, producing `EntityId<E>`s until exhausted.
pub struct EntitySetIterator<'c, E: Entity> {
    inner: EntitySetIteratorInner<'c, E>,
}

impl<'c, E: Entity> EntitySetIterator<'c, E> {
    pub(crate) fn empty() -> EntitySetIterator<'c, E> {
        EntitySetIterator {
            inner: EntitySetIteratorInner::Empty,
        }
    }

    pub(crate) fn from_population_iterator(iter: PopulationIterator<E>) -> Self {
        EntitySetIterator {
            inner: EntitySetIteratorInner::Source(SourceIterator::Population(iter)),
        }
    }

    pub(crate) fn from_sources(mut sources: Vec<SourceSet<'c, E>>) -> Self {
        if sources.is_empty() {
            return Self::empty();
        }
        if sources.len() == 1 {
            return EntitySetIterator {
                inner: EntitySetIteratorInner::Source(sources.pop().unwrap().into_iter()),
            };
        }

        // This path constructs intersections from raw source vectors, so we sort here.
        // We keep ascending order so filters checked by `all()` are smallest-first.
        sources.sort_unstable_by_key(SourceSet::sort_key);
        let mut source_iter = sources.into_iter();
        let driver = source_iter.next().unwrap().into_iter();
        EntitySetIterator {
            inner: EntitySetIteratorInner::IntersectionSources {
                driver,
                filters: source_iter.collect(),
            },
        }
    }

    pub(crate) fn from_index_set(set: Ref<'c, IndexSet<EntityId<E>>>) -> EntitySetIterator<'c, E> {
        EntitySetIterator {
            inner: EntitySetIteratorInner::Source(SourceSet::IndexSet(set).into_iter()),
        }
    }

    pub(super) fn new(set: EntitySet<'c, E>) -> Self {
        EntitySetIterator {
            inner: EntitySetIteratorInner::from_entity_set(set),
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

    /// Count query results and sample one entity uniformly from them.
    ///
    /// Returns `(count, sample)` where `sample` is `None` iff `count == 0`.
    pub fn count_and_sample_entity<R>(mut self, rng: &mut R) -> (usize, Option<EntityId<E>>)
    where
        R: Rng,
    {
        let (lower, upper) = self.size_hint();
        if Some(lower) == upper {
            if lower == 0 {
                return (0, None);
            }
            let index = rng.random_range(0..lower as u32);
            return (lower, self.nth(index as usize));
        }

        count_and_sample_single_l_reservoir(rng, self)
    }

    /// Sample up to `requested` entities uniformly from the query results. If the
    /// query's result set has fewer than `requested` entities, the entire result
    /// set is returned.
    pub fn sample_entities<R>(self, rng: &mut R, requested: usize) -> Vec<EntityId<E>>
    where
        R: Rng,
    {
        match self.size_hint() {
            (lower, Some(upper)) if lower == upper => {
                if lower == 0 {
                    warn!("Requested a sample of entities from an empty population");
                    return vec![];
                }
                sample_multiple_from_known_length(rng, self, requested)
            }
            _ => sample_multiple_l_reservoir(rng, self, requested),
        }
    }
}

impl<'a, E: Entity> Iterator for EntitySetIterator<'a, E> {
    type Item = EntityId<E>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next_inner()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint_inner()
    }

    fn count(self) -> usize {
        let EntitySetIterator { inner } = self;
        match inner {
            EntitySetIteratorInner::Source(source) => source.count(),
            EntitySetIteratorInner::IntersectionSources {
                mut driver,
                filters,
            } => driver
                .by_ref()
                .filter(|&entity_id| filters.iter().all(|filter| filter.contains(entity_id)))
                .count(),
            other => {
                let mut it = EntitySetIterator { inner: other };
                let mut n = 0usize;
                while it.next().is_some() {
                    n += 1;
                }
                n
            }
        }
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        match &mut self.inner {
            EntitySetIteratorInner::Source(source) => source.nth(n),
            EntitySetIteratorInner::IntersectionSources { driver, filters } => driver
                .by_ref()
                .filter(|&entity_id| filters.iter().all(|filter| filter.contains(entity_id)))
                .nth(n),
            _ => {
                for _ in 0..n {
                    self.next()?;
                }
                self.next()
            }
        }
    }

    fn for_each<F>(self, mut f: F)
    where
        F: FnMut(Self::Item),
    {
        let EntitySetIterator { inner } = self;
        match inner {
            EntitySetIteratorInner::Source(source) => source.for_each(f),
            other => {
                let it = EntitySetIterator { inner: other };
                for item in it {
                    f(item);
                }
            }
        }
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        let EntitySetIterator { inner } = self;
        match inner {
            EntitySetIteratorInner::Source(source) => source.fold(init, f),
            other => {
                let it = EntitySetIterator { inner: other };
                let mut acc = init;
                for item in it {
                    acc = f(acc, item);
                }
                acc
            }
        }
    }
}

impl<'c, E: Entity> std::iter::FusedIterator for EntitySetIterator<'c, E> {}

#[cfg(test)]
mod tests {
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

    use std::cell::RefCell;

    use indexmap::IndexSet;

    use crate::entity::entity_set::{EntitySet, SourceSet};
    use crate::hashing::IndexSet as FxIndexSet;
    use crate::prelude::*;
    use crate::{define_derived_property, define_property};

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
                .add_entity((ExplicitProp((i % 20) as u8), ExplicitProp2(i % 2 == 0)))
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
            .query_result_iterator((ExplicitProp(5),))
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
            .query_result_iterator((ExplicitProp(7),))
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
                .add_entity((ExplicitProp(1), ExplicitProp2(false)))
                .unwrap();
        }

        let results = context
            .query_result_iterator((ConstantProp(42), ExplicitProp2(false)))
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
                .add_entity((ExplicitProp(1), ExplicitProp2(false)))
                .unwrap();
        }

        let results = context
            .query_result_iterator((ConstantProp(42),))
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
                    .add_entity((ExplicitProp(1), ExplicitProp2(false), ConstantProp(99)))
                    .unwrap();
            } else {
                context
                    .add_entity((ExplicitProp(1), ExplicitProp2(false)))
                    .unwrap();
            }
        }

        let results = context
            .query_result_iterator((ConstantProp(99),))
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
                    .add_entity((ExplicitProp(1), ExplicitProp2(false), ConstantProp(99)))
                    .unwrap();
            } else {
                context
                    .add_entity((ExplicitProp(1), ExplicitProp2(false)))
                    .unwrap();
            }
        }

        let results = context
            .query_result_iterator((ConstantProp(99),))
            .collect::<Vec<_>>();

        assert_eq!(results.len(), 10);
    }

    // Test 7: Derived property, not indexed, initial source position = Yes
    #[test]
    fn test_derived_not_indexed_initial_source_yes() {
        let mut context = Context::new();

        for i in 0..100 {
            context
                .add_entity((ExplicitProp(i as u8), ExplicitProp2(false)))
                .unwrap();
        }

        let results = context
            .query_result_iterator((DerivedProp(true),))
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
                .add_entity((ExplicitProp(i as u8), ExplicitProp2(false)))
                .unwrap();
        }

        let results = context
            .query_result_iterator((DerivedProp(false),))
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
                .add_entity((ExplicitProp((i % 20) as u8), ExplicitProp2(i % 2 == 0)))
                .unwrap();
        }

        let results = context.query_result_iterator(()).collect::<Vec<_>>();
        for person in results {
            let explicit_prop = context.get_property::<Person, ExplicitProp>(person);
            let explicit_prop2 = context.get_property::<Person, ExplicitProp2>(person);
            println!("({:?} {:?} {:?})", person, explicit_prop, explicit_prop2);
        }

        // ExplicitProp2 has only 2 values, so it will be the smaller source
        let results = context
            .query_result_iterator((ExplicitProp(5), ExplicitProp2(false)))
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
                    .add_entity((
                        ExplicitProp(7),
                        ExplicitProp2(false),
                        ConstantProp2(200), // Non-default for smaller source
                    ))
                    .unwrap();
            } else {
                context
                    .add_entity((ExplicitProp((i % 20) as u8), ExplicitProp2(false)))
                    .unwrap();
            }
        }

        let results = context
            .query_result_iterator((ExplicitProp(7), ConstantProp2(200)))
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
                    .add_entity((ExplicitProp(99), ExplicitProp2(false)))
                    .unwrap(); // ConstantProp uses default
            } else {
                context
                    .add_entity((ExplicitProp((i % 20) as u8), ExplicitProp2(false)))
                    .unwrap();
            }
        }

        let results = context
            .query_result_iterator((ExplicitProp(99), ConstantProp(42)))
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
                    .add_entity((ConstantProp(99), ExplicitProp(0), ExplicitProp2(true)))
                    .unwrap();
            } else {
                context
                    .add_entity((ExplicitProp(0), ExplicitProp2(false)))
                    .unwrap();
            }
        }

        let results = context
            .query_result_iterator((ConstantProp(99), ExplicitProp2(true)))
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
                .add_entity((ExplicitProp(i as u8), ExplicitProp2(i < 50)))
                .unwrap();
        }

        let results = context
            .query_result_iterator((ExplicitProp2(true), DerivedProp(true)))
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
                .add_entity((ExplicitProp(i as u8), ExplicitProp2(i < 30)))
                .unwrap();
        }

        let results = context
            .query_result_iterator((ExplicitProp2(true), DerivedProp(false)))
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
                .add_entity((
                    Age(age),
                    ExplicitProp(age.wrapping_mul(7) % 100),
                    ExplicitProp2(false),
                ))
                .unwrap();
        }
        for age in 0..100 {
            context
                .add_entity((
                    Age(age),
                    ExplicitProp(age.wrapping_mul(14) % 100),
                    ExplicitProp2(false),
                ))
                .unwrap();
        }

        // Since both queries include `Age`, both will attempt to index unindexed entities. This tests that there is
        // no double borrow error.
        let results = context.query_result_iterator((Age(25),));
        let more_results = context.query_result_iterator((Age(25), ExplicitProp(75)));

        let collected_results = results.collect::<IndexSet<_>>();
        let other_collected_results = more_results.collect::<IndexSet<_>>();
        let intersection_count = collected_results
            .intersection(&other_collected_results)
            .count();
        assert_eq!(intersection_count, 1);
    }

    #[test]
    fn test_expression_intersection_iteration() {
        let set = EntitySet::from_source(SourceSet::<Person>::Population(10))
            .intersection(EntitySet::from_source(SourceSet::<Person>::Population(6)));

        let ids = set.into_iter().collect::<Vec<_>>();
        let expected = (0..6).map(EntityId::new).collect::<Vec<_>>();
        assert_eq!(ids, expected);
    }

    #[test]
    fn test_expression_difference_iteration() {
        let set = EntitySet::from_source(SourceSet::<Person>::Population(5)).difference(
            EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::new(2))),
        );

        let ids = set.into_iter().collect::<Vec<_>>();
        let expected = vec![
            EntityId::new(0),
            EntityId::new(1),
            EntityId::new(3),
            EntityId::new(4),
        ];
        assert_eq!(ids, expected);
    }

    #[test]
    fn test_expression_union_deduplicates() {
        let left = EntitySet::from_source(SourceSet::<Person>::Population(3)).difference(
            EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::new(99))),
        );
        let right = EntitySet::from_source(SourceSet::<Person>::Population(5)).difference(
            EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::new(99))),
        );
        let set = left.union(right);

        let ids = set.into_iter().collect::<Vec<_>>();
        let expected = (0..5).map(EntityId::new).collect::<Vec<_>>();
        assert_eq!(ids, expected);
    }

    #[test]
    fn test_expression_union_overlap_no_duplicates() {
        let left = EntitySet::from_source(SourceSet::<Person>::Population(5)).difference(
            EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::new(4))),
        );
        let right = EntitySet::from_source(SourceSet::<Person>::Population(7)).difference(
            EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::new(0))),
        );

        let ids = left.union(right).into_iter().collect::<IndexSet<_>>();
        let expected = (0..7).map(EntityId::new).collect::<IndexSet<_>>();
        assert_eq!(ids, expected);
    }

    #[test]
    fn test_expression_intersection_of_unions() {
        let ab = EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::new(1)))
            .union(EntitySet::from_source(SourceSet::<Person>::Entity(
                EntityId::new(2),
            )))
            .union(EntitySet::from_source(SourceSet::<Person>::Entity(
                EntityId::new(3),
            )));
        let cd = EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::new(2)))
            .union(EntitySet::from_source(SourceSet::<Person>::Entity(
                EntityId::new(3),
            )))
            .union(EntitySet::from_source(SourceSet::<Person>::Entity(
                EntityId::new(4),
            )));

        let ids = ab.intersection(cd).into_iter().collect::<Vec<_>>();
        assert_eq!(ids, vec![EntityId::new(2), EntityId::new(3)]);
    }

    #[test]
    fn test_expression_difference_not_commutative() {
        let left = EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::new(1)))
            .union(EntitySet::from_source(SourceSet::<Person>::Entity(
                EntityId::new(2),
            )))
            .union(EntitySet::from_source(SourceSet::<Person>::Entity(
                EntityId::new(3),
            )));
        let right = EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::new(2)))
            .union(EntitySet::from_source(SourceSet::<Person>::Entity(
                EntityId::new(3),
            )))
            .union(EntitySet::from_source(SourceSet::<Person>::Entity(
                EntityId::new(4),
            )));

        let left_minus_right = left.difference(right).into_iter().collect::<Vec<_>>();
        let right_minus_left =
            EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::new(2)))
                .union(EntitySet::from_source(SourceSet::<Person>::Entity(
                    EntityId::new(3),
                )))
                .union(EntitySet::from_source(SourceSet::<Person>::Entity(
                    EntityId::new(4),
                )))
                .difference(
                    EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::new(1)))
                        .union(EntitySet::from_source(SourceSet::<Person>::Entity(
                            EntityId::new(2),
                        )))
                        .union(EntitySet::from_source(SourceSet::<Person>::Entity(
                            EntityId::new(3),
                        ))),
                )
                .into_iter()
                .collect::<Vec<_>>();

        assert_eq!(left_minus_right, vec![EntityId::new(1)]);
        assert_eq!(right_minus_left, vec![EntityId::new(4)]);
    }

    #[test]
    fn test_union_size_hint_pending_right_is_unknown() {
        let left = EntitySet::from_source(SourceSet::<Person>::Population(2)).difference(
            EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::new(99))),
        );
        let right = EntitySet::from_source(SourceSet::<Person>::Population(4)).difference(
            EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::new(98))),
        );
        let iter = left.union(right).into_iter();

        assert_eq!(iter.size_hint(), (0, None));
    }

    #[test]
    fn test_nth_and_count_on_source() {
        let mut iter = EntitySet::from_source(SourceSet::<Person>::Population(5)).into_iter();
        assert_eq!(iter.nth(2), Some(EntityId::new(2)));

        let remaining = iter.count();
        assert_eq!(remaining, 2);
    }

    fn finite_set(ids: &[usize]) -> RefCell<FxIndexSet<EntityId<Person>>> {
        RefCell::new(
            ids.iter()
                .copied()
                .map(EntityId::new)
                .collect::<FxIndexSet<_>>(),
        )
    }

    fn as_entity_set(set: &RefCell<FxIndexSet<EntityId<Person>>>) -> EntitySet<Person> {
        EntitySet::from_source(SourceSet::IndexSet(set.borrow()))
    }

    #[test]
    fn prototype_empty_entity_set_yields_nothing() {
        let ids = EntitySet::<Person>::empty().into_iter().collect::<Vec<_>>();
        assert!(ids.is_empty());
    }

    #[test]
    fn prototype_iter_union_disjoint() {
        let a = finite_set(&[1, 2]);
        let b = finite_set(&[3, 4]);
        let ids = as_entity_set(&a)
            .union(as_entity_set(&b))
            .into_iter()
            .collect::<IndexSet<_>>();
        let expected = [1usize, 2, 3, 4]
            .into_iter()
            .map(EntityId::new)
            .collect::<IndexSet<_>>();
        assert_eq!(ids, expected);
    }

    #[test]
    fn prototype_iter_union_overlapping() {
        let a = finite_set(&[1, 2, 3]);
        let b = finite_set(&[2, 3, 4]);
        let ids = as_entity_set(&a)
            .union(as_entity_set(&b))
            .into_iter()
            .collect::<Vec<_>>();
        let unique = ids.iter().copied().collect::<IndexSet<_>>();
        assert_eq!(ids.len(), unique.len());
        assert!(unique.contains(&EntityId::new(1)));
        assert!(unique.contains(&EntityId::new(4)));
    }

    #[test]
    fn prototype_iter_intersection_disjoint() {
        let a = finite_set(&[1, 2]);
        let b = finite_set(&[3, 4]);
        let ids = as_entity_set(&a)
            .intersection(as_entity_set(&b))
            .into_iter()
            .collect::<Vec<_>>();
        assert!(ids.is_empty());
    }

    #[test]
    fn prototype_iter_difference_basic() {
        let a = finite_set(&[1, 2, 3, 4]);
        let b = finite_set(&[3, 4, 5, 6]);
        let ids = as_entity_set(&a)
            .difference(as_entity_set(&b))
            .into_iter()
            .collect::<IndexSet<_>>();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&EntityId::new(1)));
        assert!(ids.contains(&EntityId::new(2)));
    }

    #[test]
    fn prototype_iter_matches_contains_compound() {
        let a = finite_set(&[1, 2, 3, 4]);
        let b = finite_set(&[3, 4, 5]);
        let c = finite_set(&[7, 8, 9, 10]);
        let d = finite_set(&[9, 10, 11]);
        let left = as_entity_set(&a).intersection(as_entity_set(&b));
        let right = as_entity_set(&c).difference(as_entity_set(&d));
        let iterated = left.union(right).into_iter().collect::<IndexSet<_>>();

        let a2 = finite_set(&[1, 2, 3, 4]);
        let b2 = finite_set(&[3, 4, 5]);
        let c2 = finite_set(&[7, 8, 9, 10]);
        let d2 = finite_set(&[9, 10, 11]);
        let check = as_entity_set(&a2)
            .intersection(as_entity_set(&b2))
            .union(as_entity_set(&c2).difference(as_entity_set(&d2)));

        for value in 0..15 {
            let entity = EntityId::new(value);
            assert_eq!(iterated.contains(&entity), check.contains(entity));
        }
    }

    #[test]
    fn prototype_size_hint_single_source_and_partial_consume() {
        let mut iter = EntitySet::from_source(SourceSet::<Person>::Population(5)).into_iter();
        assert_eq!(iter.size_hint(), (5, Some(5)));
        iter.next();
        assert_eq!(iter.size_hint(), (4, Some(4)));
    }
}
