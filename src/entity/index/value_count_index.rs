//! Value-count index that maintains only counts per distinct property value.

use std::ops::AddAssign;

use hashbrown::HashTable;
use log::{error, trace};

use crate::entity::{Entity, EntityId, HashValueType};
use crate::prelude::Property;

#[derive(Default)]
pub struct ValueCountIndex<E: Entity, P: Property<E>> {
    data: HashTable<(P::CanonicalValue, usize)>,
    pub(in crate::entity) max_indexed: usize,
}

impl<E: Entity, P: Property<E>> ValueCountIndex<E, P> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: HashTable::default(),
            max_indexed: 0,
        }
    }

    /// Increments the count for `key`.
    pub fn add_entity(&mut self, key: &P::CanonicalValue, entity_id: EntityId<E>) {
        trace!("adding entity {:?} to index {}", entity_id, P::name());
        let hash = P::hash_property_value(key);

        // `hasher` is called if entries need to be moved or copied to a new table.
        // This must return the same hash value that each entry was inserted with.
        #[allow(clippy::cast_possible_truncation)]
        let hasher = |(stored_value, _count): &_| P::hash_property_value(stored_value) as u64;
        // Equality is determined by comparing the full 128-bit hashes. We do not expect any
        // collisions before the heat death of the universe.
        let hash128_equality = |(stored_value, _): &_| P::hash_property_value(stored_value) == hash;
        #[allow(clippy::cast_possible_truncation)]
        self.data
            .entry(hash as u64, hash128_equality, hasher)
            .or_insert_with(|| (*key, 0))
            .get_mut()
            .1
            .add_assign(1);
    }

    pub fn remove_entity(&mut self, key: &P::CanonicalValue, _entity_id: EntityId<E>) {
        let hash = P::hash_property_value(key);
        self.remove_entity_with_hash(hash, _entity_id);
    }

    /// Because the property value is stored beside the count, the bucket for the value
    /// has to already be in the index in order to add an entity using only the property value hash.
    pub fn add_entity_with_hash(&mut self, hash: HashValueType, entity_id: EntityId<E>) {
        // Equality is determined by comparing the full 128-bit hashes. We do not expect any
        // collisions before the heat death of the universe.
        let hash128_equality = |(stored_value, _): &_| P::hash_property_value(stored_value) == hash;

        #[allow(clippy::cast_possible_truncation)]
        if let Ok(mut entry) = self.data.find_entry(hash as u64, hash128_equality) {
            let (_, count) = entry.get_mut();
            *count += 1;
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
            let (_, count) = entry.get_mut();
            if *count == 0 {
                error!(
                    "attempted to remove entity from value-count index with count 0 for hash {}",
                    hash
                );
                return;
            }
            *count -= 1;
            if *count == 0 {
                entry.remove();
            }
        } else {
            error!(
                "could not find entry for hash {} when removing entity {:?} from index",
                hash, entity_id
            );
        }
    }

    /// Returns the count for the given hash, if present.
    pub fn get_with_hash(&self, hash: HashValueType) -> Option<usize> {
        // Equality is determined by comparing the full 128-bit hashes. We do not expect
        // any collisions before the heat death of the universe.
        let hash128_equality = |(stored_value, _): &_| P::hash_property_value(stored_value) == hash;
        #[allow(clippy::cast_possible_truncation)]
        self.data
            .find(hash as u64, hash128_equality)
            .map(|(_, c)| *c)
    }
}

#[cfg(test)]
mod tests {
    use crate::entity::index::ValueCountIndex;
    use crate::entity::PropertyIndexType;
    use crate::hashing::{hash_serialized_128, one_shot_128};
    use crate::prelude::*;

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
        let property_store = context.entity_store.get_property_store_mut::<Person>();

        property_store.set_property_indexed::<AWH>(PropertyIndexType::ValueCountIndex);
        property_store.set_property_indexed::<WHA>(PropertyIndexType::ValueCountIndex);

        context
            .add_entity((Age(1u8), Weight(2u8), Height(3u8)))
            .unwrap();

        assert_eq!(
            context.query_entity_count((Age(1u8), Weight(2u8), Height(3u8))),
            1
        );
        assert_eq!(
            context.query_entity_count((Weight(2u8), Height(3u8), Age(1u8))),
            1
        );

        context
            .add_entity((Weight(1u8), Height(2u8), Age(3u8)))
            .unwrap();

        assert_eq!(
            context.query_entity_count((Weight(1u8), Height(2u8), Age(3u8))),
            1
        );
        assert_eq!(
            context.query_entity_count((Age(3u8), Weight(1u8), Height(2u8))),
            1
        );
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

    #[test]
    fn test_add_remove_counts() {
        let mut index: ValueCountIndex<Person, Age> = ValueCountIndex::new();
        let value = Age(10);
        let hash = <Age as Property<Person>>::hash_property_value(&value);

        assert_eq!(index.get_with_hash(hash), None);

        index.add_entity(&value, EntityId::new(0));
        assert_eq!(index.get_with_hash(hash), Some(1));

        index.add_entity(&value, EntityId::new(1));
        assert_eq!(index.get_with_hash(hash), Some(2));

        index.remove_entity(&value, EntityId::new(0));
        assert_eq!(index.get_with_hash(hash), Some(1));

        index.remove_entity(&value, EntityId::new(1));
        assert_eq!(index.get_with_hash(hash), None);
    }

    #[test]
    fn test_add_remove_with_hash_requires_existing_bucket() {
        let mut index: ValueCountIndex<Person, Age> = ValueCountIndex::new();
        let value = Age(12);
        let hash = <Age as Property<Person>>::hash_property_value(&value);

        index.add_entity_with_hash(hash, EntityId::new(0));
        assert_eq!(index.get_with_hash(hash), None);

        index.add_entity(&value, EntityId::new(0));
        assert_eq!(index.get_with_hash(hash), Some(1));

        index.add_entity_with_hash(hash, EntityId::new(1));
        assert_eq!(index.get_with_hash(hash), Some(2));

        index.remove_entity_with_hash(hash, EntityId::new(1));
        assert_eq!(index.get_with_hash(hash), Some(1));

        index.remove_entity_with_hash(hash, EntityId::new(0));
        assert_eq!(index.get_with_hash(hash), None);
    }
}
