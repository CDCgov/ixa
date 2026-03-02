//! `SourceIterator` is the concrete iteration engine used by
//! `EntitySetIterator`.
//!
//! It encapsulates the concrete source chosen for traversal: an index-set
//! iterator, a property-source iterator, a whole-population iterator, or an
//! empty iterator.
//!
//! `SourceSet` (defined in `source_set.rs`) builds `SourceIterator` values,
//! and `EntitySetIterator` drives them while applying remaining set-membership
//! filters.
//!
//! Depending on source kind, the underlying iterator may be implemented either:
//! - directly in this module (for `IndexSetIterator` and `PopulationIterator`), or
//! - on intermediate wrapper types defined in `source_set.rs`
//!   (`ConcretePropertySource` and `DerivedPropertySource`), which serve as both
//!   set-facing wrappers and iterators.

use std::cell::Ref;
use std::fmt::{Debug, Formatter};

use ouroboros::self_referencing;

use super::source_set::AbstractPropertySource;
use crate::entity::{Entity, EntityId, PopulationIterator};
use crate::hashing::{IndexSet, IndexSetIter};

/// The self-referential iterator type for index sets. We don't implement
/// `Iterator` for this struct, choosing instead to access the inner
/// iterator in the `Iterator` implementation on `SourceIterator`.
#[self_referencing]
pub(super) struct IndexSetIterator<'a, E: Entity> {
    index_set: Ref<'a, IndexSet<EntityId<E>>>,
    #[borrows(index_set)]
    #[covariant]
    iter: IndexSetIter<'this, EntityId<E>>,
}

impl<'a, E: Entity> IndexSetIterator<'a, E> {
    pub fn from_index_set(index_set: Ref<'a, IndexSet<EntityId<E>>>) -> Self {
        IndexSetIteratorBuilder {
            index_set,
            iter_builder: |index_set| index_set.iter(),
        }
        .build()
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
    /// A singleton iterator
    Entity { id: EntityId<E>, exhausted: bool },
    /// An empty iterator
    Empty,
}

impl<'a, E: Entity> Debug for SourceIterator<'a, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceIterator::IndexIter(_iter) => write!(f, "IndexIter"),
            SourceIterator::PropertyVecIter(_iter) => write!(f, "PropertyVecIter"),
            SourceIterator::Population { .. } => write!(f, "WholePopulation"),
            SourceIterator::Entity { .. } => write!(f, "Entity"),
            SourceIterator::Empty => write!(f, "Empty"),
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
            SourceIterator::Entity { id: entity_id, .. } => *entity_id == id,
            SourceIterator::Empty => false,
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
            SourceIterator::Entity { id, exhausted } => {
                if *exhausted {
                    None
                } else {
                    *exhausted = true;
                    Some(*id)
                }
            }
            SourceIterator::Empty => None,
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            SourceIterator::IndexIter(iter) => iter.with_iter(|iter| iter.size_hint()),
            SourceIterator::PropertyVecIter(iter) => iter.size_hint(),
            SourceIterator::Population(iter) => iter.size_hint(),
            SourceIterator::Entity { exhausted, .. } => {
                if *exhausted {
                    (0, Some(0))
                } else {
                    (1, Some(1))
                }
            }
            SourceIterator::Empty => (0, Some(0)),
        }
    }

    fn count(self) -> usize {
        // Some of these iterators have very efficient `count` implementations, and we want to exploit
        // them when they exist.
        match self {
            SourceIterator::IndexIter(mut iter) => iter.with_iter_mut(|iter| iter.count()),
            SourceIterator::PropertyVecIter(iter) => iter.count(),
            SourceIterator::Population(iter) => iter.count(),
            SourceIterator::Entity { exhausted, .. } => {
                if exhausted {
                    0
                } else {
                    1
                }
            }
            SourceIterator::Empty => 0,
        }
    }

    fn last(self) -> Option<Self::Item> {
        match self {
            Self::IndexIter(mut iter) => iter.with_iter_mut(|iter| iter.last().copied()),
            Self::PropertyVecIter(iter) => iter.last(),
            Self::Population(iter) => iter.last(),
            Self::Entity { id, exhausted } => {
                if exhausted {
                    None
                } else {
                    Some(id)
                }
            }
            Self::Empty => None,
        }
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        match self {
            Self::IndexIter(iter) => iter.with_iter_mut(|iter| iter.nth(n).copied()),
            Self::PropertyVecIter(iter) => iter.nth(n),
            Self::Population(iter) => iter.nth(n),
            Self::Entity { id, exhausted } => {
                if n == 0 && !*exhausted {
                    *exhausted = true;
                    Some(*id)
                } else {
                    *exhausted = true;
                    None
                }
            }
            Self::Empty => None,
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
            Self::Entity { id, exhausted } => {
                if !exhausted {
                    f(id);
                }
            }
            Self::Empty => {}
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
            Self::Entity { id, exhausted } => {
                if exhausted {
                    init
                } else {
                    f(init, id)
                }
            }
            Self::Empty => init,
        }
    }
}

impl<'c, E: Entity> std::iter::FusedIterator for SourceIterator<'c, E> {}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

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
        let people_set = RefCell::new(people_set);
        let people_set_ref = people_set.borrow();

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
    fn source_iterator_contains_for_population_and_empty() {
        let mut population_iter = SourceSet::<Person>::Population(5).into_iter();
        assert_eq!(population_iter.next(), Some(EntityId::new(0)));
        assert_eq!(population_iter.next(), Some(EntityId::new(1)));

        assert!(population_iter.contains(EntityId::new(0)));
        assert!(population_iter.contains(EntityId::new(4)));
        assert!(!population_iter.contains(EntityId::new(5)));

        let empty_iter = SourceSet::<Person>::Empty.into_iter();
        assert!(!empty_iter.contains(EntityId::new(0)));
    }

    #[test]
    fn source_iterator_contains_for_entity_uses_original_set() {
        let mut iter = SourceSet::<Person>::Entity(EntityId::new(11)).into_iter();
        assert_eq!(iter.next(), Some(EntityId::new(11)));

        assert!(iter.contains(EntityId::new(11)));
        assert!(!iter.contains(EntityId::new(10)));
        assert_eq!(iter.next(), None);
    }
}
