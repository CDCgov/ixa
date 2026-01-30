/*!

An implementor of [`Entity`] is a type that names a collection of related properties in
analogy to a table in a database. The properties are analogous to the columns in the table,
and the [`EntityId<E>`] type is analogous to the primary key of the table, the row number.

[`Entity`]s are declared with the [`define_entity!`] macro:

```rust
use ixa::define_entity;
define_entity!(Person);
```

Once an [`Entity`] is defined, [`Property`]s can be defined for the [`Entity`]. See the
[`property`](crate::entity::property) module.

Right now an `Entity` type is just a zero-sized marker type. The static data associated with the type isn't used yet.

*/

use std::any::{Any, TypeId};
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use super::entity_store::get_entity_metadata_static;

/// A type that can be named and used (copied, cloned) but not created outside of this crate.
/// In the `define_entity!` macro we define the alias `pub type MyEntityId = EntityId<MyEntity>`.
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId<E: Entity>(pub(crate) usize, PhantomData<E>);
// Note: The generics on `EntityId<E>` prevent the compiler from "seeing" the derived traits in some client code,
//       so we provide blanket implementations below.

// Otherwise the compiler isn't smart enough to know `EntityId<E>` is always `PartialEq`/`Eq`
impl<E: Entity> PartialEq for EntityId<E> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<E: Entity> Eq for EntityId<E> {}

// Otherwise the compiler isn't smart enough to know `EntityId<E>` is always `Clone`
impl<E: Entity> Clone for EntityId<E> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

// Otherwise the compiler isn't smart enough to know `EntityId<E>` is always `Copy`
impl<E: Entity> Copy for EntityId<E> {}

// The value `EntityId<Person>(7, PhantomData<Person>)` has `Debug` display "PersonId(7)".
impl<E: Entity> Debug for EntityId<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = format!("{}Id", E::name());
        f.debug_tuple(name.as_str()).field(&self.0).finish()
    }
}
// The value `EntityId<Person>(7, PhantomData<Person>)` will display as "7".
impl<E: Entity> Display for EntityId<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Otherwise the compiler isn't smart enough to know `EntityId<E>` is always `Hash`
impl<E: Entity> Hash for EntityId<E> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<E: Entity> EntityId<E> {
    /// Only constructible from this crate.
    pub(crate) fn new(index: usize) -> Self {
        Self(index, PhantomData)
    }
}

/// All entities must implement this trait using the `define_entity!` macro.
pub trait Entity: Any + Default {
    fn name() -> &'static str {
        let full = std::any::type_name::<Self>();
        full.rsplit("::").next().unwrap()
    }

    fn type_id() -> TypeId {
        TypeId::of::<Self>()
    }

    /// Get a list of all properties this `Entity` has. This list is static, computed in with `ctor` magic.
    fn property_ids() -> &'static [TypeId] {
        let (property_ids, _) = get_entity_metadata_static(<Self as Entity>::type_id());
        property_ids
    }

    /// Get a list of all properties of this `Entity` that _must_ be supplied when a new entity is created.
    fn required_property_ids() -> &'static [TypeId] {
        let (_, required_property_ids) = get_entity_metadata_static(<Self as Entity>::type_id());
        required_property_ids
    }

    /// The index of this item in the owner, which is initialized globally per type
    /// upon first access. We explicitly initialize this in a `ctor` in order to know
    /// how many [`Entity`] types exist globally when we construct any `EntityStore`.
    fn id() -> usize;

    /// Creates a new boxed instance of the item.
    fn new_boxed() -> Box<Self> {
        Box::default()
    }

    /// Standard pattern for downcasting to concrete types.
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub type BxEntity = Box<dyn Entity>;

/// An iterator over the total population of `EntityId<E>`s at the time of iterator creation.
///
/// If entities are added _after_ this iterator has been created, this iterator will _not_ produce the `EntityId<E>`s
/// of the newly added entities.
#[derive(Copy, Clone)]
pub struct EntityIterator<E: Entity> {
    /// The total count of all entities of this type at the time this iterator was created
    population: usize,
    /// The next `EntityId<E>` this iterator will produce (assuming `entity_id < population`)
    entity_id: usize,

    _phantom: PhantomData<E>,
}

impl<E: Entity> EntityIterator<E> {
    // Only internal ixa code can create a new `EntityIterator<E>` in order to guarantee only valid
    // `EntityId<E>` values are ever created.
    pub(crate) fn new(population: usize) -> Self {
        EntityIterator::<E> {
            population,
            entity_id: 0,
            _phantom: PhantomData,
        }
    }
}

impl<E: Entity> Iterator for EntityIterator<E> {
    type Item = EntityId<E>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.entity_id < self.population {
            let current_id = self.entity_id;
            // `self.entity_id` saturates to `self.population`.
            self.entity_id += 1;
            Some(EntityId::new(current_id))
        } else {
            None
        }
    }

    // This iterator knows how many elements it will iterate over.
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len();
        (remaining, Some(remaining))
    }

    // Fast consuming count
    fn count(self) -> usize {
        self.len()
    }

    // Fast "seeking" operation.
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        // `self.entity_id` saturates to `self.population`.
        self.entity_id = (self.entity_id + n).min(self.population);
        self.next()
    }
}

impl<E: Entity> ExactSizeIterator for EntityIterator<E> {
    fn len(&self) -> usize {
        // Safety: Since `self.entity_id` saturates to `self.population`, we do not need `saturating_sub` here.
        self.population - self.entity_id
    }
}
// Once `EntityIterator<E>` returns `None`, it will always return `None`.
impl<E: Entity> std::iter::FusedIterator for EntityIterator<E> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::define_entity;

    define_entity!(DummyEntity);

    #[test]
    fn test_entity_iterator_basic() {
        let mut iter = EntityIterator::<DummyEntity>::new(3);

        assert_eq!(iter.len(), 3);
        assert_eq!(iter.next(), Some(EntityId::new(0)));
        assert_eq!(iter.len(), 2);
        assert_eq!(iter.next(), Some(EntityId::new(1)));
        assert_eq!(iter.len(), 1);
        assert_eq!(iter.next(), Some(EntityId::new(2)));
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None); // FusedIterator behavior
    }

    #[test]
    fn test_entity_iterator_nth() {
        let mut iter = EntityIterator::<DummyEntity>::new(10);

        // Seek to 3rd element (index 2)
        assert_eq!(iter.nth(2), Some(EntityId::new(2)));
        assert_eq!(iter.len(), 7);

        // Seek relative to current position (+1 means skip index 3, return 4)
        assert_eq!(iter.nth(1), Some(EntityId::new(4)));

        // Seek past end
        assert_eq!(iter.nth(10), None);
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_entity_iterator_size_hint() {
        let mut iter = EntityIterator::<DummyEntity>::new(5);
        assert_eq!(iter.size_hint(), (5, Some(5)));

        iter.next();
        assert_eq!(iter.size_hint(), (4, Some(4)));

        // Seek past end
        assert_eq!(iter.nth(10), None);
        assert_eq!(iter.size_hint(), (0, Some(0)));
    }

    #[test]
    fn test_entity_iterator_clonable() {
        let mut iter = EntityIterator::<DummyEntity>::new(5);
        iter.next();

        let mut cloned = iter;
        assert_eq!(iter.next(), cloned.next());
        assert_eq!(iter.size_hint(), cloned.size_hint());
    }
}
