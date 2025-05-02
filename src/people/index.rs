use super::methods::Methods;
use crate::people::external_api::ContextPeopleExtCrate;
use crate::{Context, ContextPeopleExt, PersonId, PersonProperty};
use crate::{HashMap, HashMapExt, HashSet, HashSetExt};
use bincode::serialize;
use serde::ser::{Serialize, Serializer};
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
// The lookup key for entries in the index. This is a serialized version of the value.
// If that serialization fits in 128 bits, we store it in `IndexValue::Fixed`
// Otherwise, we use a 64 bit hash.
#[doc(hidden)]
pub enum IndexValue {
    Fixed(u128),
    Hashed(u64),
}

impl Serialize for IndexValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            IndexValue::Fixed(value) => serializer.serialize_u128(*value),
            IndexValue::Hashed(value) => serializer.serialize_u64(*value),
        }
    }
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
            // Otherwise, hash the data and store it as `IndexValue::Hashed`
            let mut hasher = DefaultHasher::new();
            serialized_data.hash(&mut hasher);
            let hash = hasher.finish(); // Produces a 64-bit hash
            IndexValue::Hashed(hash)
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
    pub(super) fn new<T: PersonProperty + 'static>(_context: &Context, _property: T) -> Self {
        Self {
            name: std::any::type_name::<T>(),
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
            .or_insert_with(|| ((methods.get_display)(context, person_id), HashSet::new()))
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

/// The common callback used by multiple `Context` methods for future events
type IndexCallback = dyn Fn(&Context) + Send + Sync;

pub struct MultiIndex {
    register: Box<IndexCallback>,
    type_id: TypeId,
}

// / A static map of multi-property indices. This is used to register multi-property property indices
// / when they are first created. The map is keyed by the property IDs of the properties that are
// / indexed. The values are the `MultiIndex` objects that contain the registration function and
// / the type ID of the index.
#[doc(hidden)]
#[allow(clippy::type_complexity)]
pub static MULTI_PROPERTY_INDEX_MAP: LazyLock<
    Mutex<RefCell<HashMap<Vec<TypeId>, Arc<MultiIndex>>>>,
> = LazyLock::new(|| Mutex::new(RefCell::new(HashMap::new())));

#[allow(dead_code)]
pub fn add_multi_property_index<T: PersonProperty + 'static>(
    property_ids: &[TypeId],
    index_type: TypeId,
) {
    let current_map = MULTI_PROPERTY_INDEX_MAP.lock().unwrap();
    let mut map = current_map.borrow_mut();
    let mut ordered_property_ids = property_ids.to_owned();
    ordered_property_ids.sort();
    map.entry(ordered_property_ids)
        .or_insert(Arc::new(MultiIndex {
            register: Box::new(|context| {
                context.register_property::<T>();
                context.index_property_by_id(TypeId::of::<T>());
            }),
            type_id: index_type,
        }));
}

pub fn get_and_register_multi_property_index(
    query: &[(TypeId, IndexValue)],
    context: &Context,
) -> Option<TypeId> {
    let map = MULTI_PROPERTY_INDEX_MAP.lock().unwrap();
    let map = map.borrow();
    let mut sorted_query = query.to_owned();
    sorted_query.sort_by(|a, b| a.0.cmp(&b.0));
    let items = query.iter().map(|(t, _)| *t).collect::<Vec<_>>();
    if let Some(multi_index) = map.get(&items) {
        (multi_index.register)(context);
        return Some(multi_index.type_id);
    }
    None
}

pub fn get_multi_property_hash(query: &[(TypeId, IndexValue)]) -> IndexValue {
    let mut sorted_query = query.to_owned();
    sorted_query.sort_by(|a, b| a.0.cmp(&b.0));
    let items = query.iter().map(|(_, i)| *i).collect::<Vec<_>>();
    IndexValue::compute(&IndexValue::compute(&items))
}

pub fn process_indices(
    context: &Context,
    remaining_indices: &[&Index],
    property_names: &mut Vec<String>,
    current_matches: &HashSet<PersonId>,
    print_fn: &dyn Fn(&Context, &[String], usize),
) {
    if remaining_indices.is_empty() {
        print_fn(context, property_names, current_matches.len());
        return;
    }

    let (next_index, rest_indices) = remaining_indices.split_first().unwrap();
    let lookup = next_index.lookup.as_ref().unwrap();

    // If there is nothing in the index, we don't need to process it
    if lookup.is_empty() {
        return;
    }

    for (display, people) in lookup.values() {
        let intersect = !property_names.is_empty();
        property_names.push(display.clone());

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

    use crate::people::index::{Index, IndexValue};
    use crate::{define_person_property, Context, ContextPeopleExt};
    use std::any::TypeId;

    define_person_property!(Age, u8);

    #[test]
    fn index_name() {
        let context = Context::new();
        let index = Index::new(&context, Age);
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
        assert!(matches!(index, IndexValue::Hashed(_)));
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

    #[test]
    fn test_multi_property_index_map_add_get_and_hash() {
        let mut context = Context::new();
        let _person = context.add_person((Age, 64)).unwrap();
        let property_ids = vec![TypeId::of::<Age>()];
        let index_type = TypeId::of::<IndexValue>();
        super::add_multi_property_index::<Age>(&property_ids, index_type);
        let query = vec![(TypeId::of::<Age>(), IndexValue::Fixed(42))];
        let _ = super::get_multi_property_hash(&query);
        let registered_index = super::get_and_register_multi_property_index(&query, &context);
        assert!(registered_index.is_some());
    }
}
