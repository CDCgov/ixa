//! A `SourceSet` abstractly represents the set of `EntityId<E>`s for which a particular
//! `Property` has a particular value.
//!
//! A `SourceSet` can be converted into a `SourceIterator<'c>`, an
//! iterator over the set of `EntityId<E>`s it represents. The lifetime `'c`
//! is the lifetime of the (immutable) borrow of the underlying `Context`.
//!
//! The `SourceSet<'c>` and `SourceIterator<'c>` types are used by `QueryResultIterator<'c>`, which
//! iterates over the intersection of a set of `SourceSet`s. Internally, `QueryResultIterator` holds
//! its state in a set of `SourceSet` instances and a `SourceIterator`, which is an iterator created
//! from a `SourceSet`. A `SourceSet` wraps either an index set (an immutable reference to a set
//! from an index) or a property vector (the `Vec<Option<Property<E>::Value>>` that internally
//! stores the property values) and can compute membership very efficiently. The algorithm chooses
//! the _smallest_ `SourceSet` to create its `SourceIterator` and, when `QueryResultIterator::next()`
//! is called, this `SourceIterator` is iterated over until an ID is found that is contained
//! in all other `SourceSet`s, in which case the ID is returned, or until it is exhausted.

use std::cell::Ref;
use std::collections::hash_set::Iter as HashSetIter;
use std::marker::PhantomData;

use ouroboros::self_referencing;

use crate::entity::property_value_store_core::RawPropertyValueVec;
use crate::entity::{ContextEntitiesExt, Entity, EntityId, EntityIterator};
use crate::hashing::HashSet;
use crate::prelude::Property;
use crate::Context;

type BxPropertySource<'a, E> = Box<dyn AbstractPropertySource<'a, E> + 'a>;

/// Type erased property source representing the (abstract) set of `EntityId<E>`s
/// for which a particular property has a particular value. This is used for
/// both `ConcretePropertyVec<'a, P: Property>` and `DerivedPropertySource<'a, P: Property>`.
pub trait AbstractPropertySource<'a, E: Entity> {
    /// An upper bound on the number of elements this source will need to iterate over. The idea is to perform the
    /// minimum amount of work to determine the first set in the intersection of sets that `QueryResultIterator`
    /// represents.
    fn upper_len(&self) -> usize;

    /// A test that `entity_id` is contained in the (abstractly
    /// defined) set. This operation is very efficient.
    fn contains(&self, entity_id: EntityId<E>) -> bool;

    /// This is purely a type cast from `Box<dyn AbstractPropertyVec>` to
    /// `Box<dyn Iterator<Item = EntityId<E>>>`. Notice the type of `self`.
    fn to_iter(self: Box<Self>) -> Box<dyn Iterator<Item = EntityId<E>> + 'a>;
}

/// To iterate over the values of an unindexed derived property,
/// we need to iterate over the entire population and filter.
pub(super) struct DerivedPropertySource<'a, E: Entity, P: Property<E>> {
    /// A reference to the context so we can compute derived values
    context: &'a Context,

    /// The value that `EntityId<E>`s in this (abstract) set must have for `P`.
    value: P,

    /// See notes on the `Iterator` impl for this struct below.
    next_index: usize,

    /// We need to know the population size to know when the iterator is exhausted.
    population_size: usize,

    _phantom: PhantomData<E>,
}

impl<'a, E: Entity, P: Property<E>> DerivedPropertySource<'a, E, P> {
    pub fn new(context: &'a Context, value: P) -> Self {
        let population_size = context.get_entity_count::<E>();

        DerivedPropertySource {
            context,
            value,
            next_index: 0,
            population_size,
            _phantom: PhantomData,
        }
    }
}

impl<'a, E: Entity, P: Property<E>> AbstractPropertySource<'a, E>
    for DerivedPropertySource<'a, E, P>
{
    fn upper_len(&self) -> usize {
        self.context.get_entity_count::<E>()
    }

    fn contains(&self, entity_id: EntityId<E>) -> bool {
        P::compute_derived(self.context, entity_id) == self.value
    }

    fn to_iter(self: Box<Self>) -> Box<dyn Iterator<Item = EntityId<E>> + 'a> {
        self
    }
}

impl<'a, E: Entity, P: Property<E>> Iterator for DerivedPropertySource<'a, E, P> {
    type Item = EntityId<E>;

