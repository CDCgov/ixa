/*!

An `EntityMap<E, V>` is a map from `EntityId<E>` to values of type `V` with a hash-map-like API
optimized for densely populated maps.

An `EntityMap` is "dense" in the sense that it uses a vector `Vec<Option<V>>` internally for
storage, using the `EntityId<E>` to index into the vector. Use an `EntityMap` in any of the
following cases:

 - The number of entities is small
 - Most entities are expected to be used as a key
 - Access or creation speed is important
 - You want access to the `EntityId<E>` key a value was stored with

If you know beforehand how many entities you expect to store, use the `EntityMap::with_capacity`
constructor or `EntityMap::reserve` to preallocate the map, as that is more efficient than letting
it lazily reallocate as needed.

Because every value added to an `EntityMap` is accompanied by a valid `EntityId<E>`, `EntityMap` is
guaranteed to only "store" valid entity IDs. You can therefore use it as a replacement for
`EntityVec<E, V>` (or just `Vec<V>`) for cases where you need to recover the original entity ID that
a value was stored with, for example, by iterating over the (entity ID, value) pairs returned by
`EntityMap::iter`. The only cost you pay for this is the extra memory needed to store `Option<V>`
values instead of `V` values, which in some cases is nothing. The `EntityId<E>` itself is not
stored.

An `EntityMap<E, V>` can be cheaply converted to an `EntityVec<E, Option<V>>` using the
`EntityMap::into_entity_vec` method.

## Example

Imagine you have `Person` and `Setting` entities, and you want an efficient way to store for each
`SettingId` a `Vec<PersonId>` representing all the people that can be found in the setting. It is
possible to use a `HashMap<SettingId, Vec<PersonId>>` to store this information, but an
`EntityMap<SettingId, Vec<PersonId>>` is more efficient.

```rust,ignore
use ixa::data_structures::entity_map::EntityMap;

let mut setting_membership = EntityMap::<SettingId, Vec<PersonId>>::new();

// During population initialization you might initialize the map with data in, say, a `PersonRecord`
// struct that has a `home_id` field of type `SettingId`.
let person_id = context.add_entity(with!(Person, person_record.age));
let setting_members = setting_membership.get_or_insert(person_record.home_id, Vec::new);
setting_members.push(person_id);

// Look-ups are extremely efficient.
if let Some(setting_members) = setting_membership.get(setting_id){
    // Do something with the setting members.
}

// You can also iterate over the (entity ID, value) pairs.
for (setting_id, setting_members) in setting_membership.iter() {
    // Do something with the setting and its members.
}
```

*/

use std::fmt::{self, Debug};
use std::iter::FusedIterator;
use std::marker::PhantomData;

use crate::data_structures::entity_vec::EntityVec;
use crate::entity::{Entity, EntityId};

/// A `Vec`-backed map keyed by `EntityId<E>`.
#[derive(Clone, PartialEq, Eq)]
pub struct EntityMap<E: Entity, V> {
    data: Vec<Option<V>>,
    len: usize,
    _phantom: PhantomData<E>,
}

