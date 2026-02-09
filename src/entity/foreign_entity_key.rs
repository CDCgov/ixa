/*!

Foreign entity keys express parent-child relationships between entities.

A `ForeignEntityKey<Parent>` is a property on a child entity that stores an
`Option<EntityId<Parent>>`, analogous to a foreign key in a relational database.

Use the [`define_group!`] macro to set up the relationship:

```rust,ignore
define_entity!(Household);
define_entity!(Person);
define_group!(Household of Person);
```

Then use the context extension methods:

```rust,ignore
// Set parent
context.set_parent(Household, person_id, household_id);

// Get parent (returns Option<EntityId<Household>>)
let household = context.get_parent(Household, person_id);

// Get all children referencing this parent
let people: Vec<EntityId<Person>> = context.get_children(Person, household_id);
```

*/

use std::marker::PhantomData;

use super::context_extension::ContextEntitiesExt;
use crate::entity::property::PropertyDef;
use crate::entity::{Entity, EntityId};
use crate::Context;

/// A zero-sized marker type representing a foreign key from a child entity to a parent entity.
///
/// When used as a property on entity `C`, it stores `Option<EntityId<P>>` — the ID of
/// the parent entity, or `None` if no parent is set.
#[derive(Debug, PartialEq, Eq)]
pub struct ForeignEntityKey<Parent>(PhantomData<Parent>);

impl<Parent> Clone for ForeignEntityKey<Parent> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Parent> Copy for ForeignEntityKey<Parent> {}

impl<Parent> ForeignEntityKey<Parent> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<Parent> Default for ForeignEntityKey<Parent> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Parent> serde::Serialize for ForeignEntityKey<Parent> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_unit()
    }
}

impl<Parent: Entity> crate::entity::property::IsProperty for ForeignEntityKey<Parent> {
    type Value = Option<EntityId<Parent>>;
}

/// Context extension trait for working with foreign entity key relationships.
pub trait ContextForeignEntityKeyExt {
    /// Returns the parent entity ID for the given child, or `None` if no parent is set.
    ///
    /// ```rust,ignore
    /// let household: Option<EntityId<Household>> = context.get_parent(Household, person_id);
    /// ```
    fn get_parent<P: Entity, C: Entity>(
        &self,
        _parent: P,
        child_id: EntityId<C>,
    ) -> Option<EntityId<P>>
    where
        ForeignEntityKey<P>: PropertyDef<C, Value = Option<EntityId<P>>>;

    /// Sets the parent entity for the given child. Emits a `PropertyChangeEvent`.
    ///
    /// ```rust,ignore
    /// context.set_parent(Household, person_id, household_id);
    /// ```
    fn set_parent<P: Entity, C: Entity>(
        &mut self,
        _parent: P,
        child_id: EntityId<C>,
        parent_id: EntityId<P>,
    ) where
        ForeignEntityKey<P>: PropertyDef<C, Value = Option<EntityId<P>>>;

    /// Returns all child entity IDs that reference the given parent.
    ///
    /// This performs a linear scan over all child entities. For frequent lookups,
    /// consider indexing the property with `context.index_property::<C, ForeignEntityKey<P>>()`.
    ///
    /// ```rust,ignore
    /// let people: Vec<EntityId<Person>> = context.get_children(Person, household_id);
    /// ```
    fn get_children<C: Entity, P: Entity>(
        &self,
        _child: C,
        parent_id: EntityId<P>,
    ) -> Vec<EntityId<C>>
    where
        ForeignEntityKey<P>: PropertyDef<C, Value = Option<EntityId<P>>>;
}

impl ContextForeignEntityKeyExt for Context {
    fn get_parent<P: Entity, C: Entity>(
        &self,
        _parent: P,
        child_id: EntityId<C>,
    ) -> Option<EntityId<P>>
    where
        ForeignEntityKey<P>: PropertyDef<C, Value = Option<EntityId<P>>>,
    {
        self.get_property::<C, ForeignEntityKey<P>>(child_id)
    }

    fn set_parent<P: Entity, C: Entity>(
        &mut self,
        _parent: P,
        child_id: EntityId<C>,
        parent_id: EntityId<P>,
    ) where
        ForeignEntityKey<P>: PropertyDef<C, Value = Option<EntityId<P>>>,
    {
        self.set_property::<C, ForeignEntityKey<P>>(child_id, Some(parent_id));
    }

    fn get_children<C: Entity, P: Entity>(
        &self,
        _child: C,
        parent_id: EntityId<P>,
    ) -> Vec<EntityId<C>>
    where
        ForeignEntityKey<P>: PropertyDef<C, Value = Option<EntityId<P>>>,
    {
        self.get_entity_iterator::<C>()
            .filter(|&child_id| {
                self.get_property::<C, ForeignEntityKey<P>>(child_id) == Some(parent_id)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::events::PropertyChangeEvent;
    use crate::prelude::*;

    define_entity!(Household);
    define_entity!(Person);
    define_group!(Household of Person);

    #[test]
    fn test_default_is_none() {
        let mut context = Context::new();
        let person = context.add_entity(Person).unwrap();

        let parent = context.get_parent(Household, person);
        assert_eq!(parent, None);
    }

    #[test]
    fn test_set_and_get_parent() {
        let mut context = Context::new();
        let household = context.add_entity(Household).unwrap();
        let person = context.add_entity(Person).unwrap();

        context.set_parent(Household, person, household);
        let parent = context.get_parent(Household, person);
        assert_eq!(parent, Some(household));
    }

    #[test]
    fn test_get_children() {
        let mut context = Context::new();
        let h1 = context.add_entity(Household).unwrap();
        let h2 = context.add_entity(Household).unwrap();

        let p1 = context.add_entity(Person).unwrap();
        let p2 = context.add_entity(Person).unwrap();
        let p3 = context.add_entity(Person).unwrap();
        let _p4 = context.add_entity(Person).unwrap();

        context.set_parent(Household, p1, h1);
        context.set_parent(Household, p2, h1);
        context.set_parent(Household, p3, h2);
        // _p4 has no parent

        let h1_children = context.get_children(Person, h1);
        assert_eq!(h1_children.len(), 2);
        assert!(h1_children.contains(&p1));
        assert!(h1_children.contains(&p2));

        let h2_children = context.get_children(Person, h2);
        assert_eq!(h2_children.len(), 1);
        assert!(h2_children.contains(&p3));
    }

    #[test]
    fn test_multiple_children_same_parent() {
        let mut context = Context::new();
        let household = context.add_entity(Household).unwrap();

        let mut people = Vec::new();
        for _ in 0..5 {
            let p = context.add_entity(Person).unwrap();
            context.set_parent(Household, p, household);
            people.push(p);
        }

        let children = context.get_children(Person, household);
        assert_eq!(children.len(), 5);
        for p in &people {
            assert!(children.contains(p));
        }
    }

    #[test]
    fn test_property_change_event_on_set_parent() {
        let mut context = Context::new();
        let household = context.add_entity(Household).unwrap();
        let person = context.add_entity(Person).unwrap();

        define_data_plugin!(EventCount, usize, 0);

        context.subscribe_to_event::<PropertyChangeEvent<Person, ForeignEntityKey<Household>>>(
            move |context, event| {
                assert_eq!(event.previous, None);
                assert_eq!(event.current, Some(household));
                *context.get_data_mut(EventCount) += 1;
            },
        );

        context.set_parent(Household, person, household);
        context.execute();

        assert_eq!(*context.get_data(EventCount), 1);
    }
}
