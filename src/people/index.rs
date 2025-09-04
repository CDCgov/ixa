//! Each property type `P: PersonProperty` has a corresponding `Index<P>` type a single
//! instance of which is stored in the `PeopleData::property_indexes: RefCell<HashMap<TypeId,
//! BxIndex>>` map. The map `PeopleData::property_indexes` is keyed by the `TypeId` of the
//! property tag type `P`, _not_ the value type of the property, `PersonProperty::Value`.
//!
//! `Index<P>::lookup: HashTable<(T, std::collections::HashSet<PersonId>)>` is a hash table,
//! _not_ a hash map. The difference is, a hash table is keyed by a `u64`, in our case,
//! the 64-bit hash of the property value, and allows for a custom equality check. For
//! equality, we compare the 128-bit hash of the property value. This allows us to keep the
//! lookup operation completely type-erased. The value associated with the key is a tuple
//! of the property value and a set of `PersonId`s. The property value is stored so that
//!
//! 1. we can compute its 128-bit hash to check equality during lookup,
//! 2. we can iterate over (property value, set of `PersonId`s) pairs in the typed API, and
//! 3. we can iterate over (serialized property value, set of `PersonId`s) pairs in the
//!    type-erased API.

use crate::{people::HashValueType, Context, ContextPeopleExt, HashSet, PersonId, PersonProperty};
use hashbrown::HashTable;
use log::{error, trace};

pub type BxIndex = Box<dyn TypeErasedIndex>;

/// The typed index.
#[derive(Default)]
pub struct Index<T: PersonProperty> {
    // Primarily for debugging purposes
    #[allow(dead_code)]
    pub(super) name: &'static str,

    // We store a copy of the value here so that we can iterate over
    // it in the typed API, and so that the type-erased API can
    // access some serialization of it.
    lookup: HashTable<(T::CanonicalValue, HashSet<PersonId>)>,

    // The largest person ID that has been indexed. Used so that we
    // can lazily index when a person is added.
    pub(super) max_indexed: usize,

    pub(super) is_indexed: bool,
}

/// Implements the typed API
impl<T: PersonProperty> Index<T> {
    #[must_use]
    pub fn new() -> Box<Self> {
        Box::new(Self {
            name: T::name(),
            lookup: HashTable::default(),
            max_indexed: 0,
            is_indexed: false,
        })
    }

    /// Inserts an entity into the set associated with `key`, creating a new set if one does not yet
    /// exist. Returns a `bool` according to whether the `entity_id` already existed in the set.
    pub fn add_person(&mut self, key: &T::CanonicalValue, entity_id: PersonId) -> bool {
        trace!("adding person {} to index {}", entity_id, T::name());
        let hash = T::hash_property_value(key);

        // > `hasher` is called if entries need to be moved or copied to a new table.
        // > This must return the same hash value that each entry was inserted with.
        #[allow(clippy::cast_possible_truncation)]
        let hasher = |(stored_value, _stored_set): &_| T::hash_property_value(stored_value) as u64;
        // Equality is determined by comparing the full 128-bit hashes. We do not expect any
        // collisions before the heat death of the universe.
        let hash128_equality = |(stored_value, _): &_| T::hash_property_value(stored_value) == hash;
        #[allow(clippy::cast_possible_truncation)]
        self.lookup
            .entry(hash as u64, hash128_equality, hasher)
            .or_insert_with(|| (*key, HashSet::default()))
            .get_mut()
            .1
            .insert(entity_id)
    }

    pub fn remove_person(&mut self, key: &T::CanonicalValue, entity_id: PersonId) {
        let hash = T::hash_property_value(key);
        self.remove_person_with_hash(hash, entity_id);
    }
}

/// This trait Encapsulates the type-erased API.
pub trait TypeErasedIndex {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// Adding a person only requires the hash if the value is already in the index.
    #[allow(dead_code)]
    fn add_person_with_hash(&mut self, hash: HashValueType, entity_id: PersonId);

    /// Removing a person only requires the hash.
    fn remove_person_with_hash(&mut self, hash: HashValueType, entity_id: PersonId);

    /// Fetching a set only requires the hash.
    fn get_with_hash(&self, hash: HashValueType) -> Option<&HashSet<PersonId>>;

    fn is_indexed(&self) -> bool;
    fn set_indexed(&mut self, is_indexed: bool);
    fn index_unindexed_people(&mut self, context: &Context);

    /// Produces an iterator over pairs (serialized property value, set of `PersonId`s).
    fn iter_serialized_values_people(
        &self,
    ) -> Box<dyn Iterator<Item = (String, &HashSet<PersonId>)> + '_>;
}

// Implements the type-erased API for Index<T>.
impl<T: PersonProperty> TypeErasedIndex for Index<T> {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn add_person_with_hash(&mut self, hash: HashValueType, entity_id: PersonId) {
        // Equality is determined by comparing the full 128-bit hashes. We do not expect any
        // collisions before the heat death of the universe.
        let hash128_equality = |(stored_value, _): &_| T::hash_property_value(stored_value) == hash;

        #[allow(clippy::cast_possible_truncation)]
        if let Ok(mut entry) = self.lookup.find_entry(hash as u64, hash128_equality) {
            let (_, set) = entry.get_mut();
            set.insert(entity_id);
        } else {
            error!(
                "could not find entry for hash {} when adding person {} to index",
                hash, entity_id
            );
        }
    }

