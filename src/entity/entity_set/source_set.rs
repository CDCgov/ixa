//! `SourceSet` and `SourceIterator` are companion types that provide a uniform
//! interface over several internal data-source representations used by query
//! execution.
//!
//! `SourceSet` is the membership-oriented view (`contains`, `sort_key`), while
//! `SourceIterator` (defined in `source_iterator.rs`) is the traversal-oriented
//! view. A `SourceSet` can be converted into a `SourceIterator<'c>` over the same
//! logical set. The lifetime `'c` is the lifetime of the (immutable) borrow of
//! the underlying `Context`.
//!
//! Several auxiliary types sit between raw storage and these companion types:
//! - `AbstractPropertySource`: type-erased property-backed set interface
//! - `ConcretePropertySource`: wrapper over concrete property-value vectors
//! - `DerivedPropertySource`: wrapper that computes derived property values
//! - `IndexSetIterator` (in `source_iterator.rs`): self-referential wrapper over index-set iterators
//!
//! In some cases we intentionally do not split into separate intermediate
//! `*Set` and `*Iterator` wrappers. For simplicity, `ConcretePropertySource` and
//! `DerivedPropertySource` each implement both the set-facing
//! `AbstractPropertySource` API and `Iterator`.
//!
//! `EntitySetIterator<'c>` uses `SourceSet<'c>` and `SourceIterator<'c>` to
//! evaluate intersections. Sources may be empty, whole-population, index-backed,
//! singleton, or property-backed. The iterator chooses the smallest source as the driver and
//! checks candidate IDs against remaining sources.
//!
//! ## Source ordering and `cost_hint`
//!
//! `SourceSet::sort_key()` returns `(length_upper_bound, cost_hint)`. Source ordering for
//! intersections and unions uses lexicographic ordering on this tuple:
//! 1. smaller `length_upper_bound` first,
//! 2. on ties, smaller `cost_hint` first.
//!
//! The `cost_hint` is a lightweight heuristic for relative per-candidate
//! membership/iteration work. It is not a correctness value; it is only used to
//! break ties when upper bounds are equal.
//!
//! | Source kind                  | `cost_hint` |
//! | ---------------------------- | ----------- |
//! | `SourceSet::Empty`           | `0`         |
//! | `SourceSet::Entity`          | `1`         |
//! | `SourceSet::Population`      | `2`         |
//! | `SourceSet::IndexSet`        | `3`         |
//! | `ConcretePropertySource`     | `5`         |
//! | `DerivedPropertySource`      | `6`         |

use std::cell::Ref;
use std::marker::PhantomData;

use super::source_iterator::{IndexSetIterator, SourceIterator};
use crate::entity::index::IndexSetResult;
use crate::entity::property_value_store_core::RawPropertyValueVec;
use crate::entity::{ContextEntitiesExt, Entity, EntityId, PopulationIterator};
use crate::hashing::{HashValueType, IndexSet};
use crate::prelude::Property;
use crate::Context;

pub(super) type BxPropertySource<'a, E> = Box<dyn AbstractPropertySource<'a, E> + 'a>;

#[derive(Copy, Clone, Eq, PartialEq)]
pub(crate) struct PropertySourceId {
    pub property_id: usize,
    pub value_hash: HashValueType,
}

