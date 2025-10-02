//! A `SourceSet<'c>` is a wrapper type that holds either an index set (`Ref<'c, HashSet<PersonId>>`)
//! or a property set (`Ref<'c, Vec<Option<P::Value>>>`).
//!
//! A `SourceSet` abstractly represents the set of `PersonId`s for which a particular
//! `PersonProperty` has a particular value. A `SourceSet` can be converted into a
//! `SourceIterator<'c>`, an iterator over the set of `PersonId`s it represents. The
//! lifetime `'c` is the lifetime of the (immutable) borrow of the underlying `Context`.
//!
//! The `SourceSet<'c>` and `SourceIterator<'c>` types are used by `QueryResultIterator<'c>`, which
//! iterates over the intersection of a set of `SourceSet`s. Internally, `QueryResultIterator` holds
//! its state in a set of `SourceSet` instances and a `SourceIterator`, which is an iterator created
//! from a `SourceSet`. A `SourceSet` wraps either an index set (an immutable reference to a set
//! from an index) or a property vector (the `Vec<Option<PersonProperty::Value>>` that internally
//! stores the property values) and can compute membership very efficiently. The algorithm chooses
//! the _smallest_ `SourceSet` to create its `SourceIterator` and, when `QueryResultIterator::next()`
//! is called, this `SourceIterator` is iterated over until an ID is found that is contained
//! in all other `SourceSet`s, in which case the ID is returned, or until it is exhausted.

use crate::people::data::PeopleIterator;
use crate::{PersonId, PersonProperty};
use ouroboros::self_referencing;
use rustc_hash::FxHashSet as HashSet;
use std::cell::Ref;
use std::collections::hash_set::Iter as HashSetIter;

type BxPropertyVec<'a> = Box<dyn AbstractPropertyVec<'a> + 'a>;

/// Type erased property vec representing the (abstract) set of `PersonId`s
/// for which a particular property has a particular value.
pub trait AbstractPropertyVec<'a> {
    fn len(&self) -> usize;

    /// A test that `person_id` is contained in the (abstractly
    /// defined) set. This operation is very efficient.
    fn contains(&self, person_id: PersonId) -> bool;

    /// This is purely a type cast from `Box<dyn AbstractPropertyVec>` to
    /// `Box<dyn Iterator<Item = PersonId>>`. Notice the type of `self`.
    fn to_iter(self: Box<Self>) -> Box<dyn Iterator<Item = PersonId> + 'a>;
}

/// Typed property vec. This does double duty as a concrete property vec and as an
/// iterator. Instances of this struct represent the (abstract) set of `PeopleId`s for
/// which the property `P: PersonProperty` has the value `ConcretePropertyVec::value`.
pub(super) struct ConcretePropertyVec<'a, P: PersonProperty> {
    /// A `Ref` to the underlying property vector backing property `P`.
    values: Ref<'a, Vec<Option<P::Value>>>,
    /// The value that `PersonId`s in this (abstract) set must have for `P`.
    value: P::Value,
    /// See notes on the `Iterator` impl for this struct below.
    next_index: usize,
}

impl<'a, P: PersonProperty> ConcretePropertyVec<'a, P> {
    pub fn new(values: Ref<'a, Vec<Option<P::Value>>>, value: P::Value) -> Self {
        ConcretePropertyVec {
            values,
            value,
            next_index: 0,
        }
    }
}

impl<'a, P: PersonProperty> AbstractPropertyVec<'a> for ConcretePropertyVec<'a, P> {
    fn len(&self) -> usize {
        self.values.len()
    }

    fn contains(&self, person_id: PersonId) -> bool {
        if let Some(found_value) = self.values.get(person_id.0) {
            return *found_value == Some(self.value);
        }

        false
    }

    fn to_iter(self: Box<Self>) -> Box<dyn Iterator<Item = PersonId> + 'a> {
        self
    }
}

// This iterator implementation is identical to:
// ```rust
// self.values.iter().enumerate().filter_map(|(i, v)| {
//             if *v == Some(self.value) {
//                 Some(PersonId(i))
//             } else {
//                 None
//             }
//         })
// ```
// The type of the iterator above is not representable, so
// it turns out to be a lot easier to implement it ourselves.
impl<'a, P: PersonProperty> Iterator for ConcretePropertyVec<'a, P> {
    type Item = PersonId;

