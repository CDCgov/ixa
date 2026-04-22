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

/// Shared trait bounds for property values and canonical values.
///
/// These values must be copyable and support equality and deterministic hashing so they can
/// participate in indexing.
pub trait AnyProperty: Copy + Debug + PartialEq + Eq + Hash + 'static {}
impl<T> AnyProperty for T where T: Copy + Debug + PartialEq + Eq + Hash + 'static {}

/// All properties must implement this trait using one of the `define_property` macros.
///
/// Property values and canonical values must satisfy `AnyProperty` so they can participate in
/// property indexes.
pub trait Property<E: Entity>: AnyProperty {
    /// Some properties might store a transformed version of the value in the index. This is the
    /// type of the transformed value. For simple properties this will be the same as `Self`.
    type CanonicalValue: AnyProperty;

    /// Allocation-free representation of the query parts contributed by a property value.
    type QueryParts<'a>: AsRef<[&'a dyn Any]>
    where
        Self: 'a;

    fn name() -> &'static str {
        let full = std::any::type_name::<Self>();
        full.rsplit("::").next().unwrap()
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

    /// This transforms a `Self` into a `Self::CanonicalValue`, e.g., for storage in an index.
    /// For simple properties, this is the identity function.
    #[must_use]
    fn make_canonical(self) -> Self::CanonicalValue;

    /// The inverse transform of `make_canonical`. For simple properties, this is the identity function.
    #[must_use]
    fn make_uncanonical(value: Self::CanonicalValue) -> Self;

    /// Returns a string representation of the property value, e.g. for writing to a CSV file.
    #[must_use]
    fn get_display(&self) -> String;

    /// Reconstruct the canonical query value used for indexed lookup.
    ///
    /// Ordinary properties expect a single query part containing `Self` and canonicalize that
    /// value. Multi-properties override this to rebuild their canonical tuple value directly from
    /// the already-sorted type-erased query parts.
    #[must_use]
    fn canonical_from_sorted_query_parts(parts: &[&dyn Any]) -> Option<Self::CanonicalValue> {
        let [part] = parts else {
            return None;
        };
        part.downcast_ref::<Self>()
            .copied()
            .map(Self::make_canonical)
    }

    /// Expose the query parts for a concrete property value without allocating.
    ///
    /// Ordinary properties contribute a single value. Multi-properties override this so singleton
    /// queries over a multi-property can still be matched against a shared equivalent index.
    #[must_use]
    fn query_parts_for_value(value: &Self) -> Self::QueryParts<'_>;

    /// Overridden by multi-properties, which use the `TypeId` of the ordered tuple so that tuples
    /// with the same component types in a different order will have the same type ID.
    #[must_use]
    fn type_id() -> TypeId {
        TypeId::of::<Self>()
    }

    /// For implementing the registry pattern
    fn id() -> usize;

    /// For properties that use the index of some other property, e.g. multi-properties, this
    /// method gives the ID of the property index to use.
    ///
    /// Note that this is independent of whether or not the property actually is being indexed,
    /// which is a property of the `Context` instance, not of the `Property<E>` type itself.
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
        get_property_dependents_static::<E>(Self::id())
    }
}