/// Type erased property source representing the (abstract) set of `EntityId<E>`s
/// for which a particular property has a particular value. This is used for
/// both `ConcretePropertyVec<'a, P: Property>` and `DerivedPropertySource<'a, P: Property>`.
pub(crate) trait AbstractPropertySource<'a, E: Entity>:
    Iterator<Item = EntityId<E>>
{
    /// Identity of the logical property query represented by this source.
    fn id(&self) -> PropertySourceId;

    /// A test that `entity_id` is contained in the (abstractly
    /// defined) set. This operation is very efficient.
    fn contains(&self, entity_id: EntityId<E>) -> bool;

    /// Ordering key used for source selection.
    fn sort_key(&self) -> (usize, u8);
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
    fn id(&self) -> PropertySourceId {
        PropertySourceId {
            property_id: P::index_id(),
            value_hash: P::hash_property_value(&self.value.make_canonical()),
        }
    }

    fn contains(&self, entity_id: EntityId<E>) -> bool {
        P::compute_derived(self.context, entity_id) == self.value
    }

    fn sort_key(&self) -> (usize, u8) {
        (self.context.get_entity_count::<E>(), 6)
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
    fn id(&self) -> PropertySourceId {
        PropertySourceId {
            property_id: P::index_id(),
            value_hash: P::hash_property_value(&self.value.make_canonical()),
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

    fn sort_key(&self) -> (usize, u8) {
        let upper = if !self.is_default_value {
            self.values.len()
        } else {
            // If the property is default value, we can't use the length of the property vector, because
            // unset values are implicitly equal to the default value. Instead, we use the population size.
            self.population_size
        };
        (upper, 5)
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

/// Represents the set of `EntityId<E>`s for which a particular `Property` has a particular value.
pub(crate) enum SourceSet<'a, E: Entity> {
    Empty,
    Population(usize),
    #[allow(dead_code)]
    Entity(EntityId<E>),
    IndexSet(Ref<'a, IndexSet<EntityId<E>>>),
    PropertySet(BxPropertySource<'a, E>),
}

impl<'a, E: Entity> PartialEq for SourceSet<'a, E> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Empty, Self::Empty) => true,
            (Self::Population(left), Self::Population(right)) => left == right,
            (Self::Entity(left), Self::Entity(right)) => left == right,
            (Self::IndexSet(left), Self::IndexSet(right)) => std::ptr::eq(&**left, &**right),
            (Self::PropertySet(left), Self::PropertySet(right)) => left.id() == right.id(),
            _ => false,
        }
    }
}

impl<'a, E: Entity> Eq for SourceSet<'a, E> {}

impl<'a, E: Entity> SourceSet<'a, E> {
    pub(super) fn try_len(&self) -> Option<usize> {
        match self {
            SourceSet::Empty => Some(0),
            SourceSet::Population(population) => Some(*population),
            SourceSet::Entity(_) => Some(1),
            SourceSet::IndexSet(source) => Some(source.len()),
            SourceSet::PropertySet(_) => None,
        }
    }

    /// Ordering key used for source selection.
    pub(super) fn sort_key(&self) -> (usize, u8) {
        match self {
            SourceSet::Empty => (0, 0),
            SourceSet::Entity(_) => (1, 1),
            SourceSet::Population(population) => (*population, 2),
            SourceSet::IndexSet(source) => (source.len(), 3),
            SourceSet::PropertySet(source) => source.sort_key(),
        }
    }

    /// A constructor for `SourceSet`s during construction of `EntitySet` in
    /// `Query<E>::new_query_result()`. Returns `None` if the set is empty.
    ///
    /// We first look for an index set. If not found, we check if the property is derived.
    /// For derived properties, we wrap a reference to the `Context`. For nonderived
    /// properties, we wrap a reference to the property's backing vector.
    pub(crate) fn new<P: Property<E>>(value: P, context: &'a Context) -> Option<Self> {
        let property_store = context.entity_store.get_property_store::<E>();

        // Check for an index.
        {
            match property_store.get_index_set_with_hash_for_property_id(
                context,
                P::index_id(),
                P::hash_property_value(&value.make_canonical()),
            ) {
                IndexSetResult::Set(entity_set) => {
                    return Some(SourceSet::IndexSet(entity_set));
                }
                IndexSetResult::Empty => {
                    return None;
                }
                IndexSetResult::Unsupported => {}
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

    pub(super) fn contains(&self, id: EntityId<E>) -> bool {
        match self {
            SourceSet::Empty => false,
            SourceSet::Population(population) => id.0 < *population,
            SourceSet::Entity(entity_id) => *entity_id == id,
            SourceSet::IndexSet(source) => source.contains(&id),
            SourceSet::PropertySet(source) => source.contains(id),
        }
    }

    pub(super) fn into_iter(self) -> SourceIterator<'a, E> {
        match self {
            SourceSet::Empty => SourceIterator::Empty,
            SourceSet::Population(population) => {
                SourceIterator::Population(PopulationIterator::new(population))
            }
            SourceSet::Entity(entity_id) => SourceIterator::Entity {
                id: entity_id,
                exhausted: false,
            },
            SourceSet::IndexSet(ids) => {
                SourceIterator::IndexIter(IndexSetIterator::from_index_set(ids))
            }
            SourceSet::PropertySet(property_vec) => SourceIterator::PropertyVecIter(property_vec),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::{define_derived_property, define_entity, define_property};

    define_entity!(Person);
    define_property!(struct Age(u8), Person, default_const = Age(0));
    define_property!(struct Flag(bool), Person, default_const = Flag(false));
    define_derived_property!(struct IsAdult(bool), Person, [Age], |age| IsAdult(age.0 >= 18));

    #[test]
    fn source_set_variants_basic_behavior() {
        let empty = SourceSet::<Person>::Empty;
        assert_eq!(empty.sort_key(), (0, 0));
        assert!(!empty.contains(EntityId::new(0)));
        assert_eq!(empty.into_iter().count(), 0);

        let population = SourceSet::<Person>::Population(3);
        assert_eq!(population.sort_key(), (3, 2));
        assert!(population.contains(EntityId::new(0)));
        assert!(!population.contains(EntityId::new(3)));
        let population_ids = SourceSet::<Person>::Population(3)
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(
            population_ids,
            vec![EntityId::new(0), EntityId::new(1), EntityId::new(2)]
        );

        let singleton = SourceSet::<Person>::Entity(EntityId::new(9));
        assert_eq!(singleton.sort_key(), (1, 1));
        assert!(singleton.contains(EntityId::new(9)));
        assert!(!singleton.contains(EntityId::new(8)));
        let singleton_ids = SourceSet::<Person>::Entity(EntityId::new(9))
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(singleton_ids, vec![EntityId::new(9)]);

        let ids = RefCell::new(
            [EntityId::new(1), EntityId::new(4), EntityId::new(8)]
                .into_iter()
                .collect::<IndexSet<_>>(),
        );
        let ids_ref = ids.borrow();
        let indexed = SourceSet::<Person>::IndexSet(ids_ref);
        assert_eq!(indexed.sort_key(), (3, 3));
        assert!(indexed.contains(EntityId::new(4)));
        assert!(!indexed.contains(EntityId::new(2)));
    }

    #[test]
    fn source_set_new_uses_indexed_or_unindexed_backing() {
        let mut context = Context::new();
        for age in [1u8, 2, 2, 3] {
            context.add_entity((Age(age), Flag(true))).unwrap();
        }

        assert!(matches!(
            SourceSet::<Person>::new::<Age>(Age(2), &context).unwrap(),
            SourceSet::PropertySet(_)
        ));
        let unindexed_ids = SourceSet::<Person>::new::<Age>(Age(2), &context)
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>();

        context.index_property::<Person, Age>();
        assert!(matches!(
            SourceSet::<Person>::new::<Age>(Age(2), &context).unwrap(),
            SourceSet::IndexSet(_)
        ));

        let indexed_ids = SourceSet::<Person>::new::<Age>(Age(2), &context)
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(unindexed_ids, indexed_ids);
    }

    #[test]
    fn source_set_new_derived_vs_nonderived_backing() {
        let mut context = Context::new();
        context.add_entity((Age(12), Flag(true))).unwrap();
        context.add_entity((Age(20), Flag(true))).unwrap();
        context.add_entity((Age(44), Flag(false))).unwrap();

        let nonderived = SourceSet::<Person>::new::<Age>(Age(20), &context).unwrap();
        assert!(matches!(nonderived, SourceSet::PropertySet(_)));

        let derived = SourceSet::<Person>::new::<IsAdult>(IsAdult(true), &context).unwrap();
        assert!(matches!(derived, SourceSet::PropertySet(_)));

        let derived_ids = SourceSet::<Person>::new::<IsAdult>(IsAdult(true), &context)
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(derived_ids, vec![EntityId::new(1), EntityId::new(2)]);
    }
}
