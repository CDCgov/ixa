//! `SourceIterator` is the concrete iteration engine used by
//! `EntitySetIterator`.
//!
//! It encapsulates the concrete source chosen for traversal: an index-set
//! iterator, a property-source iterator, a whole-population iterator, or a
//! contiguous population-range iterator.
//!
//! `SourceSet` (defined in `source_set.rs`) builds `SourceIterator` values,
//! and `EntitySetIterator` drives them while applying remaining set-membership
//! filters.
//!
//! Depending on source kind, the underlying iterator may be implemented either:
//! - directly in this module (for `IndexSetIterator`, `PopulationIterator`, and
//!   `EntityIdRangeIterator`), or
//! - on intermediate wrapper types defined in `source_set.rs`
//!   (`ConcretePropertySource` and `DerivedPropertySource`), which serve as both
//!   set-facing wrappers and iterators.

use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;
use std::ops::Range;

use ouroboros::self_referencing;

use super::source_set::AbstractPropertySource;
use crate::entity::{Entity, EntityId, PopulationIterator};
use crate::hashing::{IndexSet, IndexSetIter};

/// The self-referential iterator type for index sets. We don't implement
/// `Iterator` for this struct, choosing instead to access the inner
/// iterator in the `Iterator` implementation on `SourceIterator`.
#[self_referencing]
pub(super) struct IndexSetIterator<'a, E: Entity> {
    index_set: &'a IndexSet<EntityId<E>>,
    #[borrows(index_set)]
    #[covariant]
    iter: IndexSetIter<'this, EntityId<E>>,
}

impl<'a, E: Entity> IndexSetIterator<'a, E> {
    pub fn from_index_set(index_set: &'a IndexSet<EntityId<E>>) -> Self {
        IndexSetIteratorBuilder {
            index_set,
            iter_builder: |index_set| index_set.iter(),
        }
        .build()
    }
}

/// Internal iterator for arbitrary contiguous ranges of entity IDs.
///
/// This is intentionally separate from `PopulationIterator`, which retains the
/// narrower meaning of iterating over the entire population snapshot
/// `0..population`.
#[derive(Clone)]
pub(super) struct EntityIdRangeIterator<E: Entity> {
    source: Range<usize>,
    next_index: usize,
    _phantom: PhantomData<E>,
}

impl<E: Entity> EntityIdRangeIterator<E> {
    pub(super) fn new(source: Range<usize>) -> Self {
        Self {
            next_index: source.start,
            source,
            _phantom: PhantomData,
        }
    }

    fn source(&self) -> &Range<usize> {
        &self.source
    }

    fn remaining_range(&self) -> Range<usize> {
        self.next_index..self.source.end
    }
}

impl<E: Entity> Iterator for EntityIdRangeIterator<E> {
    type Item = EntityId<E>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut range = self.remaining_range();
        let next = range.next();
        self.next_index = range.start;
        next.map(EntityId::new)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.remaining_range().size_hint()
    }

    fn count(self) -> usize {
        self.remaining_range().count()
    }

    fn last(self) -> Option<Self::Item> {
        self.remaining_range().last().map(EntityId::new)
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let mut range = self.remaining_range();
        let nth = range.nth(n);
        self.next_index = range.start;
        nth.map(EntityId::new)
    }

    fn for_each<F>(self, mut f: F)
    where
        F: FnMut(Self::Item),
    {
        self.remaining_range()
            .for_each(|index| f(EntityId::new(index)));
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.remaining_range()
            .fold(init, |acc, index| f(acc, EntityId::new(index)))
    }
}

/// Kinds of iterators that are used as a basis for `EntitySetIterator`
pub(super) enum SourceIterator<'a, E: Entity> {
    /// An iterator over an index set
    IndexIter(IndexSetIterator<'a, E>),
    /// An iterator over a property vector
    PropertyVecIter(Box<dyn AbstractPropertySource<'a, E> + 'a>),
    /// An iterator over the entire population
    Population(PopulationIterator<E>),
    /// An iterator over a contiguous range of entity IDs
    PopulationRange(EntityIdRangeIterator<E>),
}

impl<'a, E: Entity> Debug for SourceIterator<'a, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceIterator::IndexIter(_iter) => write!(f, "IndexIter"),
            SourceIterator::PropertyVecIter(_iter) => write!(f, "PropertyVecIter"),
            SourceIterator::Population(_) => write!(f, "Population"),
            SourceIterator::PopulationRange(_) => write!(f, "PopulationRange"),
        }
    }
}

impl<'a, E: Entity> SourceIterator<'a, E> {
    /// Test whether `id` is a member of the original source set this iterator was built from.
    #[must_use]
    #[inline]
    pub(super) fn contains(&self, id: EntityId<E>) -> bool {
        match self {
            SourceIterator::IndexIter(iter) => {
                iter.with_index_set(|index_set| index_set.contains(&id))
            }
            SourceIterator::PropertyVecIter(source) => source.contains(id),
            SourceIterator::Population(source) => id.0 < source.population(),
            SourceIterator::PopulationRange(source) => source.source().contains(&id.0),
        }
    }
}