    fn next(&mut self) -> Option<Self::Item> {
        // Scan the population until we find a value that matches the query value,
        // or until we exhaust the population.
        while self.next_index < self.population_size {
            let entity_id = EntityId::<E>::new(self.next_index);
            self.next_index += 1;
            if P::compute_derived(self.context, entity_id) == self.value {
                return Some(entity_id);
            }
        }

        None
    }
}

/// Typed property vec. This does double duty as a concrete property vec and as an
/// iterator. Instances of this struct represent the (abstract) set of `EntityId`s for
/// which the property `P: Property<E>` has the value `ConcretePropertyVec::value`.
pub(super) struct ConcretePropertySource<'a, E: Entity, P: Property<E>> {
    /// A `Ref` to the underlying property vector backing property `P`.
    values: &'a RawPropertyValueVec<P>,

    /// The value that `EntityId<E>`s in this (abstract) set must have for `P`.
    value: P,

    /// See notes on the `Iterator` impl for this struct below.
    next_index: usize,

    /// A minor optimization that allows us to avoid initializing the property vector if it's
    /// default value is a constant. This is `true` if `self.value` is the constant default value,
    /// `false` otherwise. When this is true, unset values are implicitly equal to `self.value`.
    is_default_value: bool,

    /// In the constant initializer case, we need to know the
    /// population size to know when the iterator is exhausted.
    population_size: usize,

    _phantom: PhantomData<E>,
}

impl<'a, E: Entity, P: Property<E>> ConcretePropertySource<'a, E, P> {
    /// Takes a `Ref` to the values vector, the `value` we are searching
    /// for, and whether unset values should be considered equal to `value`.
    pub fn new(values: &'a RawPropertyValueVec<P>, value: P, population_size: usize) -> Self {
        let is_default_value = !P::is_required() && P::default_const() == value;
        ConcretePropertySource {
            values,
            value,
            next_index: 0,
            is_default_value,
            population_size,
            _phantom: PhantomData,
        }
    }
}

impl<'a, E: Entity, P: Property<E>> AbstractPropertySource<'a, E>
    for ConcretePropertySource<'a, E, P>
{
    fn upper_len(&self) -> usize {
        if !self.is_default_value {
            self.values.len()
        } else {
            // If the property is default value, we can't use the length of the property vector, because
            // unset values are implicitly equal to the default value. Instead, we use the population size.
            self.population_size
        }
    }

    fn contains(&self, person_id: EntityId<E>) -> bool {
        // Recall that the "Option" indicates whether `person_id.0` is in bounds.
        if let Some(found_value) = self.values.get(person_id.0) {
            found_value == self.value
        } else {
            // Unset values are implicitly equal to the default value.
            self.is_default_value
        }
    }

    fn to_iter(self: Box<Self>) -> Box<dyn Iterator<Item = EntityId<E>> + 'a> {
        self
    }
}

impl<'a, E: Entity, P: Property<E>> Iterator for ConcretePropertySource<'a, E, P> {
    type Item = EntityId<E>;

    fn next(&mut self) -> Option<Self::Item> {
        // Scan the property vector until we find a value that matches the query value,
        // or until we exhause the population
        while self.next_index < self.population_size {
            self.next_index += 1;
            if let Some(found_value) = self.values.get(self.next_index - 1) {
                // The vector is not exhausted...
                if found_value == self.value {
                    return Some(EntityId::new(self.next_index - 1));
                }
            } else {
                // The vector is exhausted, but the population is not.
                if self.is_default_value {
                    // Unset values are implicitly equal to the default value.
                    return Some(EntityId::new(self.next_index - 1));
                } else {
                    // We know none of the remaining population will match, so we skip to the end and return `None`.
                    self.next_index = self.population_size;
                }
            }
        }

        // The population is exhausted.
        None
    }
}

/// The self-referential iterator type for index sets. We don't implement
/// `Iterator` for this struct, choosing instead to access the inner
/// iterator in the `Iterator` implementation on `SourceIterator`.
#[self_referencing]
pub(super) struct IndexSetIterator<'a, E: Entity> {
    index_set: Ref<'a, HashSet<EntityId<E>>>,
    #[borrows(index_set)]
    #[covariant]
    iter: HashSetIter<'this, EntityId<E>>,
}

impl<'a, E: Entity> IndexSetIterator<'a, E> {
    pub fn from_index_set(index_set: Ref<'a, HashSet<EntityId<E>>>) -> Self {
        IndexSetIteratorBuilder {
            index_set,
            iter_builder: |index_set| index_set.iter(),
        }
        .build()
    }
}

