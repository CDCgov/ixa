use super::methods::Methods;
use crate::warn;
use crate::{Context, ContextPeopleExt, PersonId, PersonProperty};
use crate::{HashMap, HashSet};
use bincode::serialize;
use std::cell::Ref;

use serde::Serialize;
use tokio::time::Instant;

use std::collections::hash_map::Entry;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

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
}

pub struct IndexProfileData {
    pub access_count: usize,
    pub lookup_time: f64,
    pub indexing_time: f64,
    pub hits: usize,
    pub misses: usize,
}

// An index for a single property.
pub struct Index {
    // Primarily for debugging purposes
    #[allow(dead_code)]
    pub(super) name: &'static str,

    // The hash of the property value maps to a list of PersonIds or None if we're not indexing.
    // Note: Unfortunately, this must be pub until `Index::lookup_ref()` can be implemented; for
    //       use in `ContextPeopleExtInternal::query_people_internal()`.
    lookup: Option<HashMap<IndexValue, (String, HashSet<PersonId>)>>,

    // The largest person ID that has been indexed. Used so that we
    // can lazily index when a person is added.
    pub(super) max_indexed: usize,

    // Profiling data
    // ToDo: Put this inside a `RefCell<IndexProfileData>` (and get rid of atomics).
    pub(self) access_count: AtomicUsize,
    pub(self) lookup_time: Duration,
    pub(self) indexing_time: Duration,
    pub(self) hits: AtomicUsize,
    pub(self) misses: AtomicUsize,
}

impl Index {
    pub(super) fn new<T: PersonProperty + 'static>(_context: &Context, _property: T) -> Self {
        Self {
            name: std::any::type_name::<T>(),
            lookup: None,
            max_indexed: 0,
            access_count: AtomicUsize::default(),
            lookup_time: Duration::ZERO,
            indexing_time: Duration::ZERO,
            hits: AtomicUsize::default(),
            misses: AtomicUsize::default(),
        }
    }

    pub(super) fn get_profile_data(&self) -> IndexProfileData {
        IndexProfileData {
            access_count: self.access_count.load(Ordering::Relaxed),
            lookup_time: self.lookup_time.as_secs_f64(),
            indexing_time: self.indexing_time.as_secs_f64(),
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }

    pub(super) fn lookup(&self, index_value: IndexValue) -> Option<&HashSet<PersonId>> {
        self.access_count.fetch_add(1, Ordering::Relaxed);

        if let Some(lookup) = &self.lookup {
            if let Some(&(ref _name, ref set)) = lookup.get(&index_value) {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Some(set);
            }
        }
        self.misses.fetch_add(1, Ordering::Relaxed);

        None
    }

    // ToDo: Handle the case that this property isn't indexing.
    pub(super) fn add_person(&mut self, context: &Context, methods: &Methods, person_id: PersonId) {
        let start_time = Instant::now();
        let hash = (methods.indexer)(context, person_id);
        self.access_count.fetch_add(1, Ordering::Relaxed);

        let result = self.lookup.as_mut().unwrap().entry(hash);
        match result {
            Entry::Occupied(mut entry) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                entry.get_mut().1.insert(person_id);
            }
            Entry::Vacant(entry) => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                let mut hashset = HashSet::default();
                hashset.insert(person_id);
                entry.insert(((methods.get_display)(context, person_id), hashset));
            }
        }

        self.lookup_time = start_time.elapsed();
    }

    pub(super) fn remove_person(
        &mut self,
        context: &Context,
        methods: &Methods,
        person_id: PersonId,
    ) {
        let start_time = Instant::now();
        self.access_count.fetch_add(1, Ordering::Relaxed);

        let hash = (methods.indexer)(context, person_id);
        if let Some(entry) = self.lookup.as_mut().unwrap().get_mut(&hash) {
            entry.1.remove(&person_id);
            // Clean up the entry if there are no people
            if entry.0.is_empty() {
                self.lookup.as_mut().unwrap().remove(&hash);
            }
        } else {
            warn!("Tried to remove person_id from index that doesn't contain it");
        }

        self.lookup_time = start_time.elapsed();
    }

    /// If this indexer is indexing, index the people that have been added to the population
    /// since last index.
    pub(super) fn index_unindexed_people(&mut self, context: &Context, methods: &Methods) {
        let start_duration = Instant::now();
        self.access_count.fetch_add(1, Ordering::Relaxed);

        // Note that this is essentially the same as `add_person` but avoids repeated method calls
        // and allows us to count this as a single `account_access`.
        if let Some(lookup) = &mut self.lookup {
            let current_pop = context.get_current_population();
            for id in self.max_indexed..current_pop {
                let person_id = PersonId(id);
                let hash = (methods.indexer)(context, person_id);
                match lookup.entry(hash) {
                    Entry::Occupied(mut entry) => {
                        self.hits.fetch_add(1, Ordering::Relaxed);
                        entry.get_mut().1.insert(person_id);
                    }
                    Entry::Vacant(entry) => {
                        self.misses.fetch_add(1, Ordering::Relaxed);
                        let mut hashset = HashSet::default();
                        hashset.insert(person_id);
                        entry.insert(((methods.get_display)(context, person_id), hashset));
                    }
                }
            }
            self.max_indexed = current_pop;
        }

        self.indexing_time += start_duration.elapsed();
    }

    /// Enable indexing for this property
    pub(super) fn enable_indexing(&mut self) {
        if !self.is_indexing_enabled() {
            self.lookup = Some(HashMap::default());
        }
    }

    /// Whether this property is being indexed.
    pub(super) fn is_indexing_enabled(&self) -> bool {
        self.lookup.is_some()
    }
}

// This method cannot be implemented on `Index` until RFC 3519: arbitrary_self_types
// is stabilized. See: https://github.com/rust-lang/rust/issues/44874.
// ToDo: This architecture is incompatible with updating hits, misses, and time stats in safe Rust.
//       One way around this is to not hold onto references to each `HashSet` and compute
//       intersections as the results are encountered.That is likely to have a large performance
//       impact unless there is a way to intelligently order the processing of the query's
//       properties. (There is.) Another is to change the architecture: move intersections into
//       `Index`, move stats out of `Index`, etc.
pub(super) fn lookup_ref(
    this: Ref<Index>,
    index_value: IndexValue,
) -> Result<Option<Ref<HashSet<PersonId>>>, ()> {
    this.access_count.fetch_add(1, Ordering::Relaxed);

    if let Ok(lookup) = Ref::filter_map(this, |x| x.lookup.as_ref()) {
        return match Ref::filter_map(lookup, |x| x.get(&index_value).map(|entry| &entry.1)) {
            Ok(matching_people) => {
                // The property is indexing, and there are people with that value
                // this.hits.fetch_add(1, Ordering::Relaxed);
                Ok(Some(matching_people))
            }
            Err(..) => {
                // The property is indexing, but there are no people with that value
                Ok(None)
            }
        };
    }
    // The property is not indexing
    // this.misses.fetch_add(1, Ordering::Relaxed);
    Err(())
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
