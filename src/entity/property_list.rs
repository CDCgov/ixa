/*!

This module supports two user-facing patterns:

1. initializing a new entity with [`ContextEntitiesExt::add_entity`], and
2. specifying strata for value-change counting APIs such as
   [`ContextEntitiesExt::track_periodic_value_change_counts`].

For `add_entity`, pass either:

- the entity type directly, such as `Person`, to use default property values, or
- [`with!`](crate::with) to provide one or more initial property values, such as
  `with!(Person, Age(25), InfectionStatus::Infected)`.

For value-change counting APIs, use tuple types in the generic parameter list, such as
`(InfectionStatus,)` or `(AgeGroup, InfectionStatus)`.

In both cases, all properties must belong to the same entity, and property values must be distinct.

*/

use std::any::TypeId;

use seq_macro::seq;

use super::entity::{Entity, EntityId};
use super::property::Property;
use super::property_store::PropertyStore;
use crate::entity::ContextEntitiesExt;
use crate::{Context, IxaError};

pub trait PropertyList<E: Entity>: Copy + 'static {
    /// Validates that the properties are distinct. If not, returns an error describing the problematic properties.
    fn validate() -> Result<(), IxaError>;

    /// Checks that this property list includes all properties in the given list.
    fn contains_properties(property_type_ids: &[TypeId]) -> bool;

    /// Checks that this property list contains all required properties of the entity.
    fn contains_required_properties() -> bool {
        Self::contains_properties(E::required_property_ids())
    }

    /// Assigns the given entity the property values in `self` in the `property_store`.
    /// This method does NOT emit property change events, as it is called upon entity creation.
    fn set_values_for_new_entity(
        &self,
        entity_id: EntityId<E>,
        property_store: &mut PropertyStore<E>,
    );

    /// Gets the tuple of property values for the given entity.
    fn get_values_for_entity(context: &Context, entity_id: EntityId<E>) -> Self;
}

/// Values accepted by [`ContextEntitiesExt::add_entity`].
pub trait PropertyInitializationList<E: Entity>: PropertyList<E> {}

// The empty tuple is an empty `PropertyList<E>` for every `E: Entity`.
impl<E: Entity> PropertyList<E> for () {
    fn validate() -> Result<(), IxaError> {
        Ok(())
    }
    fn contains_properties(property_type_ids: &[TypeId]) -> bool {
        property_type_ids.is_empty()
    }
    fn set_values_for_new_entity(
        &self,
        _entity_id: EntityId<E>,
        _property_store: &mut PropertyStore<E>,
    ) {
        // No values to assign.
    }

    fn get_values_for_entity(_context: &Context, _entity_id: EntityId<E>) -> Self {}
}

// An Entity ZST itself is an empty `PropertyList` for that entity.
// This allows `context.add_entity(Person)` instead of `context.add_entity(())`.
impl<E: Entity + Copy> PropertyList<E> for E {
    fn validate() -> Result<(), IxaError> {
        Ok(())
    }
    fn contains_properties(property_type_ids: &[TypeId]) -> bool {
        property_type_ids.is_empty()
    }
    fn set_values_for_new_entity(
        &self,
        _entity_id: EntityId<E>,
        _property_store: &mut PropertyStore<E>,
    ) {
        // No values to assign.
    }

    fn get_values_for_entity(_context: &Context, _entity_id: EntityId<E>) -> E {
        E::default()
    }
}

impl<E: Entity + Copy> PropertyInitializationList<E> for E {}

// ToDo(RobertJacobsonCDC): The following is a fundamental limitation in Rust. If downstream code *can* implement a
//     trait impl that will cause conflicting implementations with some blanket impl, it disallows it, regardless of
//     whether the conflict actually exists.
// A single `Property` is a `PropertyList` of length 1
// impl<E: Entity, P: Property<E>> PropertyList<E> for P {
//     fn validate() -> Result<(), String> {
//         Ok(())
//     }
//     fn contains_properties(property_type_ids: &[TypeId]) -> bool {
//         property_type_ids.len() == 0
//             || property_type_ids.len() == 1 && property_type_ids[0] == P::type_id()
//     }
//     fn set_values_for_new_entity(&self, entity_id: EntityId<E>, property_store: &mut PropertyStore<E>) {
//         let property_value_store = property_store.get_mut::<P>();
//         property_value_store.set(entity_id, *self);
//     }
// }

// A single `Property` tuple is a `PropertyList` of length 1. This supports internal tuple
// machinery, but naked tuples are not accepted directly by `add_entity`.
impl<E: Entity, P: Property<E>> PropertyList<E> for (P,) {
    fn validate() -> Result<(), IxaError> {
        Ok(())
    }
    fn contains_properties(property_type_ids: &[TypeId]) -> bool {
        property_type_ids.is_empty()
            || property_type_ids.len() == 1 && property_type_ids[0] == P::type_id()
    }
    fn set_values_for_new_entity(
        &self,
        entity_id: EntityId<E>,
        property_store: &mut PropertyStore<E>,
    ) {
        let property_value_store = property_store.get_mut::<P>();
        property_value_store.set(entity_id, self.0);
    }

    fn get_values_for_entity(context: &Context, entity_id: EntityId<E>) -> Self {
        (context.get_property::<E, P>(entity_id),)
    }
}

// Used only within this module.
macro_rules! impl_property_list {
    ($ct:literal) => {
        seq!(N in 0..$ct {
            impl<E: Entity, #( P~N: Property<E>,)*> PropertyList<E> for (#(P~N, )*){
                fn validate() -> Result<(), IxaError> {
                    // For `Property` distinctness check
                    let property_type_ids: [TypeId; $ct] = [#(<P~N as $crate::entity::property::Property<E>>::type_id(),)*];

                    for i in 0..$ct - 1 {
                        for j in (i + 1)..$ct {
                            if property_type_ids[i] == property_type_ids[j] {
                                return Err(IxaError::DuplicatePropertyInPropertyList {
                                    first_index: i,
                                    second_index: j,
                                });
                            }
                        }
                    }

                    Ok(())
                }

                fn contains_properties(property_type_ids: &[TypeId]) -> bool {
                    let self_property_type_ids: [TypeId; $ct] = [#(<P~N as $crate::entity::property::Property<E>>::type_id(),)*];

                    property_type_ids.len() <= $ct && property_type_ids.iter().all(|id| self_property_type_ids.contains(id))
                }

                fn set_values_for_new_entity(&self, entity_id: EntityId<E>, property_store: &mut PropertyStore<E>){
                    #({
                        let property_value_store = property_store.get_mut::<P~N>();
                        property_value_store.set(entity_id, self.N);
                    })*
                }

                fn get_values_for_entity(context: &Context, entity_id: EntityId<E>) -> Self {
                    (#(context.get_property::<E, P~N>(entity_id), )*)
                }
            }
        });
    };
}

// Generate impls for tuple lengths 2 through 20. These tuple impls remain available for internal
// initialization/query machinery and for type-level strata lists, but not as direct `add_entity`
// inputs.
seq!(Z in 2..=20 {
    impl_property_list!(Z);
});