impl<'a, E: Entity> Iterator for SourceIterator<'a, E> {
    type Item = EntityId<E>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            SourceIterator::IndexIter(index_set_iter) => {
                index_set_iter.with_iter_mut(|iter| iter.next().copied())
            }
            SourceIterator::PropertyVecIter(iter) => iter.next(),
            SourceIterator::Population(iter) => iter.next(),
            SourceIterator::PopulationRange(iter) => iter.next(),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            SourceIterator::IndexIter(iter) => iter.with_iter(|iter| iter.size_hint()),
            SourceIterator::PropertyVecIter(iter) => iter.size_hint(),
            SourceIterator::Population(iter) => iter.size_hint(),
            SourceIterator::PopulationRange(iter) => iter.size_hint(),
        }
    }

    fn count(self) -> usize {
        // Some of these iterators have very efficient `count` implementations, and we want to exploit
        // them when they exist.
        match self {
            SourceIterator::IndexIter(mut iter) => iter.with_iter_mut(|iter| iter.count()),
            SourceIterator::PropertyVecIter(iter) => iter.count(),
            SourceIterator::Population(iter) => iter.count(),
            SourceIterator::PopulationRange(iter) => iter.count(),
        }
    }

    fn last(self) -> Option<Self::Item> {
        match self {
            Self::IndexIter(mut iter) => iter.with_iter_mut(|iter| iter.last().copied()),
            Self::PropertyVecIter(iter) => iter.last(),
            Self::Population(iter) => iter.last(),
            Self::PopulationRange(iter) => iter.last(),
        }
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        match self {
            Self::IndexIter(iter) => iter.with_iter_mut(|iter| iter.nth(n).copied()),
            Self::PropertyVecIter(iter) => iter.nth(n),
            Self::Population(iter) => iter.nth(n),
            Self::PopulationRange(iter) => iter.nth(n),
        }
    }

    fn for_each<F>(self, mut f: F)
    where
        F: FnMut(Self::Item),
    {
        match self {
            Self::IndexIter(mut iter) => {
                iter.with_iter_mut(|iter| iter.for_each(|entity_id| f(*entity_id)))
            }
            Self::PropertyVecIter(iter) => iter.for_each(f),
            Self::Population(iter) => iter.for_each(f),
            Self::PopulationRange(iter) => iter.for_each(f),
        }
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        match self {
            Self::IndexIter(mut iter) => {
                iter.with_iter_mut(|iter| iter.fold(init, |acc, entity_id| f(acc, *entity_id)))
            }
            Self::PropertyVecIter(iter) => iter.fold(init, f),
            Self::Population(iter) => iter.fold(init, f),
            Self::PopulationRange(iter) => iter.fold(init, f),
        }
    }
}

impl<'c, E: Entity> std::iter::FusedIterator for SourceIterator<'c, E> {}

#[cfg(test)]
mod tests {
    use super::super::source_set::{ConcretePropertySource, SourceSet};
    use crate::entity::property_value_store_core::RawPropertyValueVec;
    use crate::entity::EntityId;
    use crate::hashing::IndexSet;
    use crate::{define_entity, define_property};

    define_entity!(Person);
    define_property!(struct Age(u8), Person, default_const = Age(0));

    #[test]
    fn source_iterator_contains_for_index_source_uses_original_set() {
        let values: RawPropertyValueVec<Age> =
            [0u8, 3, 2, 3, 4, 5, 3].into_iter().map(Age).collect();
        let people_set: IndexSet<EntityId<Person>> = IndexSet::from_iter([
            EntityId::new(0),
            EntityId::new(2),
            EntityId::new(3),
            EntityId::new(6),
        ]);
        let people_set_ref = &people_set;

        let mut iter = SourceSet::IndexSet(people_set_ref).into_iter();
        assert_eq!(iter.next(), Some(EntityId::new(0)));

        assert!(iter.contains(EntityId::new(0)));
        assert!(iter.contains(EntityId::new(6)));
        assert!(!iter.contains(EntityId::new(5)));

        let mut property_iter = SourceSet::PropertySet(Box::new(ConcretePropertySource::<
            Person,
            Age,
        >::new(
            &values, Age(3u8), 8
        )))
        .into_iter();
        assert_eq!(property_iter.next(), Some(EntityId::new(1)));
        assert!(property_iter.contains(EntityId::new(1)));
        assert!(property_iter.contains(EntityId::new(3)));
        assert!(!property_iter.contains(EntityId::new(4)));
    }

    #[test]
    fn source_iterator_contains_for_population_ranges() {
        let mut population_iter = SourceSet::<Person>::full_population(5).into_iter();
        assert_eq!(population_iter.next(), Some(EntityId::new(0)));
        assert_eq!(population_iter.next(), Some(EntityId::new(1)));

        assert!(population_iter.contains(EntityId::new(0)));
        assert!(population_iter.contains(EntityId::new(4)));
        assert!(!population_iter.contains(EntityId::new(5)));

        let empty_iter = SourceSet::<Person>::empty_range().into_iter();
        assert!(!empty_iter.contains(EntityId::new(0)));
    }

    #[test]
    fn source_iterator_contains_for_singleton_range_uses_original_set() {
        let mut iter = SourceSet::<Person>::singleton(EntityId::new(11)).into_iter();
        assert_eq!(iter.next(), Some(EntityId::new(11)));

        assert!(iter.contains(EntityId::new(11)));
        assert!(!iter.contains(EntityId::new(10)));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn source_iterator_contains_for_offset_ranges_uses_original_set() {
        let mut iter = SourceSet::<Person>::population_range(2..7).into_iter();
        assert_eq!(iter.next(), Some(EntityId::new(2)));
        assert_eq!(iter.next(), Some(EntityId::new(3)));

        assert!(iter.contains(EntityId::new(2)));
        assert!(iter.contains(EntityId::new(6)));
        assert!(!iter.contains(EntityId::new(7)));
    }
}
