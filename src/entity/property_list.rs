/*!

A [`PropertyList<E>`] is a tuple of properties of the same [`Entity`] `E`, used
as an initialization list for a new entity. It sets the property values for an entity
upon creation.

*/

use seq_macro::seq;

use super::entity::{Entity, EntityId};
use super::property::PropertyDef;
use super::property_store::PropertyStore;

pub trait PropertyList<E: Entity>: Copy + 'static {
    /// Assigns the given entity the property values in `self` in the `property_store`.
    /// This method does NOT emit property change events, as it is called upon entity creation.
    fn set_values_for_entity(&self, entity_id: EntityId<E>, property_store: &PropertyStore<E>);
}

// An entity marker type is an empty `PropertyList` for itself, allowing `add_entity(Person)`.
impl<E: Entity + Copy + 'static> PropertyList<E> for E {
    fn set_values_for_entity(&self, _entity_id: EntityId<E>, _property_store: &PropertyStore<E>) {
        // No values to assign.
    }
}

// The empty tuple is an empty `PropertyList<E>` for every `E: Entity`.
impl<E: Entity> PropertyList<E> for () {
    fn set_values_for_entity(&self, _entity_id: EntityId<E>, _property_store: &PropertyStore<E>) {
        // No values to assign.
    }
}

// A single `Property` tuple is a `PropertyList` of length 1
impl<E: Entity, P: PropertyDef<E, Value = P>> PropertyList<E> for (P,) {
    fn set_values_for_entity(&self, entity_id: EntityId<E>, property_store: &PropertyStore<E>) {
        let property_value_store = property_store.get::<P>();
        property_value_store.set(entity_id, self.0);
    }
}

// Used only within this module.
macro_rules! impl_property_list {
    ($ct:literal) => {
        seq!(N in 0..$ct {
            impl<E: Entity, #( P~N: PropertyDef<E, Value = P~N>,)*> PropertyList<E> for (#(P~N, )*){
                fn set_values_for_entity(&self, entity_id: EntityId<E>, property_store: &PropertyStore<E>){
                    #({
                        let property_value_store = property_store.get::<P~N>();
                        property_value_store.set(entity_id, self.N);
                    })*
                }
            }
        });
    };
}

// Generate impls for tuple lengths 2 through 5.
seq!(Z in 2..=5 {
    impl_property_list!(Z);
});
