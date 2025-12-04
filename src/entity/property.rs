/*!

A `Property` is the newtype value type for properties associated to an `Entity`.

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
    /// The property is not derived and has no initial value. Its initialization is _explicit_, meaning if client
    /// code doesn't set the value explicitly, then the value is not set, and attempts to read the value will result
    /// in an error.
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
    /// If `make_uncanonical` is nontrivial, this method usually transforms `value` into a
    /// `Self` first so that the value is formatted in a way the user expects.
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
    fn index() -> usize;

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
        unsafe { get_property_dependents_static(Self::index()) }
    }
}

#[cfg(feature = "disabled")]
mod tests {
    use super::*;
    use crate::{define_entity, define_property};

    #[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
    struct Person;

    define_entity!(Person);

    #[derive(Copy, Clone, Debug, PartialEq, Serialize)]
    pub struct Pu32(u32);
    define_property!(Pu32, Person);

    #[derive(Copy, Clone, Debug, PartialEq, Serialize)]
    pub struct POu32(Option<u32>);
    define_property!(POu32, Person);

    #[derive(Copy, Clone, Debug, PartialEq, Serialize)]
    pub struct Name(&'static str);
    define_property!(Name, Person);

    #[derive(Copy, Clone, Debug, PartialEq, Serialize)]
    pub struct Age(u8);
    define_property!(Age, Person);

    #[derive(Copy, Clone, Debug, PartialEq, Serialize)]
    pub struct Weight(f64);
    define_property!(Weight, Person);

    /*
        define_multi_property!(ProfileNAW, (Name, Age, Weight));
        define_multi_property!(ProfileAWN, (Age, Weight, Name));
        define_multi_property!(ProfileWAN, (Weight, Age, Name));

        #[test]
        fn test_multi_property_ordering() {
            let a: ProfileNAW = ("Jane", 22, 180.5);
            let b: ProfileAWN = (22, 180.5, "Jane");
            let c: ProfileWAN = (180.5, 22, "Jane");

            assert_eq!(ProfileNAW::type_id(), ProfileAWN::type_id());
            assert_eq!(ProfileNAW::type_id(), ProfileWAN::type_id());

            let a_canonical: <ProfileNAW as Property>::CanonicalValue =
                ProfileNAW::make_canonical(a);
            let b_canonical: <ProfileAWN as Property>::CanonicalValue =
                ProfileAWN::make_canonical(b);
            let c_canonical: <ProfileWAN as Property>::CanonicalValue =
                ProfileWAN::make_canonical(c);

            assert_eq!(a_canonical, b_canonical);
            assert_eq!(a_canonical, c_canonical);

            // Actually, all of the `Profile***::hash_property_value` methods should be the same,
            // so we could use any single one.
            assert_eq!(
                ProfileNAW::hash_property_value(&a_canonical),
                ProfileAWN::hash_property_value(&b_canonical)
            );
            assert_eq!(
                ProfileNAW::hash_property_value(&a_canonical),
                ProfileWAN::hash_property_value(&c_canonical)
            );

            // Since the canonical values are the same, we could have used any single one, but this
            // demonstrates that we can convert from one order to another.
            assert_eq!(ProfileNAW::make_uncanonical(b_canonical), a);
            assert_eq!(ProfileAWN::make_uncanonical(c_canonical), b);
            assert_eq!(ProfileWAN::make_uncanonical(a_canonical), c);
        }

        #[test]
        fn test_multi_property_vs_property_query() {
            let mut context = Context::new();

            context
                .add_person(((Name, "John"), (Age, 42), (Weight, 220.5)))
                .unwrap();
            context
                .add_person(((Name, "Jane"), (Age, 22), (Weight, 180.5)))
                .unwrap();
            context
                .add_person(((Name, "Bob"), (Age, 32), (Weight, 190.5)))
                .unwrap();
            context
                .add_person(((Name, "Alice"), (Age, 22), (Weight, 170.5)))
                .unwrap();

            context.index_property(ProfileNAW);

            {
                let data = context.get_data(PeoplePlugin);
                assert!(data
                    .property_indexes
                    .borrow()
                    .get(&ProfileNAW::type_id())
                    .is_some());
            }

            {
                let example_query = ((Name, "Alice"), (Age, 22), (Weight, 170.5));
                let query_multi_property_type_id = Query::multi_property_type_id(&example_query);
                assert!(query_multi_property_type_id.is_some());
                assert_eq!(ProfileNAW::type_id(), query_multi_property_type_id.unwrap());
                assert_eq!(
                    Query::multi_property_value_hash(&example_query),
                    ProfileNAW::hash_property_value(&ProfileNAW::make_canonical(("Alice", 22, 170.5)))
                );
            }

            context.with_query_results((ProfileNAW, ("John", 42, 220.5)), &mut |results| {
                assert_eq!(results.len(), 1);
            });
        }
    */

    #[test]
    fn test_get_display() {
        let mut context = Context::new();
        let person = context.add_person(((POu32, Some(42)), (Pu32, 22))).unwrap();
        assert_eq!(
            format!(
                "{:}",
                POu32::get_display(&context.get_property(person, POu32))
            ),
            "42"
        );
        assert_eq!(
            format!(
                "{:}",
                Pu32::get_display(&context.get_property(person, Pu32))
            ),
            "22"
        );
        let person2 = context.add_person(((POu32, None), (Pu32, 11))).unwrap();
        assert_eq!(
            format!(
                "{:}",
                POu32::get_display(&context.get_property(person2, POu32))
            ),
            "None"
        );
    }

    #[test]
    fn test_debug_trait() {
        let property = Pu32;
        let debug_str = format!("{:?}", property);
        assert_eq!(debug_str, "Pu32");

        let property = POu32;
        let debug_str = format!("{:?}", property);
        assert_eq!(debug_str, "POu32");
    }
}
