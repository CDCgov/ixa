/*!

Each property type `P: PersonProperty` has a corresponding `Index<P>` type a single
instance of which is stored in the `PeopleData::property_indexes: RefCell<HashMap<TypeId,
BxIndex>>` map. The map `PeopleData::property_indexes` is keyed by the `TypeId` of the
property tag type `P`, _not_ the value type of the property, `PersonProperty::Value`.

`Index<P>::lookup: HashTable<(T, std::collections::HashSet<PersonId>)>` is a hash table,
_not_ a hash map. The difference is, a hash table is keyed by a `u64`, in our case,
the 64-bit hash of the property value, and allows for a custom equality check. For
equality, we compare the 128-bit hash of the property value. This allows us to keep the
lookup operation completely type-erased. The value associated with the key is a tuple
of the property value and a set of `PersonId`s. The property value is stored so that

1. we can compute its 128-bit hash to check equality during lookup,
2. we can iterate over (property value, set of `PersonId`s) pairs in the typed API, and
3. we can iterate over (serialized property value, set of `PersonId`s) pairs in the type-erased API.

*/

use crate::people::external_api::ContextPeopleExtCrate;
use crate::{
    hashing::one_shot_128, people::HashValueType, Context, ContextPeopleExt, HashMap, HashSet,
    PersonId, PersonProperty,
};
use hashbrown::HashTable;
use std::any::TypeId;
use std::cell::RefCell;
use std::sync::{Arc, LazyLock, Mutex};

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
    lookup: HashTable<(T::Value, HashSet<PersonId>)>,

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

    /// Inserts an entity into the set associated with `key`, creating a new set if one does not yet exist. Returns a
    /// `bool` according to whether the `entity_id` already existed in the set.
    pub fn add_person(&mut self, key: &T::Value, entity_id: PersonId) -> bool {
        let hash = T::hash_property_value(key);

        // > `hasher` is called if entries need to be moved or copied to a new table.
        // > This must return the same hash value that each entry was inserted with.
        #[allow(clippy::cast_possible_truncation)]
        let hasher = |(stored_value, _stored_set): &_| T::hash_property_value(stored_value) as u64;
        // Equality is determined by comparing the full 128-bit hashes. We do not expect any collisions before the heat
        // death of the universe.
        let hash128_equality = |(stored_value, _): &_| T::hash_property_value(stored_value) == hash;
        #[allow(clippy::cast_possible_truncation)]
        self.lookup
            .entry(hash as u64, hash128_equality, hasher)
            .or_insert_with(|| (*key, HashSet::default()))
            .get_mut()
            .1
            .insert(entity_id)
    }

    pub fn remove_person(&mut self, key: &T::Value, entity_id: PersonId) {
        let hash = T::hash_property_value(key);
        self.remove_person_with_hash(hash, entity_id);
    }
}

/// This trait Encapsulates the type-erased API.
pub trait TypeErasedIndex {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// Removing a person only requires the hash.
    fn remove_person_with_hash(&mut self, hash: HashValueType, entity_id: PersonId);

    /// Fetching a set only requires the hash.
    fn get_with_hash(&self, hash: HashValueType) -> Option<&HashSet<PersonId>>;

    // /// Fetching a set only requires the hash.
    // fn get_with_hash_mut(&mut self, hash: HashValueType) -> Option<&mut HashSet<PersonId>>;

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
        for id in self.max_indexed..current_pop {
            let person_id = PersonId(id);
            let value = context.get_person_property(person_id, T::get_instance());
            self.add_person(&value, person_id);
        }
        self.max_indexed = current_pop;
    }

    fn iter_serialized_values_people(
        &self,
    ) -> Box<dyn Iterator<Item = (String, &HashSet<PersonId>)> + '_> {
        Box::new(self.lookup.iter().map(|(k, v)| (T::get_display(k), v)))
    }
}

/// The common callback used by multiple `Context` methods for future events
type IndexCallback = dyn Fn(&Context) + Send + Sync;

pub struct MultiIndex {
    register: Box<IndexCallback>,
    type_id: TypeId,
}

/// A static map of multi-property indices. This is used to register multi-property
/// property indices when they are first created. The map is keyed by the property IDs of
/// the properties that are indexed. The values are the `MultiIndex` objects that contain
/// a function which registers and indexes the property and the type ID of the index.
#[doc(hidden)]
#[allow(clippy::type_complexity)]
pub static MULTI_PROPERTY_INDEX_MAP: LazyLock<
    Mutex<RefCell<HashMap<HashValueType, Arc<MultiIndex>>>>,
> = LazyLock::new(|| Mutex::new(RefCell::new(HashMap::default())));

/// Creates a record in `MULTI_PROPERTY_INDEX_MAP` if one doesn't already exist.
/// Called from the `define_multi_property_index!` macro. The registered `type_id`
/// for a multi-index is the `type_id` of the first multi-index having that set
/// of properties to register.
#[allow(dead_code)]
pub fn add_multi_property_index<T: PersonProperty>(
    property_type_ids: &mut [TypeId],
    index_type: TypeId,
) {
    let current_map = MULTI_PROPERTY_INDEX_MAP.lock().unwrap();
    let mut map = current_map.borrow_mut();
    let property_id_hash = get_multi_property_hash(property_type_ids);

    map.entry(property_id_hash).or_insert(Arc::new(MultiIndex {
        register: Box::new(|context| {
            context.register_property::<T>();
            context.index_property_by_id(TypeId::of::<T>());
        }),
        type_id: index_type,
    }));
}

pub fn get_and_register_multi_property_index(
    query: &[(TypeId, HashValueType)],
    context: &Context,
) -> Option<TypeId> {
    let map = MULTI_PROPERTY_INDEX_MAP.lock().unwrap();
    let map = map.borrow();
    let mut property_type_ids = query.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    let hash = get_multi_property_hash(&mut property_type_ids);

    if let Some(multi_index) = map.get(&hash) {
        (multi_index.register)(context);
        return Some(multi_index.type_id);
    }
    None
}

pub fn get_multi_property_value_hash(query: &[(TypeId, HashValueType)]) -> HashValueType {
    let mut items = query.iter().map(|(_, i)| *i).collect::<Vec<_>>();
    items.sort_unstable();
    one_shot_128(&items)
}

pub fn get_multi_property_hash(property_type_ids: &mut [TypeId]) -> HashValueType {
    property_type_ids.sort();
    one_shot_128(&property_type_ids)
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
    use crate::people::index::{HashValueType, Index};
    use crate::prelude::*;
    use crate::PersonProperty;
    use std::any::TypeId;

    define_person_property!(Age, u8);

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

    #[test]
    fn test_multi_property_index_map_add_get_and_hash() {
        let mut context = Context::new();
        let _person = context.add_person((Age, 64u8)).unwrap();
        let mut property_ids = vec![TypeId::of::<Age>()];
        let index_type = TypeId::of::<HashValueType>();
        super::add_multi_property_index::<Age>(&mut property_ids, index_type);
        let query = vec![(
            TypeId::of::<Age>(),
            <Age as PersonProperty>::hash_property_value(&42u8),
        )];
        let _ = super::get_multi_property_value_hash(&query);
        let registered_index = super::get_and_register_multi_property_index(&query, &context);
        assert!(registered_index.is_some());
    }
}
