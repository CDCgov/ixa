#![allow(unused)]
//! An `Index<P: Property>` implements a map from `property_value` to `set_of_entities`,
//! where the `set_of_entities` is a `HashSet<EntityId<P>>` of entities having the value of
//! their property `P` equal to `property_value`.
//!
//! Type-erased access to the index is provided through `PropertyValueStore`, which provides
//! the type-erased interface to property storage more generally.
//!
//! `Index<P>::lookup: HashTable<(T, HashSet<EntityId<E>>)>` is a hash table,
//! _not_ a hash map. The difference is, a hash table is keyed by a `u64`, in our case,
//! the 64-bit hash of the property value, and allows for a custom equality check. For
//! equality, we compare the 128-bit hash of the property value. This allows us to keep the
//! lookup operation completely type-erased. The value associated with the key is a tuple
//! of the property value and a set of `EntityId<E>`s. The property value is stored so that
//!
//! 1. we can compute its 128-bit hash to check equality during lookup,
//! 2. we can iterate over (property value, set of `EntityId<E>`s) pairs in the typed API, and
//! 3. we can iterate over (serialized property value, set of `EntityId<E>`s) pairs in the
//!    type-erased API.

use hashbrown::HashTable;
use log::{error, trace};

use crate::entity::property::Property;
use crate::entity::{Entity, EntityId, HashValueType};
use crate::HashSet;

/// The typed index.
#[derive(Default)]
pub struct Index<E: Entity, P: Property<E>> {
    // Primarily for debugging purposes
    #[allow(dead_code)]
    pub(super) name: &'static str,

    // We store a copy of the value here so that we can iterate over
    // it in the typed API, and so that the type-erased API can
    // access some serialization of it.
    data: HashTable<(P::CanonicalValue, HashSet<EntityId<E>>)>,

    // The largest person ID that has been indexed. Used so that we
    // can lazily index when a person is added.
    pub(super) max_indexed: usize,
}

impl<E: Entity, P: Property<E>> Index<E, P> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: P::name(),
            data: HashTable::default(),
            max_indexed: 0,
        }
    }

    /// Inserts an entity into the set associated with `key`, creating a new set if one does not yet
    /// exist. Returns a `bool` according to whether the `entity_id` already existed in the set.
    pub fn add_entity(&mut self, key: &P::CanonicalValue, entity_id: EntityId<E>) -> bool {
        trace!("adding entity {:?} to index {}", entity_id, P::name());
        let hash = P::hash_property_value(key);

        // > `hasher` is called if entries need to be moved or copied to a new table.
        // > This must return the same hash value that each entry was inserted with.
        #[allow(clippy::cast_possible_truncation)]
        let hasher = |(stored_value, _stored_set): &_| P::hash_property_value(stored_value) as u64;
        // Equality is determined by comparing the full 128-bit hashes. We do not expect any
        // collisions before the heat death of the universe.
        let hash128_equality = |(stored_value, _): &_| P::hash_property_value(stored_value) == hash;
        #[allow(clippy::cast_possible_truncation)]
        self.data
            .entry(hash as u64, hash128_equality, hasher)
            .or_insert_with(|| (*key, HashSet::default()))
            .get_mut()
            .1
            .insert(entity_id)
    }

    pub fn remove_entity(&mut self, key: &P::CanonicalValue, entity_id: EntityId<E>) {
        let hash = P::hash_property_value(key);
        self.remove_entity_with_hash(hash, entity_id);
    }

    /// Because the property value is stored beside the set of entities, the bucket for the value
    /// has to already be in the index in order to add an entity using only the property value hash.
    pub fn add_entity_with_hash(&mut self, hash: HashValueType, entity_id: EntityId<E>) {
        // Equality is determined by comparing the full 128-bit hashes. We do not expect any
        // collisions before the heat death of the universe.
        let hash128_equality = |(stored_value, _): &_| P::hash_property_value(stored_value) == hash;

        #[allow(clippy::cast_possible_truncation)]
        if let Ok(mut entry) = self.data.find_entry(hash as u64, hash128_equality) {
            let (_, set) = entry.get_mut();
            set.insert(entity_id);
        } else {
            error!(
                "could not find entry for hash {} when adding person {:?} to index",
                hash, entity_id
            );
        }
    }

    /// Removing an entity only requires the hash.
    pub fn remove_entity_with_hash(&mut self, hash: HashValueType, entity_id: EntityId<E>) {
        // Equality is determined by comparing the full 128-bit hashes. We do not expect any
        // collisions before the heat death of the universe.
        let hash128_equality = |(stored_value, _): &_| P::hash_property_value(stored_value) == hash;

        #[allow(clippy::cast_possible_truncation)]
        if let Ok(mut entry) = self.data.find_entry(hash as u64, hash128_equality) {
            let (_, set) = entry.get_mut();
            set.remove(&entity_id);
            // Clean up the entry if there are no entities
            if set.is_empty() {
                entry.remove();
            }
        } else {
            error!(
                "could not find entry for hash {} when removing entity {:?} from index",
                hash, entity_id
            );
        }
    }

    /// Fetching a set only requires the hash.
    pub fn get_with_hash(&self, hash: HashValueType) -> Option<&HashSet<EntityId<E>>> {
        // Equality is determined by comparing the full 128-bit hashes. We do not expect
        // any collisions before the heat death of the universe.
        let hash128_equality = |(stored_value, _): &_| P::hash_property_value(stored_value) == hash;
        #[allow(clippy::cast_possible_truncation)]
        self.data
            .find(hash as u64, hash128_equality)
            .map(|(_, set)| set)
    }
}