    /// Removing a person only requires the hash.
    fn remove_person_with_hash(&mut self, hash: HashValueType, entity_id: PersonId) {
        // Equality is determined by comparing the full 128-bit hashes. We do not expect any
        // collisions before the heat death of the universe.
        let hash128_equality = |(stored_value, _): &_| T::hash_property_value(stored_value) == hash;

        #[allow(clippy::cast_possible_truncation)]
        if let Ok(mut entry) = self.lookup.find_entry(hash as u64, hash128_equality) {
            let (_, set) = entry.get_mut();
            set.remove(&entity_id);
            // Clean up the entry if there are no people
            if set.is_empty() {
                entry.remove();
            }
        } else {
            error!(
                "could not find entry for hash {} when removing person {} from index",
                hash, entity_id
            );
        }
    }

    /// Fetching a set only requires the hash.
    fn get_with_hash(&self, hash: HashValueType) -> Option<&HashSet<PersonId>> {
        // Equality is determined by comparing the full 128-bit hashes. We do not expect
        // any collisions before the heat death of the universe.
        let hash128_equality = |(stored_value, _): &_| T::hash_property_value(stored_value) == hash;
        #[allow(clippy::cast_possible_truncation)]
        self.lookup
            .find(hash as u64, hash128_equality)
            .map(|(_, set)| set)
    }

    fn is_indexed(&self) -> bool {
        self.is_indexed
    }

    fn set_indexed(&mut self, is_indexed: bool) {
        self.is_indexed = is_indexed;
    }

    fn index_unindexed_people(&mut self, context: &Context) {
        if !self.is_indexed {
            return;
        }
        let current_pop = context.get_current_population();
        trace!(
            "{}: indexing unindexed people {}..<{}",
            T::name(),
            self.max_indexed,
            current_pop
        );

        for id in self.max_indexed..current_pop {
            let person_id = PersonId(id);
            let value = context.get_person_property(person_id, T::get_instance());
            self.add_person(&T::make_canonical(value), person_id);
        }
        self.max_indexed = current_pop;
    }

    fn iter_serialized_values_people(
        &self,
    ) -> Box<dyn Iterator<Item = (String, &HashSet<PersonId>)> + '_> {
        Box::new(self.lookup.iter().map(|(k, v)| (T::get_display(k), v)))
    }
}

pub fn process_indices(
    context: &Context,
    remaining_indices: &[&BxIndex],
    property_names: &mut Vec<String>,
    current_matches: &HashSet<PersonId>,
    print_fn: &dyn Fn(&Context, &[String], usize),
) {
    if remaining_indices.is_empty() {
        print_fn(context, property_names, current_matches.len());
        return;
    }

    let (&next_index, rest_indices) = remaining_indices.split_first().unwrap();
    // If there is nothing in the index, we don't need to process it
    if !next_index.is_indexed() {
        return;
    }

    for (display, people) in next_index.iter_serialized_values_people() {
        let intersect = !property_names.is_empty();
        property_names.push(display);

        let matches = if intersect {
            &current_matches.intersection(people).copied().collect()
        } else {
            people
        };

        process_indices(context, rest_indices, property_names, matches, print_fn);
        property_names.pop();
    }
}

#[cfg(test)]
mod test {
    // Tests in `src/people/query.rs` also exercise indexing code.

    use crate::hashing::{hash_serialized_128, one_shot_128};
    use crate::people::index::Index;
    use crate::prelude::*;
    use crate::{define_multi_property, set_log_level, set_module_filter, PersonProperty};
    use log::LevelFilter;

    define_person_property!(Age, u8);
    define_person_property!(Weight, u8);
    define_person_property!(Height, u8);

    define_multi_property!(AWH, (Age, Weight, Height));
    define_multi_property!(WHA, (Weight, Height, Age));

    #[test]
    fn test_multi_property_index_typed_api() {
        let mut context = Context::new();
        set_log_level(LevelFilter::Trace);
        set_module_filter("ixa", LevelFilter::Trace);

        context.index_property(WHA);
        context.index_property(AWH);

        context
            .add_person(((Age, 1u8), (Weight, 2u8), (Height, 3u8)))
            .unwrap();

        let results_a = context.query_people((AWH, (1u8, 2u8, 3u8)));
        assert_eq!(results_a.len(), 1);

        let results_b = context.query_people((WHA, (2u8, 3u8, 1u8)));
        assert_eq!(results_b.len(), 1);

        assert_eq!(results_a, results_b);
        println!("Results: {:?}", results_a);

        context
            .add_person(((Weight, 1u8), (Height, 2u8), (Age, 3u8)))
            .unwrap();

        let results_a = context.query_people((WHA, (1u8, 2u8, 3u8)));
        assert_eq!(results_a.len(), 1);

        let results_b = context.query_people((AWH, (3u8, 1u8, 2u8)));
        assert_eq!(results_b.len(), 1);

        assert_eq!(results_a, results_b);

        println!("Results: {:?}", results_a);

        set_module_filter("ixa", LevelFilter::Info);
        set_log_level(LevelFilter::Off);
    }

    #[test]
    fn index_name() {
        let index = Index::<Age>::new();
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
        let value1 = 42;
        let value2 = 43;
        assert_ne!(
            <Age as PersonProperty>::hash_property_value(&value1),
            <Age as PersonProperty>::hash_property_value(&value2)
        );
    }
}