    fn next(&mut self) -> Option<Self::Item> {
        // Scan the property vector until we find a value that matches the query value,
        // or until we reach the end of the vector.
        while let Some(found_value) = self.values.get(self.next_index) {
            self.next_index += 1;
            if *found_value == Some(self.value) {
                return Some(PersonId(self.next_index - 1));
            }
        }
        None
    }
}

/// The self-referential iterator type for index sets. We don't implement
/// `Iterator` for this struct, choosing instead to access the inner
/// iterator in the `Iterator` implementation on `SourceIterator`.
#[self_referencing]
pub(super) struct IndexSetIterator<'a> {
    index_set: Ref<'a, HashSet<PersonId>>,
    #[borrows(index_set)]
    #[covariant]
    iter: HashSetIter<'this, PersonId>,
}

impl<'a> IndexSetIterator<'a> {
    pub fn from_index_set(index_set: Ref<'a, HashSet<PersonId>>) -> Self {
        IndexSetIteratorBuilder {
            index_set,
            iter_builder: |index_set| index_set.iter(),
        }
        .build()
    }
}

pub enum SourceSet<'a> {
    IndexSet(Ref<'a, HashSet<PersonId>>),
    PropertyVec(BxPropertyVec<'a>),
}

impl<'a> SourceSet<'a> {
    pub(super) fn len(&self) -> usize {
        match self {
            SourceSet::IndexSet(source) => source.len(),
            SourceSet::PropertyVec(source) => source.len(),
        }
    }

    pub(super) fn contains(&self, id: PersonId) -> bool {
        match self {
            SourceSet::IndexSet(source) => source.contains(&id),
            SourceSet::PropertyVec(source) => source.contains(id),
        }
    }

    pub(super) fn into_iter(self) -> SourceIterator<'a> {
        match self {
            SourceSet::IndexSet(ids) => {
                SourceIterator::IndexIter(IndexSetIterator::from_index_set(ids))
            }
            SourceSet::PropertyVec(property_vec) => {
                SourceIterator::PropertyVecIter(property_vec.to_iter())
            }
        }
    }
}

/// Kinds of iterators that are used as a basis for `QueryResultIterator`
pub(crate) enum SourceIterator<'a> {
    /// An iterator over an index set
    IndexIter(IndexSetIterator<'a>),
    /// An iterator over a property vector
    PropertyVecIter(Box<dyn Iterator<Item = PersonId> + 'a>),
    /// An iterator over the entire population
    WholePopulation(PeopleIterator),
    /// An empty iterator
    Empty,
}

impl<'a> Iterator for SourceIterator<'a> {
    type Item = PersonId;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            SourceIterator::IndexIter(index_set_iter) => {
                index_set_iter.with_iter_mut(|iter| iter.next().copied())
            }
            SourceIterator::PropertyVecIter(iter) => iter.next(),
            SourceIterator::WholePopulation(iter) => iter.next(),
            SourceIterator::Empty => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            SourceIterator::IndexIter(iter) => iter.with_iter(|iter| iter.size_hint()),
            SourceIterator::PropertyVecIter(_) => (0, None),
            SourceIterator::WholePopulation(iter) => iter.size_hint(),
            SourceIterator::Empty => (0, Some(0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::define_person_property_with_default;
    use std::cell::RefCell;

    define_person_property_with_default!(Age, u8, 0);

    #[test]
    fn test_iterators() {
        let values = RefCell::new(vec![0u8, 3, 2, 3, 4, 5, 3].into_iter().map(Some).collect());
        let values = values.borrow();
        let people_set =
            HashSet::from_iter([PersonId(0), PersonId(2), PersonId(3), PersonId(6)].into_iter());
        let people_set = RefCell::new(people_set);
        let people_set_ref = people_set.borrow();
        {
            let pvi =
                SourceSet::PropertyVec(Box::new(ConcretePropertyVec::<Age>::new(values, 3u8)));
            let isi = SourceSet::IndexSet(people_set_ref);
            let sources = vec![pvi, isi];

            for source in sources {
                for id in source.into_iter() {
                    print!("{}, ", id);
                }
                println!();
            }
        }
    }
}
