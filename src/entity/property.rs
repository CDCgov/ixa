/*!

A `Property` is the value type for properties associated to an `Entity`.

The `Property` trait should be implemented only with one of the macros `define_property!`, `impl_property!`,
`define_derived_property!`, `impl_derived_property!`, or `define_multi_property!` to ensure correct and consistent
implementation.

*/

use std::any::{Any, TypeId};
use std::fmt::Debug;
use std::hash::Hash;

use crate::entity::property_store::get_property_dependents_static;
use crate::entity::{Entity, EntityId};
use crate::hashing::{one_shot_128, HashValueType};
use crate::{Context, HashSet};

/// The kind of initialization that a property has.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PropertyInitializationKind {
    /// The property is not derived and has no initial value. Its initialization is _explicit_, meaning it must be set
    /// by client code at time of creation. Initialization is _explicit_ if and only if the property is _required_,
    /// that is, if a value for the property must be supplied at time of entity creation.
    Explicit,

    /// The property is a derived property (it's value is computed dynamically from other property values). It cannot
    /// be set explicitly.
    Derived,

    /// The property is given a constant initial value. Its initialization does not
    /// trigger a change event.
    Constant,
}

/// Shared trait bounds for property values.
///
/// These values must be copyable and support equality and deterministic hashing so they can
/// participate in indexing.
pub trait AnyProperty: Copy + Debug + PartialEq + Eq + Hash + 'static {}
impl<T> AnyProperty for T where T: Copy + Debug + PartialEq + Eq + Hash + 'static {}

/// `const fn` string equality — `==` on `&str` isn't `const` on stable.
#[must_use]
pub const fn const_str_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let a = a.as_bytes();
    let b = b.as_bytes();
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// All properties must implement this trait using one of the `define_property` macros.
///
/// Property values must satisfy `AnyProperty` so they can participate in property indexes.
pub trait Property<E: Entity>: AnyProperty {
    /// Allocation-free representation of the query parts contributed by a property value.
    type QueryParts<'a>: AsRef<[&'a dyn Any]>
    where
        Self: 'a;

    /// Source-level name, set by the macros to `stringify!($property)`.
    const NAME: &'static str;

    fn name() -> &'static str {
        Self::NAME
    }

    /// The kind of initialization this property has.
    #[must_use]
    fn initialization_kind() -> PropertyInitializationKind;

    #[must_use]
    #[inline]
    fn is_derived() -> bool {
        Self::initialization_kind() == PropertyInitializationKind::Derived
    }

    #[must_use]
    #[inline]
    fn is_required() -> bool {
        Self::initialization_kind() == PropertyInitializationKind::Explicit
    }

    /// Compute the value of the property, possibly by accessing the context and using the entity's ID.
    #[must_use]
    fn compute_derived(context: &Context, entity_id: EntityId<E>) -> Self;

    /// Return the default initial constant value.
    #[must_use]
    fn default_const() -> Self;

    /// Returns a string representation of the property value, e.g. for writing to a CSV file.
    #[must_use]
    fn get_display(&self) -> String;

    /// Reconstruct the property value used for indexed lookup.
    ///
    /// Ordinary properties expect a single query part containing `Self`. Multi-properties override
    /// this to rebuild their declared tuple value directly from already-sorted type-erased query
    /// parts.
    #[must_use]
    fn value_from_query_parts(parts: &[&dyn Any]) -> Option<Self> {
        let [part] = parts else {
            return None;
        };
        part.downcast_ref::<Self>().copied()
    }

    /// Hash the logical query value represented by `value`.
    ///
    /// This hash is used as part of the `PropertySourceId` for property-backed entity
    /// sets. It lets the entity-set layer recognize when two sources represent the same
    /// logical query, even if they are produced through different internal source types.
    ///
    /// Multi-properties override this so equivalent tuple orders hash the component values in the
    /// same `TypeId` order and therefore share the same source identity.
    #[must_use]
    fn query_value_hash(value: &Self) -> HashValueType {
        one_shot_128(value)
    }

    /// Expose the query parts for a concrete property value without allocating.
    ///
    /// Ordinary properties contribute a single value. Multi-properties override this so singleton
    /// queries over a multi-property can still be matched against the representative
    /// multi-property for the equivalent component set.
    #[must_use]
    fn query_parts_for_value(value: &Self) -> Self::QueryParts<'_>;

    /// The logical type identity for this property.
    #[must_use]
    fn type_id() -> TypeId {
        TypeId::of::<Self>()
    }

    /// For implementing the registry pattern
    fn id() -> usize;

    /// Returns a vector of transitive non-derived dependencies. If the property is not derived, the
    /// Vec will be empty. The dependencies are represented by their `Property<E>::id()` value.
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
    /// represented by their `Property::id()`. The list is pre-computed in `ctor`s.
    fn dependents() -> &'static [usize] {
        get_property_dependents_static::<E>(Self::id())
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;

    use super::*;
    use crate::{define_entity, define_property};

    define_entity!(PropertyTestPerson);
    define_property!(struct PropertyTestAge(u8), PropertyTestPerson);

    #[test]
    fn const_str_eq_compares_lengths_and_bytes() {
        assert!(const_str_eq("Age", "Age"));
        assert!(!const_str_eq("Age", "Ages"));
        assert!(!const_str_eq("Age", "Axe"));
    }

    #[test]
    fn default_property_query_helpers_use_single_value() {
        let value = PropertyTestAge(42);
        let parts = [&value as &dyn Any];

        assert_eq!(
            <PropertyTestAge as Property<PropertyTestPerson>>::canonical_from_sorted_query_parts(
                &parts
            ),
            Some(value)
        );
        assert_eq!(
            <PropertyTestAge as Property<PropertyTestPerson>>::canonical_from_sorted_query_parts(
                &[]
            ),
            None
        );
        assert_eq!(
            <PropertyTestAge as Property<PropertyTestPerson>>::index_id(),
            <PropertyTestAge as Property<PropertyTestPerson>>::id()
        );
    }
}
