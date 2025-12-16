/*!

A [`PropertyList<E>`] is just a tuple of distinct properties of the same [`Entity`] `E`. It
is used in two distinct places: as an initialization list for a new entity, and as a query.

Both use cases have the following two constraints:

1. The properties are properties of the same entity.
2. The properties are distinct.

We enforce the first constraint with the type system by only implementing `PropertyList<E>`
for tuples of types implementing `Property<E>` (of length up to some max). Using properties
for mismatched entities will result in a nice compile-time error at the point of use.

Unfortunately, the second constraint has to be enforced at runtime. We implement `PropertyList::validate()` to do this.

For both use cases, the order in which the properties appear is
unimportant in spite of the Rust language semantics of tuple types.

*/

use std::any::TypeId;

use seq_macro::seq;

use super::entity::{Entity, EntityId};
use super::property::Property;
use super::property_store::PropertyStore;

pub trait PropertyList<E: Entity>: Copy + 'static {
    /// Validates that the properties are distinct. If not, returns a string describing the problematic properties.
    fn validate() -> Result<(), String>;

    /// Checks that this property list includes all properties in the given list.
    fn contains_properties(property_type_ids: &[TypeId]) -> bool;

    /// Checks that this property list contains all required properties of the entity.
    fn contains_required_properties() -> bool {
        Self::contains_properties(E::required_property_ids())
    }

    /// Assigns the given entity the property values in `self` in the `property_store`.
    /// This method does NOT emit property change events, as it is called upon entity creation.
    fn set_values_for_entity(&self, entity_id: EntityId<E>, property_store: &PropertyStore);
}

// The empty tuple is an empty `PropertyList<E>` for every `E: Entity`.
impl<E: Entity> PropertyList<E> for () {
    fn validate() -> Result<(), String> {
        Ok(())
    }
    fn contains_properties(property_type_ids: &[TypeId]) -> bool {
        property_type_ids.is_empty()
    }
    fn set_values_for_entity(&self, _entity_id: EntityId<E>, _property_store: &PropertyStore) {
        // No values to assign.
    }
}

// ToDo: Why does the following trigger a "conflicting implementation" error?
// A single `Property` is a `PropertyList` of length 1
// impl<E: Entity, P: Property<E>> PropertyList<E> for P {
//     fn validate() -> Result<(), String> {
//         Ok(())
//     }
// }

// A single `Property` tuple is a `PropertyList` of length 1
impl<E: Entity, P: Property<E>> PropertyList<E> for (P,) {
    fn validate() -> Result<(), String> {
        Ok(())
    }
    fn contains_properties(property_type_ids: &[TypeId]) -> bool {
        property_type_ids.len() == 0
            || property_type_ids.len() == 1 && property_type_ids[0] == P::type_id()
    }
    fn set_values_for_entity(&self, entity_id: EntityId<E>, property_store: &PropertyStore) {
        let property_value_store = property_store.get::<E, P>();
        property_value_store.set(entity_id, self.0);
    }
}

// Used only within this module.
macro_rules! impl_property_list {
    ($ct:literal) => {
        seq!(N in 0..$ct {
            impl<E: Entity, #( P~N: Property<E>,)*> PropertyList<E> for (#(P~N, )*){
                fn validate() -> Result<(), String> {
                    // For `Property` distinctness check
                    let property_type_ids: [TypeId; $ct] = [#(P~N::type_id(),)*];

                    for i in 0..$ct - 1 {
                        for j in (i + 1)..$ct {
                            if property_type_ids[i] == property_type_ids[j] {
                                return Err(format!(
                                    "the same property appears in both position {} and {} in the property list",
                                    i,
                                    j
                                ));
                            }
                        }
                    }

                    Ok(())
                }

                fn contains_properties(property_type_ids: &[TypeId]) -> bool {
                    let self_property_type_ids: [TypeId; $ct] = [#(P~N::type_id(),)*];

                    property_type_ids.len() <= $ct && property_type_ids.iter().all(|id| self_property_type_ids.contains(id))
                }

                fn set_values_for_entity(&self, entity_id: EntityId<E>, property_store: &PropertyStore){
                    #({
                        let property_value_store = property_store.get::<E, P~N>();
                        // The compiler isn't smart enough to know that `entity_id` is `Copy` when this is
                        // borrow-checked, so we clone it.
                        property_value_store.set(entity_id.clone(), self.N);
                    })*
                }
            }
        });
    };
}

// Generate impls for tuple lengths 2 through 10.
seq!(Z in 2..=5 {
    impl_property_list!(Z);
});
