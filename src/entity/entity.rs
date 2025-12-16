/*!

An implementor of `Entity` is a type that names a collection of related properties in analogy to a table in a database. The properties are analogous to the columns in the table, and the `EntityId<E>` type is analogous to the primary key of the table, the row number.

Right now an `Entity` type is just a zero-sized marker type. The static data associated with the type isn't used yet.

*/

use std::any::{Any, TypeId};
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use super::entity_store::get_entity_metadata_static;

/// A type that can be named and used (copied, cloned) but not created outside of this crate.
/// In the `define_entity!` macro we define the alias `pub type MyEntityId = EntityId<MyEntity>`.
#[derive(Serialize, Deserialize)]
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
        Self(self.0, PhantomData)
    }
}

// Otherwise the compiler isn't smart enough to know `EntityId<E>` is always `Copy`
impl<E: Entity> Copy for EntityId<E> {}

impl<E: Entity> Debug for EntityId<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = format!("{}Id", E::name());
        f.debug_tuple(name.as_str()).field(&self.0).finish()
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
    // pub(crate)
    pub fn new(index: usize) -> Self {
        Self(index, PhantomData)
    }
}

/// All entities must implement this trait using the `define_entity!` macro.
pub trait Entity: Any + Default {
    fn name() -> &'static str
    where
        Self: Sized;

    fn type_id() -> TypeId
    where
        Self: Sized,
    {
        TypeId::of::<Self>()
    }

    /// Get a list of all properties this `Entity` has. This list is static, computed in with `ctor` magic.
    fn property_ids() -> &'static [TypeId]
    where
        Self: Sized,
    {
        let (property_ids, _) = unsafe { get_entity_metadata_static(<Self as Entity>::type_id()) };
        property_ids
    }

    /// Get a list of all properties of this `Entity` that _must_ be supplied when a new entity is created.
    fn required_property_ids() -> &'static [TypeId]
    where
        Self: Sized,
    {
        let (_, required_property_ids) =
            unsafe { get_entity_metadata_static(<Self as Entity>::type_id()) };
        required_property_ids
    }

    /// The index of this item in the owner, which is initialized globally per type
    /// upon first access. We explicitly initialize this in a `ctor` in order to know
    /// how many [`Entity`] types exist globally when we construct any `EntityStore`.
    fn index() -> usize
    where
        Self: Sized;

    /// Creates a new boxed instance of the item.
    fn new_boxed() -> Box<Self> {
        Box::new(Default::default())
    }

    /// Standard pattern for downcasting to concrete types.
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub type BxEntity = Box<dyn Entity>;
