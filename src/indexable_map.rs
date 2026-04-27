//! Vec-backed storage keyed by values that expose a stable numeric index.

use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

/// A value that can be used as an index into vector-backed storage.
pub trait Indexable: Copy {
    /// Returns this value's zero-based storage index.
    fn index(self) -> usize;
}

macro_rules! impl_index_via_from {
    ($($type:ty),* $(,)?) => {
        $(
            impl Indexable for $type {
                #[inline]
                fn index(self) -> usize {
                    usize::from(self)
                }
            }
        )*
    };
}

macro_rules! impl_index_via_try_from {
    ($($type:ty),* $(,)?) => {
        $(
            impl Indexable for $type {
                #[inline]
                fn index(self) -> usize {
                    usize::try_from(self)
                        .unwrap_or_else(|_| panic!("index value {self} does not fit in usize"))
                }
            }
        )*
    };
}

impl Indexable for usize {
    #[inline]
    fn index(self) -> usize {
        self
    }
}

impl_index_via_from!(u8, u16);
impl_index_via_try_from!(u32, u64, u128);

/// A map from an indexed key type to values, backed by a `Vec<Option<V>>`.
///
/// `IndexableMap` uses the key's numeric index as a direct offset into the
/// backing vector, which makes lookups O(1) without hashing. Missing keys
/// occupy an empty slot in the backing vector.
#[derive(Clone, PartialEq, Eq)]
pub struct IndexableMap<I: Indexable, V> {
    entries: Vec<Option<V>>,
    len: usize,
    _key: PhantomData<I>,
}

impl<I: Indexable, V> IndexableMap<I, V> {
    /// Creates an empty map.
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty map with capacity for at least `capacity` index slots.
    #[must_use]
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            len: 0,
            _key: PhantomData,
        }
    }

    /// Returns the number of occupied entries.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the map contains no values.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the number of allocated index slots.
    #[must_use]
    #[inline]
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }

    /// Returns the exclusive upper bound of indices represented by the backing vector.
    #[must_use]
    #[inline]
    pub fn index_bound(&self) -> usize {
        self.entries.len()
    }

    /// Reserves capacity for at least `additional` more index slots.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.entries.reserve(additional);
    }

    /// Returns `true` if the map contains a value for `key`.
    #[must_use]
    #[inline]
    pub fn contains_key(&self, key: I) -> bool {
        self.get(key).is_some()
    }

    /// Returns a shared reference to the value for `key`.
    #[must_use]
    #[inline]
    pub fn get(&self, key: I) -> Option<&V> {
        self.entries.get(key.index()).and_then(Option::as_ref)
    }

    /// Returns a mutable reference to the value for `key`.
    #[must_use]
    #[inline]
    pub fn get_mut(&mut self, key: I) -> Option<&mut V> {
        self.entries.get_mut(key.index()).and_then(Option::as_mut)
    }

    /// Inserts `value` for `key`, returning the previous value if one existed.
    #[inline]
    pub fn insert(&mut self, key: I, value: V) -> Option<V> {
        let index = key.index();
        let len = self.entries.len();

        if index == len {
            self.entries.push(Some(value));
            self.len += 1;
            return None;
        }

        if index > len {
            self.entries.resize_with(index, || None);
            self.entries.push(Some(value));
            self.len += 1;
            return None;
        }

        let slot = &mut self.entries[index];
        let previous = slot.replace(value);
        if previous.is_none() {
            self.len += 1;
        }
        previous
    }

    /// Returns the value for `key`, inserting the result of `default` if absent.
    #[inline]
    pub fn get_or_insert_with(&mut self, key: I, default: impl FnOnce() -> V) -> &mut V {
        let index = key.index();
        if index >= self.entries.len() {
            self.entries.resize_with(index + 1, || None);
        }
        let slot = &mut self.entries[index];
        if slot.is_none() {
            self.len += 1;
        }
        slot.get_or_insert_with(default)
    }

    /// Removes and returns the value for `key`.
    #[inline]
    pub fn remove(&mut self, key: I) -> Option<V> {
        let value = self.entries.get_mut(key.index())?.take();
        if value.is_some() {
            self.len -= 1;
        }
        value
    }

    /// Removes all entries. The allocated capacity is retained.
    #[inline]
    pub fn clear(&mut self) {
        self.entries.clear();
        self.len = 0;
    }

    /// Iterates over occupied entries as `(raw_index, &value)`.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &V)> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.as_ref().map(|value| (index, value)))
    }

    /// Iterates over occupied entries as `(raw_index, &mut value)`.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, &mut V)> {
        self.entries
            .iter_mut()
            .enumerate()
            .filter_map(|(index, entry)| entry.as_mut().map(|value| (index, value)))
    }

    /// Iterates over occupied raw indices.
    pub fn indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.is_some().then_some(index))
    }

    /// Iterates over values.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, value)| value)
    }

    /// Iterates mutably over values.
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.iter_mut().map(|(_, value)| value)
    }
}