/// Represents the set of `EntityId<E>`s for which a particular `Property` has a particular value.
pub enum SourceSet<'a, E: Entity> {
    IndexSet(Ref<'a, HashSet<EntityId<E>>>),
    PropertySet(BxPropertySource<'a, E>),
}

impl<'a, E: Entity> SourceSet<'a, E> {
    /// A constructor for `SourceSet`s during construction of `QueryResultIterator` in
    /// `Query<E>::new_query_result_iterator()`. Returns `None` if the set is empty.
    ///
    /// We first look for an index set. If not found, we check if the property is derived.
    /// For derived properties, we wrap a reference to the `Context`. For nonderived
    /// properties, we wrap a reference to the property's backing vector.
    ///
    /// This method refreshes outdated indexes.
    pub(super) fn new<P: Property<E>>(value: P, context: &'a Context) -> Option<Self> {
        let property_store = context.entity_store.get_property_store::<E>();

        // Check for an index.
        {
            let property_index_value_store = property_store.get_with_id(P::index_id());
            if property_index_value_store.index_unindexed_entities(context) {
                return property_index_value_store
                    .get_index_set_with_hash(P::hash_property_value(&value.make_canonical()))
                    .map(SourceSet::IndexSet);
            }
        }

        // No index. Check if derived.
        if P::is_derived() {
            Some(SourceSet::PropertySet(Box::new(DerivedPropertySource::<
                E,
                P,
            >::new(
                context, value
            ))))
        } else {
            let property_value_store = property_store.get::<P>();
            let values: &'a RawPropertyValueVec<P> = &property_value_store.data;

            Some(SourceSet::<'a>::PropertySet(Box::<
                ConcretePropertySource<'a, E, P>,
            >::new(
                ConcretePropertySource::<'a, E, P>::new(
                    values,
                    value,
                    context.get_entity_count::<E>(),
                ),
            )))
        }
    }

    pub(super) fn upper_len(&self) -> usize {
        match self {
            SourceSet::IndexSet(source) => source.len(),
            SourceSet::PropertySet(source) => source.upper_len(),
        }
    }

    pub(super) fn contains(&self, id: EntityId<E>) -> bool {
        match self {
            SourceSet::IndexSet(source) => source.contains(&id),
            SourceSet::PropertySet(source) => source.contains(id),
        }
    }

    pub(super) fn into_iter(self) -> SourceIterator<'a, E> {
        match self {
            SourceSet::IndexSet(ids) => {
                SourceIterator::IndexIter(IndexSetIterator::from_index_set(ids))
            }
            SourceSet::PropertySet(property_vec) => {
                SourceIterator::PropertyVecIter(property_vec.to_iter())
            }
        }
    }
}

/// Kinds of iterators that are used as a basis for `QueryResultIterator`
pub(crate) enum SourceIterator<'a, E: Entity> {
    /// An iterator over an index set
    IndexIter(IndexSetIterator<'a, E>),
    /// An iterator over a property vector
    PropertyVecIter(Box<dyn Iterator<Item = EntityId<E>> + 'a>),
    /// An iterator over the entire population
    WholePopulation(EntityIterator<E>),
    /// An empty iterator
    Empty,
}

impl<'a, E: Entity> Iterator for SourceIterator<'a, E> {
    type Item = EntityId<E>;

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

    fn count(self) -> usize {
        // Some of these iterators have very efficient `count` implementations, and we want to exploit
        // them when they exist.
        match self {
            SourceIterator::IndexIter(mut iter) => iter.with_iter_mut(|iter| iter.count()),
            SourceIterator::PropertyVecIter(iter) => iter.count(),
            SourceIterator::WholePopulation(iter) => iter.count(),
            SourceIterator::Empty => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::{define_entity, define_property};

    define_entity!(Person);
    define_property!(struct Age(u8), Person, default_const = Age(0));

    #[test]
    fn test_iterators() {
        let values: RawPropertyValueVec<Age> =
            [0u8, 3, 2, 3, 4, 5, 3].into_iter().map(Age).collect();
        let people_set = HashSet::from_iter([
            EntityId::new(0),
            EntityId::new(2),
            EntityId::new(3),
            EntityId::new(6),
        ]);
        let people_set = RefCell::new(people_set);
        let people_set_ref = people_set.borrow();
        {
            let pvi = SourceSet::PropertySet(Box::new(ConcretePropertySource::<_, Age>::new(
                &values,
                Age(3u8),
                8,
            )));
            let isi = SourceSet::IndexSet(people_set_ref);
            let sources = vec![pvi, isi];

            for source in sources {
                for id in source.into_iter() {
                    print!("{:?}, ", id);
                }
                println!();
            }
        }
    }
}
