use super::methods::Methods;
use crate::{Context, ContextPeopleExt, PersonId, PersonProperty};
use crate::{HashMap, HashMapExt, HashSet, HashSetExt};
use bincode::serialize;
use serde::Serialize;
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;
use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize)]
// The lookup key for entries in the index. This is a serialized version of the value.
// a hash is calculated and stored in a Fixed(u64).
#[doc(hidden)]
pub enum IndexValue {
    Fixed(u64),
}

impl IndexValue {
    pub fn compute<T: Serialize>(val: &T) -> IndexValue {
        // Serialize `val` to a `Vec<u8>` using `bincode`
        let serialized_data = serialize(val).expect("Failed to serialize value");

        let mut hasher = DefaultHasher::new();
        serialized_data.hash(&mut hasher);
        let hash = hasher.finish(); // Produces a 64-bit hash
        IndexValue::Fixed(hash)
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

// explain...
#[doc(hidden)]
#[allow(clippy::type_complexity)]
pub static MULTI_PROPERTY_INDEX_MAP: LazyLock<Mutex<RefCell<HashMap<Vec<TypeId>, TypeId>>>> =
    LazyLock::new(|| Mutex::new(RefCell::new(HashMap::new())));

#[allow(dead_code)]
pub fn add_multi_property_index(property_ids: &[TypeId], index_type: TypeId) {
    let current_map = MULTI_PROPERTY_INDEX_MAP.lock().unwrap();
    let mut map = current_map.borrow_mut();
    let mut ordered_property_ids = property_ids.to_owned();
    ordered_property_ids.sort();
    map.entry(ordered_property_ids).or_insert(index_type);
}

pub fn get_multi_property_index(query: &[(TypeId, IndexValue)]) -> Option<TypeId> {
    let map = MULTI_PROPERTY_INDEX_MAP.lock().unwrap();
    let map = map.borrow();
    let mut sorted_query = query.to_owned();
    sorted_query.sort_by(|a, b| a.0.cmp(&b.0));
    let items = query.iter().map(|(t, _)| *t).collect::<Vec<_>>();
    map.get(&items).copied()
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
    use crate::{define_person_property, Context};

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
        assert!(matches!(index, IndexValue::Fixed(_)));
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
