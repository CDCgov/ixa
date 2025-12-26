/*!

A `Property` is the value type for properties associated to an `Entity`.

*/

use std::any::TypeId;
use std::fmt::Debug;

use serde::Serialize;

use crate::entity::property_store::get_property_dependents_static;
use crate::entity::{Entity, EntityId};
use crate::hashing::hash_serialized_128;
use crate::{Context, HashSet};

/// The kind of initialization that a property has.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PropertyInitializationKind {
    /// The property is not derived and has no initial value. Its initialization is _explicit_, meaning it must be set by client code at time of creation.
    ///
    /// Note that a "required" property is explicit, but an explicit property need not be required.
    /// A non-required explicit property can be left unset.
    Explicit,

    /// The property is a derived property (it's value is computed dynamically from other property values)
    Derived,

    /// The property is given a constant initial value. Its initialization does not
    /// trigger a change event.
    Constant,
}

// A type-erased interface for properties.
pub trait AnyProperty: Copy + Debug + PartialEq + Serialize + 'static {}
impl<T> AnyProperty for T where T: Copy + Debug + PartialEq + Serialize + 'static {}

/// All properties must implement this trait using one of the `define_property` macros.
pub trait Property<E: Entity>: AnyProperty {
    /// Some properties might store a transformed version of the value in the index. This is the
    /// type of the transformed value. For simple properties this will be the same as `Self`.
    type CanonicalValue: AnyProperty;

    /// The kind of initialization this property has.
    #[must_use]
    fn initialization_kind() -> PropertyInitializationKind;

    /// Whether this property is derived.
    #[must_use]
    #[inline]
    fn is_derived() -> bool {
        Self::initialization_kind() == PropertyInitializationKind::Derived
    }

    #[must_use]
    fn is_required() -> bool {
        false
    }

    /// Compute the value of the property, possibly by accessing the context and using the entity's ID.
    #[must_use]
    fn compute_derived(context: &Context, entity_id: EntityId<E>) -> Self;

    /// Return the default initial constant value.
    #[must_use]
    fn default_const() -> Self;

    /// This transforms a `Self` into a `Self::CanonicalValue`, e.g., for storage in an index.
    /// For simple properties, this is the identity function.
    #[must_use]
    fn make_canonical(self) -> Self::CanonicalValue;

    /// The inverse transform of `make_canonical`. For simple properties, this is the identity function.
    #[must_use]
    fn make_uncanonical(value: Self::CanonicalValue) -> Self;

    fn name() -> &'static str;

    /// Returns a string representation of the property value, e.g. for writing to a CSV file.
    #[must_use]
    fn get_display(&self) -> String;

    /// For cases when the property's hash needs to be computed in a special way.
    #[must_use]
    fn hash_property_value(value: &Self::CanonicalValue) -> u128 {
        hash_serialized_128(value)
    }

    /// Overridden by multi-properties, which use the `TypeId` of the ordered tuple so that tuples
    /// with the same component types in a different order will have the same type ID.
    #[must_use]
    fn type_id() -> TypeId {
        TypeId::of::<Self>()
    }

    /// For implementing the registry pattern
    fn id() -> usize;

    /// For properties that use the index of some other property, e.g. multi-properties.
    fn index_id() -> usize {
        Self::id()
    }

    /// Returns a vector of transitive non-derived dependencies. If the property is not derived, the
    /// Vec will be empty. The dependencies are represented by their `Property<E>::index()` value.
    ///
    /// This function is only used to construct the static dependency graph
    /// within property `ctor`s, after which time the dependents of a property
    /// are accessible through `Property<E>::dependents()` as a `&'static [usize]`.
    fn non_derived_dependencies() -> Vec<usize> {
        let mut result = HashSet::default();
        Self::collect_non_derived_dependencies(&mut result);
        result.into_iter().collect()
    }

    /// An auxiliary helper for `non_derived_dependencies` above.
    fn collect_non_derived_dependencies(result: &mut HashSet<usize>);

    /// Get a list of derived properties that depend on this property. The properties are
    /// represented by their `Property::index()`. The list is pre-computed in `ctor`s.
    fn dependents() -> &'static [usize] {
        unsafe { get_property_dependents_static(Self::id()) }
    }
}