impl<I: Indexable, V> Default for IndexableMap<I, V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            len: 0,
            _key: PhantomData,
        }
    }
}

impl<I: Indexable, V: Debug> Debug for IndexableMap<I, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<I: Indexable, V> FromIterator<(I, V)> for IndexableMap<I, V> {
    fn from_iter<T: IntoIterator<Item = (I, V)>>(iter: T) -> Self {
        let mut map = Self::new();
        map.extend(iter);
        map
    }
}

impl<I: Indexable, V> Extend<(I, V)> for IndexableMap<I, V> {
    fn extend<T: IntoIterator<Item = (I, V)>>(&mut self, iter: T) {
        for (key, value) in iter {
            self.insert(key, value);
        }
    }
}

impl<I: Indexable, V> Index<I> for IndexableMap<I, V> {
    type Output = V;

    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        self.get(index).expect("no entry found for index")
    }
}

impl<I: Indexable, V> IndexMut<I> for IndexableMap<I, V> {
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        self.get_mut(index).expect("no entry found for index")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use crate::define_entity;
    use crate::entity::{ContextEntitiesExt, EntityId};

    define_entity!(IndexMapPerson);

    #[test]
    fn stores_values_by_raw_index() {
        let mut map = IndexableMap::<usize, &str>::new();

        assert!(map.is_empty());
        assert_eq!(map.insert(2, "two"), None);

        assert_eq!(map.len(), 1);
        assert_eq!(map.index_bound(), 3);
        assert_eq!(map.get(0), None);
        assert_eq!(map.get(2), Some(&"two"));
        assert!(map.contains_key(2));
    }

    #[test]
    fn replaces_and_removes_values() {
        let mut map = IndexableMap::<u32, i32>::new();

        assert_eq!(map.insert(1, 10), None);
        assert_eq!(map.insert(1, 20), Some(10));
        assert_eq!(map.len(), 1);
        assert_eq!(map[1_u32], 20);

        map[1_u32] = 30;
        assert_eq!(map.remove(1), Some(30));
        assert_eq!(map.remove(1), None);
        assert!(map.is_empty());
    }

    #[test]
    fn insert_at_index_beyond_length_grows_backing_vec() {
        let mut map = IndexableMap::<usize, &str>::new();
        map.insert(5, "five");

        assert_eq!(map.len(), 1);
        assert_eq!(map.index_bound(), 6);
        assert_eq!(map.get(0), None);
        assert_eq!(map.get(4), None);
        assert_eq!(map.get(5), Some(&"five"));
    }

