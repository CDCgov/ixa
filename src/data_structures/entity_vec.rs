/*!

An `EntityVec<E: Entity, V>` is a vector of values of type `V` that can only be indexed by keys of
type `EntityId<E>`.

An `EntityVec<E: Entity, V>` is a thin wrapper around a `Vec<V>` that enforces type safety of the
indexing (key) values. The most common `Vec<V>` methods are implemented.

Importantly, while `EntityVec<E: Entity, V>` can be _indexed_ by an `EntityId<E>` value, it cannot
construct an `EntityId<E>` value itself, because it does not guarantee that its length does not
exceed the range of valid `EntityId<E>` values. This allows methods like `EntityVec::push` and
`EntityVec::extend` that extend the length of the vector to remain unconstrained. If you need to be
able to retrieve the `EntityId<E>` that a value is associated with (e.g. by iterating over
(entity ID, value) pairs), use an [`EntityMap`](super::entity_map::EntityMap) instead.

For a hash-map-like API, see [`EntityMap`](super::entity_map::EntityMap).

## Example

Imagine you have a `Person` entity, and you want an efficient way to store for each `PersonId` a
`Vec<Itinerary>` representing different itineraries associated with that person. You might
initialize `itineraries_by_person: EntityVec<Person, Vec<Itinerary>>` during population creation, by
subscribing to the `EntityCreationEvent<Person>` event, or as a separate iteration over an existing
population.

```rust,ignore
use ixa::data_structures::entity_vec::EntityVec;

let mut itineraries_by_person: EntityVec<Person, Vec<Itinerary>> = EntityVec::new();

// Populate in entity-id order. For example, this could be done while creating people
// or while iterating over an existing population.
for person_id in context.get_entity_iterator::<Person>() {
    let itinerary_list = compute_itineraries_for_person(person_id);
    itineraries_by_person.push(itinerary_list);
}

// Later, given an existing PersonId:
let person_id: PersonId = /* fetch the `PersonId` somehow. */;