impl<E: Entity, V> EntityMap<E, V> {
    /// Creates an empty `EntityMap`.
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            len: 0,
            _phantom: PhantomData,
        }
    }

    /// Creates an empty `EntityMap` with space for at least `capacity` values.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            len: 0,
            _phantom: PhantomData,
        }
    }

    /// Cheap conversion to an `EntityVec<E, Option<V>>`
    pub fn into_entity_vec(self) -> EntityVec<E, Option<V>> {
        self.data.into()
    }

    /// Returns the number of stored key-value pairs.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns the capacity of the backing storage.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    /// Returns `true` if the map contains no values.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Reserves capacity for at least `additional` more values.
    pub fn reserve(&mut self, additional: usize) {
        self.data.reserve(additional);
    }

    /// Shrinks the backing vector to fit the highest occupied entity ID.
    pub fn shrink_to_fit(&mut self) {
        while self.data.last().is_some_and(Option::is_none) {
            self.data.pop();
        }
        self.data.shrink_to_fit();
    }

    /// Returns `true` if `entity_id` is present in the map.
    pub fn contains_key(&self, entity_id: EntityId<E>) -> bool {
        self.get(entity_id).is_some()
    }

    /// Returns the value for `entity_id`, or `None` if not present.
    pub fn get(&self, entity_id: EntityId<E>) -> Option<&V> {
        self.data.get(entity_id.0).and_then(Option::as_ref)
    }

    /// Returns the value for `entity_id` mutably, or `None` if not present.
    pub fn get_mut(&mut self, entity_id: EntityId<E>) -> Option<&mut V> {
        self.data.get_mut(entity_id.0).and_then(Option::as_mut)
    }

    /// Inserts `value` for `entity_id`, returning the previous value if one existed.
    pub fn insert(&mut self, entity_id: EntityId<E>, value: V) -> Option<V> {
        if entity_id.0 >= self.data.len() {
            self.data.resize_with(entity_id.0 + 1, || None);
        }

        let slot = &mut self.data[entity_id.0];
        let previous = slot.replace(value);
        if previous.is_none() {
            self.len += 1;
        }
        previous
    }

    /// Returns the value for `entity_id`, inserting `value` if it is not already present.
    pub fn get_or_insert(&mut self, entity_id: EntityId<E>, value: V) -> &mut V {
        self.get_or_insert_with(entity_id, || value)
    }

    /// Returns the value for `entity_id`, inserting a value from `f` if it is not already present.
    pub fn get_or_insert_with<F>(&mut self, entity_id: EntityId<E>, f: F) -> &mut V
    where
        F: FnOnce() -> V,
    {
        if entity_id.0 >= self.data.len() {
            self.data.resize_with(entity_id.0 + 1, || None);
        }

        let slot = &mut self.data[entity_id.0];
        if slot.is_none() {
            *slot = Some(f());
            self.len += 1;
        }

        slot.as_mut().unwrap()
    }

    /// Removes and returns the value for `entity_id`, if present.
    pub fn remove(&mut self, entity_id: EntityId<E>) -> Option<V> {
        let removed = self.data.get_mut(entity_id.0).and_then(Option::take);
        if removed.is_some() {
            self.len -= 1;
        }
        removed
    }

    /// Clears the map, removing all key-value pairs.
    pub fn clear(&mut self) {
        self.data.clear();
        self.len = 0;
    }

    /// Returns an iterator over `(EntityId<E>, &V)` pairs.
    pub fn iter(&self) -> Iter<'_, E, V> {
        Iter {
            inner: self.data.iter().enumerate(),
            remaining: self.len,
            _phantom: PhantomData,
        }
    }
}

impl<E: Entity, V> Default for EntityMap<E, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Entity, V: Debug> Debug for EntityMap<E, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<E: Entity, V> Extend<(EntityId<E>, V)> for EntityMap<E, V> {
    fn extend<I: IntoIterator<Item = (EntityId<E>, V)>>(&mut self, iter: I) {
        for (entity_id, value) in iter {
            let _ = self.insert(entity_id, value);
        }
    }
}

impl<E: Entity, V> FromIterator<(EntityId<E>, V)> for EntityMap<E, V> {
    fn from_iter<I: IntoIterator<Item = (EntityId<E>, V)>>(iter: I) -> Self {
        let mut map = Self::new();
        map.extend(iter);
        map
    }
}

