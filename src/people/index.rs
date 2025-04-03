use crate::people::methods::Methods;
use crate::{type_of, Context, ContextPeopleExt, PersonId, PersonProperty};
use crate::{HashMap, HashSet};
use bincode::serialize;
use serde::Serialize;
use std::any::TypeId;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
// The lookup key for entries in the index. This is a serialized version of the value.
// If that serialization fits in 128 bits, we store it in `IndexValue::Fixed` to
// avoid the allocation of the `Vec`. Otherwise, it goes in `IndexValue::Variable`.
#[doc(hidden)]
pub enum IndexValue {
    Fixed(u128),
    Variable(Vec<u8>),
}

impl IndexValue {
    pub fn compute<T: Serialize>(val: &T) -> IndexValue {
        // Serialize `val` to a `Vec<u8>` using `bincode`
        let serialized_data = serialize(val).expect("Failed to serialize value");

        // If serialized data fits within 16 bytes...
        if serialized_data.len() <= 16 {
            // ...store it as `IndexValue::Fixed`
            let mut tmp: [u8; 16] = [0; 16];
            tmp[..serialized_data.len()].copy_from_slice(&serialized_data[..]);

            IndexValue::Fixed(u128::from_le_bytes(tmp))
        } else {
            // Otherwise, store it as `IndexValue::Variable`
            IndexValue::Variable(serialized_data)
        }
    }

    pub fn to_le_bytes(&self) -> Vec<u8> {
        match self {
            IndexValue::Fixed(value) => Vec::from(value.to_le_bytes()),
            IndexValue::Variable(data) => data.clone(),
        }
    }
}

// An index for a single property.
pub struct Index {
    // Primarily for debugging purposes
    #[allow(dead_code)]
    pub(super) name: &'static str,

    // The hash of the property value maps to a list of PersonIds
    // or None if we're not indexing
    pub(super) lookup: Option<HashMap<IndexValue, (String, HashSet<PersonId>)>>,

    // The largest person ID that has been indexed. Used so that we
    // can lazily index when a person is added.
    pub(super) max_indexed: usize,
}

impl Index {
    pub(super) fn new<T: PersonProperty + 'static>(_property: T) -> Self {
        Self::with_type_name(std::any::type_name::<T>())
    }

    pub(super) fn with_type_name(name: &'static str) -> Self {
        Self {
            name,
            lookup: None,
            max_indexed: 0,
        }
    }

    pub(super) fn add_person(&mut self, context: &Context, methods: &Methods, person_id: PersonId) {
        let hash = (methods.indexer)(context, person_id);
        self.lookup
            .as_mut()
            .unwrap()
            .entry(hash)
            .or_insert_with(|| {
                (
                    (methods.get_display)(context, person_id),
                    HashSet::default(),
                )
            })
            .1
            .insert(person_id);
    }

    pub(super) fn remove_person(
        &mut self,
        context: &Context,
        methods: &Methods,
        person_id: PersonId,
    ) {
        let hash = (methods.indexer)(context, person_id);
        if let Some(entry) = self.lookup.as_mut().unwrap().get_mut(&hash) {
            entry.1.remove(&person_id);
            // Clean up the entry if there are no people
            if entry.0.is_empty() {
                self.lookup.as_mut().unwrap().remove(&hash);
            }
        }
    }

    pub(super) fn index_unindexed_people(&mut self, context: &Context, methods: &Methods) {
        if self.lookup.is_none() {
            return;
        }
        let current_pop = context.get_current_population();
        for id in self.max_indexed..current_pop {
            let person_id = PersonId(id);
            self.add_person(context, methods, person_id);
        }
        self.max_indexed = current_pop;
    }
}

pub struct IndexMap {
    map: HashMap<TypeId, Index>,
}

impl Default for IndexMap {
    fn default() -> Self {
        Self::new()
    }
}

impl IndexMap {
    pub fn new() -> IndexMap {
        IndexMap {
            map: HashMap::default(),
        }
    }

    /// Inserts an index if it doesn't already exist, and returns a mutable reference to the index.
    #[inline(always)]
    pub fn get_container_mut<T: PersonProperty + 'static>(&mut self) -> &mut Index {
        self.map
            .entry(type_of::<T>())
            .or_insert_with(|| Index::new(T::get_instance()))
    }

    #[inline(always)]
    pub fn get_container_by_id_mut(
        &mut self,
        type_id: TypeId,
        type_name: &'static str,
    ) -> &mut Index {
        self.map
            .entry(type_id)
            .or_insert_with(|| Index::with_type_name(type_name))
    }

    #[inline(always)]
    pub fn get_container_by_id_ref(&self, type_id: TypeId) -> Option<&Index> {
        self.map.get(&type_id)
    }

    #[inline(always)]
    pub fn get_container_ref<T: PersonProperty + 'static>(&self) -> Option<&Index> {
        self.map.get(&type_of::<T>())
    }

    #[inline(always)]
    pub fn contains_key(&self, type_of: &TypeId) -> bool {
        self.map.contains_key(type_of)
    }
}

#[cfg(test)]
mod test {
    // Tests in `src/people/query.rs` also exercise indexing code.

    use crate::define_person_property;
    use crate::people::index::Index;
    use crate::people::index::IndexValue;

    define_person_property!(Age, u8);

    #[test]
    fn index_name() {
        let index = Index::new(Age);
        assert!(index.name.contains("Age"));
    }

    #[test]
    fn test_index_value_hasher_finish2_short() {
        let value = 42;
        let index = IndexValue::compute(&value);
        assert!(matches!(index, IndexValue::Fixed(_)));
    }

    #[test]
    fn test_index_value_hasher_finish2_long() {
        let value = "this is a longer string that exceeds 16 bytes";
        let index = IndexValue::compute(&value);
        assert!(matches!(index, IndexValue::Variable(_)));
    }

    #[test]
    fn test_index_value_compute_same_values() {
        let value = "test value";
        let value2 = "test value";
        assert_eq!(IndexValue::compute(&value), IndexValue::compute(&value2));
    }

    #[test]
    fn test_index_value_compute_different_values() {
        let value1 = 42;
        let value2 = 43;
        assert_ne!(IndexValue::compute(&value1), IndexValue::compute(&value2));
    }
}