mod test {
    // Tests in `src/entity/query.rs` also exercise indexing code.
    use super::Index;
    use crate::hashing::{hash_serialized_128, one_shot_128};
    use crate::prelude::*;
    use crate::{define_entity, define_multi_property, define_property};

    define_entity!(Person);
    define_property!(struct Age(pub u8), Person, default_const = Age(0));
    define_property!(struct Weight(pub u8), Person, default_const = Weight(0));
    define_property!(struct Height(pub u8), Person, default_const = Height(0));

    define_multi_property!((Age, Weight, Height), Person);
    define_multi_property!((Weight, Height, Age), Person);

    type AWH = (Age, Weight, Height);
    type WHA = (Weight, Height, Age);

    #[test]
    fn test_multi_property_index_typed_api() {
        let mut context = Context::new();

        context.index_property::<Person, WHA>();
        context.index_property::<Person, AWH>();

        context
            .add_entity((Age(1u8), Weight(2u8), Height(3u8)))
            .unwrap();

        let mut results_a = Default::default();
        context.with_query_results((Age(1u8), Weight(2u8), Height(3u8)), &mut |results| {
            results_a = results.clone()
        });
        assert_eq!(results_a.len(), 1);

        let mut results_b = Default::default();
        context.with_query_results((Weight(2u8), Height(3u8), Age(1u8)), &mut |results| {
            results_b = results.clone()
        });
        assert_eq!(results_b.len(), 1);

        assert_eq!(results_a, results_b);
        println!("Results: {:?}", results_a);

        context
            .add_entity((Weight(1u8), Height(2u8), Age(3u8)))
            .unwrap();

        let mut results_a = Default::default();
        context.with_query_results((Weight(1u8), Height(2u8), Age(3u8)), &mut |results| {
            results_a = results.clone()
        });
        assert_eq!(results_a.len(), 1);

        let mut results_b = Default::default();
        context.with_query_results((Age(3u8), Weight(1u8), Height(2u8)), &mut |results| {
            results_b = results.clone()
        });
        assert_eq!(results_b.len(), 1);

        assert_eq!(results_a, results_b);

        println!("Results: {:?}", results_a);
    }

    #[test]
    fn index_name() {
        let index = Index::<Person, Age>::new();
        assert!(index.name.contains("Age"));
    }

    #[test]
    fn test_index_value_compute_same_values() {
        let value = hash_serialized_128("test value");
        let value2 = hash_serialized_128("test value");
        assert_eq!(one_shot_128(&value), one_shot_128(&value2));
    }

    #[test]
    fn test_index_value_compute_different_values() {
        let value1 = Age(42);
        let value2 = Age(43);
        assert_ne!(
            <Age as Property<Person>>::hash_property_value(&value1),
            <Age as Property<Person>>::hash_property_value(&value2)
        );
    }
}