if let Some(itineraries) = itineraries_by_person.get_mut(person_id) {
    itineraries.push(/* some Itinerary */);
}
```

*/

use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

use crate::entity::{Entity, EntityId};

/**
A `Vec`-backed collection indexed by `EntityId<E>`.
*/
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EntityVec<E: Entity, V> {
    data: Vec<V>,
    _phantom: PhantomData<E>,
}

impl<E: Entity, V> EntityVec<E, V> {
    /// Creates an empty `EntityVec`.
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            _phantom: PhantomData,
        }
    }

    /// Creates an empty `EntityVec` with space for at least `capacity` items.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            _phantom: PhantomData,
        }
    }

    /// Returns the number of values stored.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns the capacity of the backing vector.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    /// Returns `true` if no values are stored.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Reserves capacity for at least `additional` more values.
    pub fn reserve(&mut self, additional: usize) {
        self.data.reserve(additional);
    }

    /// Shrinks the backing vector to fit its length.
    pub fn shrink_to_fit(&mut self) {
        self.data.shrink_to_fit();
    }

    /// Appends `value` to the end of the vector.
    pub fn push(&mut self, value: V) {
        self.data.push(value);
    }

    /// Removes and returns the last value, or `None` if empty.
    pub fn pop(&mut self) -> Option<V> {
        self.data.pop()
    }

    /// Returns the value for `entity_id`, or `None` if this vector is not long enough.
    pub fn get(&self, entity_id: EntityId<E>) -> Option<&V> {
        self.data.get(entity_id.0)
    }

    /// Returns the mutable value for `entity_id`, or `None` if this vector is not long enough.
    pub fn get_mut(&mut self, entity_id: EntityId<E>) -> Option<&mut V> {
        self.data.get_mut(entity_id.0)
    }

    /// Returns the last value, or `None` if empty.
    pub fn last(&self) -> Option<&V> {
        self.data.last()
    }

    /// Returns the last value mutably, or `None` if empty.
    pub fn last_mut(&mut self) -> Option<&mut V> {
        self.data.last_mut()
    }

    /// Returns the backing slice.
    pub fn as_slice(&self) -> &[V] {
        &self.data
    }

    /// Returns the backing slice mutably.
    pub fn as_mut_slice(&mut self) -> &mut [V] {
        &mut self.data
    }

    /// Returns an iterator over the stored values.
    pub fn iter(&self) -> std::slice::Iter<'_, V> {
        self.data.iter()
    }

    /// Returns a mutable iterator over the stored values.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, V> {
        self.data.iter_mut()
    }

    /// Clears the vector, removing all values.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Truncates the vector to `len` items.
    pub fn truncate(&mut self, len: usize) {
        self.data.truncate(len);
    }

    /// Extends the vector with values from `iter`, assigning contiguous IDs to new items.
    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = V>,
    {
        self.data.extend(iter);
    }

    /// Resizes the vector to `new_len`, cloning `value` as needed.
    pub fn resize(&mut self, new_len: usize, value: V)
    where
        V: Clone,
    {
        self.data.resize(new_len, value);
    }

    /// Resizes the vector to `new_len`, generating values with `f` as needed.
    pub fn resize_with<F>(&mut self, new_len: usize, f: F)
    where
        F: FnMut() -> V,
    {
        self.data.resize_with(new_len, f);
    }

    /// Returns `true` if the vector contains `value`.
    pub fn contains(&self, value: &V) -> bool
    where
        V: PartialEq,
    {
        self.data.contains(value)
    }

    /// Consumes the `EntityVec` and returns the backing `Vec`.
    pub fn into_vec(self) -> Vec<V> {
        self.data
    }
}

impl<E: Entity, V> Default for EntityVec<E, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Entity, V: Debug> Debug for EntityVec<E, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.data.fmt(f)
    }
}

impl<E: Entity, V> From<Vec<V>> for EntityVec<E, V> {
    fn from(data: Vec<V>) -> Self {
        Self {
            data,
            _phantom: PhantomData,
        }
    }
}

impl<E: Entity, V> From<EntityVec<E, V>> for Vec<V> {
    fn from(value: EntityVec<E, V>) -> Self {
        value.data
    }
}

impl<E: Entity, V> FromIterator<V> for EntityVec<E, V> {
    fn from_iter<I: IntoIterator<Item = V>>(iter: I) -> Self {
        Self::from(Vec::from_iter(iter))
    }
}

impl<E: Entity, V> Extend<V> for EntityVec<E, V> {
    fn extend<I: IntoIterator<Item = V>>(&mut self, iter: I) {
        self.data.extend(iter);
    }
}

impl<E: Entity, V> Index<EntityId<E>> for EntityVec<E, V> {
    type Output = V;

    fn index(&self, index: EntityId<E>) -> &Self::Output {
        &self.data[index.0]
    }
}

impl<E: Entity, V> IndexMut<EntityId<E>> for EntityVec<E, V> {
    fn index_mut(&mut self, index: EntityId<E>) -> &mut Self::Output {
        &mut self.data[index.0]
    }
}

impl<E: Entity, V> IntoIterator for EntityVec<E, V> {
    type Item = V;
    type IntoIter = std::vec::IntoIter<V>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl<'a, E: Entity, V> IntoIterator for &'a EntityVec<E, V> {
    type Item = &'a V;
    type IntoIter = std::slice::Iter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, E: Entity, V> IntoIterator for &'a mut EntityVec<E, V> {
    type Item = &'a mut V;
    type IntoIter = std::slice::IterMut<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::EntityVec;
    use crate::define_entity;
    use crate::entity::EntityId;

    define_entity!(TestEntity);
    #[test]
    fn new_is_empty() {
        let vec = EntityVec::<TestEntity, i32>::new();
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());
        assert_eq!(vec.capacity(), 0);
    }

    #[test]
    fn with_capacity_sets_initial_capacity() {
        let vec = EntityVec::<TestEntity, i32>::with_capacity(8);
        assert_eq!(vec.len(), 0);
        assert!(vec.capacity() >= 8);
    }

    #[test]
    fn push_appends_values_in_order() {
        let mut vec = EntityVec::<TestEntity, &'static str>::new();
        vec.push("zero");
        vec.push("one");
        vec.push("two");

        assert_eq!(vec.len(), 3);
        assert_eq!(vec[EntityId::new(0)], "zero");
        assert_eq!(vec[EntityId::new(1)], "one");
        assert_eq!(vec[EntityId::new(2)], "two");
    }

    #[test]
    fn get_and_get_mut_are_bounds_checked() {
        let mut vec = EntityVec::<TestEntity, i32>::new();
        vec.push(10);
        vec.push(20);
        let id0 = EntityId::new(0);
        let id1 = EntityId::new(1);

        assert_eq!(vec.get(id0), Some(&10));
        assert_eq!(vec.get(id1), Some(&20));
        assert_eq!(vec.get(EntityId::new(2)), None);

        *vec.get_mut(id1).unwrap() = 99;
        assert_eq!(vec.get(id1), Some(&99));
        assert_eq!(vec.get_mut(EntityId::new(2)), None);
    }

    #[test]
    fn index_and_index_mut_use_entity_ids() {
        let mut vec = EntityVec::<TestEntity, i32>::new();
        vec.push(1);
        vec.push(2);
        let id0 = EntityId::new(0);
        let id1 = EntityId::new(1);

        vec[id1] = 7;

        assert_eq!(vec[id0], 1);
        assert_eq!(vec[id1], 7);
    }

    #[test]
    fn pop_last_last_mut_and_clear_work() {
        let mut vec = EntityVec::<TestEntity, String>::new();
        vec.push(String::from("a"));
        vec.push(String::from("b"));

        assert_eq!(vec.last().map(String::as_str), Some("b"));
        vec.last_mut().unwrap().push('!');
        assert_eq!(vec.last().map(String::as_str), Some("b!"));
        assert_eq!(vec.pop(), Some(String::from("b!")));
        assert_eq!(vec.pop(), Some(String::from("a")));
        assert_eq!(vec.pop(), None);

        vec.push(String::from("c"));
        vec.clear();
        assert!(vec.is_empty());
        assert_eq!(vec.last(), None);
        assert_eq!(vec.last_mut(), None);
    }

    #[test]
    fn reserve_and_shrink_to_fit_forward_to_backing_vec() {
        let mut vec = EntityVec::<TestEntity, i32>::new();
        vec.reserve(16);
        assert!(vec.capacity() >= 16);

        vec.extend(0..4);
        vec.shrink_to_fit();
        assert!(vec.capacity() >= vec.len());
    }

    #[test]
    fn as_slice_and_as_mut_slice_expose_backing_storage() {
        let mut vec = EntityVec::<TestEntity, i32>::from(vec![1, 2, 3]);
        assert_eq!(vec.as_slice(), &[1, 2, 3]);

        vec.as_mut_slice()[1] = 9;
        assert_eq!(vec.as_slice(), &[1, 9, 3]);
    }

    #[test]
    fn iter_and_iter_mut_visit_values_in_order() {
        let mut vec = EntityVec::<TestEntity, i32>::from(vec![1, 2, 3]);
        let values: Vec<_> = vec.iter().copied().collect();
        assert_eq!(values, vec![1, 2, 3]);

        for value in vec.iter_mut() {
            *value *= 2;
        }

        assert_eq!(vec.as_slice(), &[2, 4, 6]);
    }

    #[test]
    fn truncate_removes_trailing_items() {
        let mut vec = EntityVec::<TestEntity, i32>::from(vec![1, 2, 3, 4]);
        vec.truncate(2);

        assert_eq!(vec.len(), 2);
        assert_eq!(vec.get(EntityId::new(0)), Some(&1));
        assert_eq!(vec.get(EntityId::new(1)), Some(&2));
        assert_eq!(vec.get(EntityId::new(2)), None);
    }

    #[test]
    fn contains_checks_values() {
        let vec = EntityVec::<TestEntity, i32>::from(vec![3, 5, 8]);
        assert!(vec.contains(&5));
        assert!(!vec.contains(&13));
    }

    #[test]
    fn from_iter_and_inherent_extend_append_in_order() {
        let mut vec: EntityVec<TestEntity, i32> = [1, 2].into_iter().collect();
        EntityVec::extend(&mut vec, [3, 4]);

        assert_eq!(vec.as_slice(), &[1, 2, 3, 4]);
        vec.push(5);
        assert_eq!(vec[EntityId::new(4)], 5);
    }

    #[test]
    fn trait_extend_appends_values() {
        let mut vec = EntityVec::<TestEntity, i32>::new();
        <EntityVec<TestEntity, i32> as Extend<i32>>::extend(&mut vec, [7, 8, 9]);
        assert_eq!(vec.as_slice(), &[7, 8, 9]);
    }

    #[test]
    fn into_vec_and_from_vec_round_trip() {
        let vec = EntityVec::<TestEntity, i32>::from(vec![4, 5, 6]);
        let raw = vec.into_vec();
        assert_eq!(raw, vec![4, 5, 6]);

        let wrapped = EntityVec::<TestEntity, i32>::from(raw.clone());
        let round_trip: Vec<_> = wrapped.into();
        assert_eq!(round_trip, raw);
    }

    #[test]
    fn into_iterator_variants_match_backing_vec_order() {
        let mut vec = EntityVec::<TestEntity, i32>::from(vec![1, 2, 3]);

        let shared: Vec<_> = (&vec).into_iter().copied().collect();
        assert_eq!(shared, vec![1, 2, 3]);

        for value in &mut vec {
            *value += 10;
        }
        assert_eq!(vec.as_slice(), &[11, 12, 13]);

        let owned: Vec<_> = vec.into_iter().collect();
        assert_eq!(owned, vec![11, 12, 13]);
    }

    #[test]
    fn debug_delegates_to_backing_vec() {
        let vec = EntityVec::<TestEntity, i32>::from(vec![1, 2, 3]);
        assert_eq!(format!("{vec:?}"), "[1, 2, 3]");
    }

    #[test]
    fn clone_and_eq_compare_stored_values() {
        let vec = EntityVec::<TestEntity, i32>::from(vec![1, 2, 3]);
        let clone = vec.clone();

        assert_eq!(vec, clone);
        assert_ne!(vec, EntityVec::<TestEntity, i32>::from(vec![1, 2]));
    }

    #[test]
    fn resize_and_resize_with_extend_storage_by_index() {
        let mut vec = EntityVec::<TestEntity, i32>::new();
        vec.resize(3, 7);
        assert_eq!(vec.as_slice(), &[7, 7, 7]);

        let mut next = 10;
        vec.resize_with(5, || {
            let value = next;
            next += 1;
            value
        });

        assert_eq!(vec.as_slice(), &[7, 7, 7, 10, 11]);
    }
}
