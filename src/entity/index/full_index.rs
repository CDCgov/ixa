//! Full property-value index that maps each distinct value to the set of matching entity IDs.

use hashbrown::HashTable;
use log::{error, trace};

use crate::entity::{Entity, EntityId, HashValueType};
use crate::hashing::IndexSet;
use crate::prelude::Property;

/// An index that maintains a full set of entity IDs for each distinct property value.
/// The entity IDs are stored in an `IndexSet` for both fast containment checks and fast
/// direct indexing (fast random sampling).
#[derive(Default)]
pub struct FullIndex<E: Entity, P: Property<E>> {
    // We store a copy of the value here so that we can iterate over
    // it in the typed API, and so that the type-erased API can
    // access some serialization of it.
    data: HashTable<(P::CanonicalValue, IndexSet<EntityId<E>>)>,

    // The largest person ID that has been indexed. Used so that we
    // can lazily index when a person is added.
    pub(in crate::entity) max_indexed: usize,
}

impl<E: Entity, P: Property<E>> FullIndex<E, P> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: HashTable::default(),
            max_indexed: 0,
        }
    }

    /// Inserts an entity into the set associated with `key`, creating a new set if one does not yet
    /// exist.
    pub fn add_entity(&mut self, key: &P::CanonicalValue, entity_id: EntityId<E>) {
        trace!("adding entity {:?} to index {}", entity_id, P::name());
        let hash = P::hash_property_value(key);

        // `hasher` is called if entries need to be moved or copied to a new table.
        // This must return the same hash value that each entry was inserted with.
        #[allow(clippy::cast_possible_truncation)]
        let hasher = |(stored_value, _stored_set): &_| P::hash_property_value(stored_value) as u64;
        // Equality is determined by comparing the full 128-bit hashes. We do not expect any
        // collisions before the heat death of the universe.
        let hash128_equality = |(stored_value, _): &_| P::hash_property_value(stored_value) == hash;
        #[allow(clippy::cast_possible_truncation)]
        self.data
            .entry(hash as u64, hash128_equality, hasher)
            .or_insert_with(|| (*key, IndexSet::default()))
            .get_mut()
            .1
            .insert(entity_id);
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
            set.swap_remove(&entity_id);
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
    pub fn get_with_hash(&self, hash: HashValueType) -> Option<&IndexSet<EntityId<E>>> {
        // Equality is determined by comparing the full 128-bit hashes. We do not expect
        // any collisions before the heat death of the universe.
        let hash128_equality = |(stored_value, _): &_| P::hash_property_value(stored_value) == hash;
        #[allow(clippy::cast_possible_truncation)]
        self.data
            .find(hash as u64, hash128_equality)
            .map(|(_, set)| set)
    }
}

#[cfg(test)]
mod tests {
    // Tests in `src/entity/query.rs` also exercise indexing code.
    use crate::hashing::{hash_serialized_128, one_shot_128};
    use crate::prelude::*;

    define_entity!(Person);
    define_property!(struct Age(u8), Person, default_const = Age(0));
    define_property!(struct Weight(u8), Person, default_const = Weight(0));
    define_property!(struct Height(u8), Person, default_const = Height(0));

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
            results_a = results.into_iter().collect::<Vec<_>>()
        });
        assert_eq!(results_a.len(), 1);

        let mut results_b = Default::default();
        context.with_query_results((Weight(2u8), Height(3u8), Age(1u8)), &mut |results| {
            results_b = results.into_iter().collect::<Vec<_>>()
        });
        assert_eq!(results_b.len(), 1);

        assert_eq!(results_a, results_b);
        println!("Results: {:?}", results_a);

        context
            .add_entity((Weight(1u8), Height(2u8), Age(3u8)))
            .unwrap();

        let mut results_a = Default::default();
        context.with_query_results((Weight(1u8), Height(2u8), Age(3u8)), &mut |results| {
            results_a = results.into_iter().collect::<Vec<_>>()
        });
        assert_eq!(results_a.len(), 1);

        let mut results_b = Default::default();
        context.with_query_results((Age(3u8), Weight(1u8), Height(2u8)), &mut |results| {
            results_b = results.into_iter().collect::<Vec<_>>()
        });
        assert_eq!(results_b.len(), 1);

        assert_eq!(results_a, results_b);

        println!("Results: {:?}", results_a);
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