impl<'a, E: Entity, V> IntoIterator for &'a EntityMap<E, V> {
    type Item = (EntityId<E>, &'a V);
    type IntoIter = Iter<'a, E, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over `(EntityId<E>, &V)` pairs from an `EntityMap<E, V>`.
pub struct Iter<'a, E: Entity, V> {
    inner: std::iter::Enumerate<std::slice::Iter<'a, Option<V>>>,
    remaining: usize,
    _phantom: PhantomData<E>,
}

impl<'a, E: Entity, V> Iterator for Iter<'a, E, V> {
    type Item = (EntityId<E>, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        for (index, value) in self.inner.by_ref() {
            if let Some(value) = value.as_ref() {
                self.remaining -= 1;
                return Some((EntityId::new(index), value));
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }

    fn count(self) -> usize {
        self.remaining
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if n >= self.remaining {
            self.remaining = 0;
            self.inner.by_ref().for_each(drop);
            return None;
        }

        let mut skipped = 0;
        for (index, value) in self.inner.by_ref() {
            if let Some(value) = value.as_ref() {
                if skipped == n {
                    self.remaining -= n + 1;
                    return Some((EntityId::new(index), value));
                }
                skipped += 1;
            }
        }

        self.remaining = 0;
        None
    }
}

impl<'a, E: Entity, V> ExactSizeIterator for Iter<'a, E, V> {
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<'a, E: Entity, V> FusedIterator for Iter<'a, E, V> {}

#[cfg(test)]
mod tests {
    use super::EntityMap;
    use crate::define_entity;
    use crate::entity::EntityId;

    define_entity!(TestEntity);

    #[test]
    fn new_is_empty() {
        let map = EntityMap::<TestEntity, i32>::new();

        assert_eq!(map.len(), 0);
        assert!(map.is_empty());
        assert_eq!(map.capacity(), 0);
    }

    #[test]
    fn with_capacity_sets_initial_capacity() {
        let map = EntityMap::<TestEntity, i32>::with_capacity(8);

        assert_eq!(map.len(), 0);
        assert!(map.capacity() >= 8);
    }

    #[test]
    fn insert_and_get_work_for_sparse_ids() {
        let mut map = EntityMap::<TestEntity, &'static str>::new();
        let id2 = EntityId::new(2);
        let id5 = EntityId::new(5);

        assert_eq!(map.insert(id2, "two"), None);
        assert_eq!(map.insert(id5, "five"), None);

        assert_eq!(map.len(), 2);
        assert!(!map.is_empty());
        assert_eq!(map.get(EntityId::new(0)), None);
        assert_eq!(map.get(id2), Some(&"two"));
        assert_eq!(map.get(id5), Some(&"five"));
        assert_eq!(map.get(EntityId::new(6)), None);
    }

    #[test]
    fn insert_replaces_existing_value_without_changing_len() {
        let mut map = EntityMap::<TestEntity, i32>::new();
        let id = EntityId::new(3);

        assert_eq!(map.insert(id, 10), None);
        assert_eq!(map.insert(id, 20), Some(10));

        assert_eq!(map.len(), 1);
        assert_eq!(map.get(id), Some(&20));
    }

    #[test]
    fn get_mut_updates_value() {
        let mut map = EntityMap::<TestEntity, i32>::new();
        let id = EntityId::new(1);
        let _ = map.insert(id, 10);

        *map.get_mut(id).unwrap() = 99;

        assert_eq!(map.get(id), Some(&99));
        assert_eq!(map.get_mut(EntityId::new(7)), None);
    }

    #[test]
    fn get_or_insert_inserts_missing_value_and_returns_mutable_reference() {
        let mut map = EntityMap::<TestEntity, i32>::new();
        let id = EntityId::new(3);

        let value = map.get_or_insert(id, 10);
        *value = 15;

        assert_eq!(map.len(), 1);
        assert_eq!(map.get(id), Some(&15));
    }

    #[test]
    fn get_or_insert_does_not_replace_existing_value() {
        let mut map = EntityMap::<TestEntity, i32>::new();
        let id = EntityId::new(2);
        let _ = map.insert(id, 20);

        let value = map.get_or_insert(id, 99);

        assert_eq!(*value, 20);
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(id), Some(&20));
    }

    #[test]
    fn get_or_insert_with_only_evaluates_closure_for_missing_key() {
        let mut map = EntityMap::<TestEntity, i32>::new();
        let missing_id = EntityId::new(1);
        let existing_id = EntityId::new(4);
        let _ = map.insert(existing_id, 40);
        let mut calls = 0;

        let inserted = map.get_or_insert_with(missing_id, || {
            calls += 1;
            10
        });
        assert_eq!(*inserted, 10);

        let existing = map.get_or_insert_with(existing_id, || {
            calls += 1;
            99
        });
        assert_eq!(*existing, 40);

        assert_eq!(calls, 1);
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn contains_key_tracks_presence() {
        let mut map = EntityMap::<TestEntity, i32>::new();
        let id = EntityId::new(4);

        assert!(!map.contains_key(id));
        let _ = map.insert(id, 12);
        assert!(map.contains_key(id));
        assert!(!map.contains_key(EntityId::new(3)));
    }

    #[test]
    fn remove_returns_value_and_decrements_len() {
        let mut map = EntityMap::<TestEntity, i32>::new();
        let id1 = EntityId::new(1);
        let id4 = EntityId::new(4);
        let _ = map.insert(id1, 10);
        let _ = map.insert(id4, 40);

        assert_eq!(map.remove(id1), Some(10));
        assert_eq!(map.remove(id1), None);

        assert_eq!(map.len(), 1);
        assert_eq!(map.get(id1), None);
        assert_eq!(map.get(id4), Some(&40));
    }

    #[test]
    fn remove_missing_key_returns_none_without_changing_len() {
        let mut map = EntityMap::<TestEntity, i32>::new();
        let _ = map.insert(EntityId::new(1), 10);
        let _ = map.insert(EntityId::new(8), 80);

        assert_eq!(map.remove(EntityId::new(4)), None);

        assert_eq!(map.len(), 2);
        assert!(!map.is_empty());
        assert_eq!(map.get(EntityId::new(1)), Some(&10));
        assert_eq!(map.get(EntityId::new(8)), Some(&80));
    }

    #[test]
    fn clear_removes_all_entries() {
        let mut map = EntityMap::<TestEntity, i32>::new();
        let _ = map.insert(EntityId::new(0), 1);
        let _ = map.insert(EntityId::new(2), 2);

        map.clear();

        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
        assert_eq!(map.get(EntityId::new(0)), None);
        assert_eq!(map.get(EntityId::new(2)), None);
    }

    #[test]
    fn reserve_and_shrink_to_fit_manage_capacity() {
        let mut map = EntityMap::<TestEntity, i32>::new();
        map.reserve(16);
        assert!(map.capacity() >= 16);

        let _ = map.insert(EntityId::new(10), 10);
        let _ = map.insert(EntityId::new(20), 20);
        assert_eq!(map.remove(EntityId::new(20)), Some(20));

        map.shrink_to_fit();

        assert_eq!(map.len(), 1);
        assert_eq!(map.get(EntityId::new(10)), Some(&10));
        assert_eq!(map.get(EntityId::new(20)), None);
        assert!(map.capacity() >= 11);
    }

    #[test]
    fn iter_yields_only_present_entries_in_entity_id_order() {
        let mut map = EntityMap::<TestEntity, &'static str>::new();
        let _ = map.insert(EntityId::new(3), "three");
        let _ = map.insert(EntityId::new(0), "zero");
        let _ = map.insert(EntityId::new(5), "five");

        let items: Vec<_> = map.iter().collect();

        assert_eq!(
            items,
            vec![
                (EntityId::new(0), &"zero"),
                (EntityId::new(3), &"three"),
                (EntityId::new(5), &"five"),
            ]
        );
    }

    #[test]
    fn iter_size_hint_len_and_count_track_remaining_entries() {
        let mut map = EntityMap::<TestEntity, i32>::new();
        let _ = map.insert(EntityId::new(1), 10);
        let _ = map.insert(EntityId::new(4), 40);
        let _ = map.insert(EntityId::new(7), 70);

        let mut iter = map.iter();
        assert_eq!(iter.size_hint(), (3, Some(3)));
        assert_eq!(iter.len(), 3);

        assert_eq!(iter.next(), Some((EntityId::new(1), &10)));
        assert_eq!(iter.size_hint(), (2, Some(2)));
        assert_eq!(iter.len(), 2);
        assert_eq!(iter.count(), 2);
    }

    #[test]
    fn iter_nth_skips_missing_entries_and_updates_remaining_len() {
        let mut map = EntityMap::<TestEntity, i32>::new();
        let _ = map.insert(EntityId::new(2), 20);
        let _ = map.insert(EntityId::new(5), 50);
        let _ = map.insert(EntityId::new(8), 80);

        let mut iter = map.iter();
        assert_eq!(iter.nth(1), Some((EntityId::new(5), &50)));
        assert_eq!(iter.len(), 1);
        assert_eq!(iter.next(), Some((EntityId::new(8), &80)));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn iter_nth_past_end_returns_none_and_exhausts_iterator() {
        let mut map = EntityMap::<TestEntity, i32>::new();
        let _ = map.insert(EntityId::new(1), 10);
        let _ = map.insert(EntityId::new(3), 30);

        let mut iter = map.iter();
        assert_eq!(iter.nth(2), None);
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn debug_formats_like_a_map() {
        let mut map = EntityMap::<TestEntity, i32>::new();
        let _ = map.insert(EntityId::new(1), 10);
        let _ = map.insert(EntityId::new(4), 40);

        assert_eq!(
            format!("{:?}", map),
            "{TestEntityId(1): 10, TestEntityId(4): 40}"
        );
    }

    #[test]
    fn default_matches_new() {
        let map = EntityMap::<TestEntity, i32>::default();

        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn extend_and_from_iter_insert_all_entries() {
        let entries = vec![(EntityId::new(2), 20), (EntityId::new(5), 50)];

        let mut map = EntityMap::<TestEntity, i32>::new();
        map.extend(entries.clone());
        assert_eq!(map.get(EntityId::new(2)), Some(&20));
        assert_eq!(map.get(EntityId::new(5)), Some(&50));

        let collected = EntityMap::<TestEntity, i32>::from_iter(entries);
        assert_eq!(collected.get(EntityId::new(2)), Some(&20));
        assert_eq!(collected.get(EntityId::new(5)), Some(&50));
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn into_iterator_for_reference_matches_iter() {
        let mut map = EntityMap::<TestEntity, i32>::new();
        let _ = map.insert(EntityId::new(0), 10);
        let _ = map.insert(EntityId::new(2), 20);

        let from_iter: Vec<_> = map.iter().collect();
        let from_into_iter: Vec<_> = (&map).into_iter().collect();

        assert_eq!(from_iter, from_into_iter);
    }
}