    #[test]
    fn inserts_default_lazily() {
        let mut map = IndexableMap::<usize, Vec<i32>>::new();

        map.get_or_insert_with(4, Vec::new).push(1);
        map.get_or_insert_with(4, Vec::new).push(2);

        assert_eq!(map.get(4), Some(&vec![1, 2]));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn get_mut_returns_mutable_reference_or_none() {
        let mut map = IndexableMap::<usize, i32>::new();
        map.insert(2, 10);

        *map.get_mut(2).unwrap() = 20;
        assert_eq!(map.get(2), Some(&20));

        assert!(map.get_mut(99).is_none());
    }

    #[test]
    #[should_panic(expected = "no entry found for index")]
    fn indexing_a_missing_key_panics() {
        let map = IndexableMap::<usize, i32>::new();
        let _ = map[0];
    }

    #[test]
    fn iterates_occupied_indices() {
        let mut map = IndexableMap::<usize, &str>::new();
        map.insert(3, "three");
        map.insert(1, "one");

        let entries = map.iter().collect::<Vec<_>>();
        assert_eq!(entries, vec![(1, &"one"), (3, &"three")]);
        assert_eq!(map.indices().collect::<Vec<_>>(), vec![1, 3]);
    }

    #[test]
    fn supports_various_integer_key_types() {
        let mut u8_map = IndexableMap::<u8, &str>::new();
        u8_map.insert(3, "three");
        assert_eq!(u8_map.get(3), Some(&"three"));

        let mut u64_map = IndexableMap::<u64, &str>::new();
        u64_map.insert(7, "seven");
        assert_eq!(u64_map.get(7), Some(&"seven"));
    }

    #[test]
    fn supports_entity_ids_as_keys() {
        let mut map = IndexableMap::<EntityId<IndexMapPerson>, &str>::new();
        let person_id = EntityId::new(2);

        map.insert(person_id, "value");

        assert_eq!(person_id.index(), 2);
        assert_eq!(map.get(person_id), Some(&"value"));
    }

    #[test]
    fn keys_real_entity_ids_from_context() {
        let mut context = Context::new();
        let alice = context.add_entity::<IndexMapPerson, _>(()).unwrap();
        let bob = context.add_entity::<IndexMapPerson, _>(()).unwrap();
        let carol = context.add_entity::<IndexMapPerson, _>(()).unwrap();

        let mut map = IndexableMap::<EntityId<IndexMapPerson>, &str>::new();
        map.insert(alice, "alice");
        map.insert(carol, "carol");

        assert_eq!(map.get(alice), Some(&"alice"));
        assert_eq!(map.get(bob), None);
        assert_eq!(map.get(carol), Some(&"carol"));
        assert_eq!(map.len(), 2);
        assert_eq!(
            map.indices().collect::<Vec<_>>(),
            vec![alice.index(), carol.index()]
        );
    }

    #[test]
    fn clear_resets_state() {
        let mut map = IndexableMap::<usize, i32>::new();
        map.insert(2, 20);
        map.insert(5, 50);

        map.clear();

        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
        assert_eq!(map.index_bound(), 0);
        assert_eq!(map.get(2), None);
    }

    #[test]
    fn iter_mut_and_values_mut_modify_values() {
        let mut map = IndexableMap::<usize, i32>::new();
        map.insert(0, 1);
        map.insert(2, 3);

        for (_, value) in map.iter_mut() {
            *value *= 10;
        }
        for value in map.values_mut() {
            *value += 1;
        }

        assert_eq!(map.values().copied().collect::<Vec<_>>(), vec![11, 31]);
    }

    #[test]
    fn collects_and_extends_from_iter() {
        let mut map: IndexableMap<usize, &str> = [(2, "two"), (0, "zero")].into_iter().collect();
        map.extend([(1, "one"), (0, "ZERO")]);

        let entries = map.iter().collect::<Vec<_>>();
        assert_eq!(entries, vec![(0, &"ZERO"), (1, &"one"), (2, &"two")]);
    }

    #[test]
    fn debug_shows_occupied_entries() {
        let mut map = IndexableMap::<usize, i32>::new();
        map.insert(1, 10);
        map.insert(3, 30);

        assert_eq!(format!("{map:?}"), "{1: 10, 3: 30}");
    }

    #[test]
    fn reserve_grows_capacity() {
        let mut map = IndexableMap::<usize, i32>::new();
        map.reserve(64);

        assert!(map.capacity() >= 64);
        assert_eq!(map.len(), 0);
    }
}
