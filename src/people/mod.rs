//! A generic mechanism for representing people and associated data.
//!
//! We have a set of people indexed by [`PersonId`] and then each person
//! can have an arbitrary number of person properties
//! [`PersonProperty`], which are values keyed by a type. Person
//! properties are defined with a macro ([`define_person_property!()`]
//! or [`define_person_property_with_default!()`])
//!
//! # Initializing Person Properties
//!
//! Person properties can have their initial values set in several ways:
//!
//! * An initial value can be provided at person creation time in
//!   [`Context::add_person()`].
//! * The property can have a default value (provided when the
//!   property is defined.)
//! * The property can have an initializer function (provided when
//!   the property is defined) that is called lazily when the
//!   property is first accessed.
//!
//! If neither a default or an initializer is provided, then you
//! must provide an initial value for each person on person
//! creation. Failure to do so will generally cause failure of
//! [`Context::add_person()`].
//!
//! # Setting Person Properties
//!
//! Properties can also have their values changed with [`Context::set_person_property()`].
//! If the property is not initialized yet, this will implicitly call the
//! initializer (or set the default) and then reset the value.
//!
//! # Derived Properties
//!
//! It is also possible to have a "derived property" whose value is
//! computed based on a set of other properties. When a derived
//! property is defined, you must supply a function that takes the
//! values for those dependencies and computes the current value
//! of the property. Note that this function cannot access the context
//! directly and therefore cannot read any other properties. It also
//! should have a consistent result for any set of inputs, because
//! it may be called multiple times with those inputs, depending
//! on the program structure.
//!
//! # Change Events
//!
//! Whenever a person property `E` has potentially changed, either
//! because it was set directly or because it is a derived property
//! and one of its dependencies changed, a
//! [`PersonPropertyChangeEvent<E>`] will be emitted. Note that Ixa does
//! not currently check that the new value is actually different from the old value,
//! so calling [`Context::set_person_property()`] will always emit an event.
//! Initialization is not considered a change, but [`Context::set_person_property()`]
//! on a lazily initialized event will emit an event for the change from
//! the initialized value to the new value.
//!
//! # Querying
//!
//! Person properties provides an interface to query for people matching
//! a given set of properties. The basic syntax is to supply a set of
//! (property, value) pairs, like so `query_people(((Age, 30), (Gender, Female)))`.
//! Note that these need to be wrapped in an extra set of parentheses
//! to make them a single tuple to pass to [`Context::query_people()`]. Queries implement
//! strict equality, so if you want a fancier predicate you need to implement
//! a derived property that computes it and then query over the derived property.
//!
//! The internals of query are deliberately opaque in that Ixa may or
//! may not ordinarily choose to create caches or indexes for
//! queries. However, you force an index to be created for a single
//! property by using [`Context::index_property()`].

mod context_extension;
mod data;
mod event;
pub(crate) mod external_api;
mod index;
pub(crate) mod methods;
mod multi_property;
mod property;
mod query;

use std::any::TypeId;
use std::cell::RefCell;
use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;

pub use context_extension::ContextPeopleExt;
use data::PeopleData;
pub use data::PersonPropertyHolder;
pub use event::{PersonCreatedEvent, PersonPropertyChangeEvent};
pub use index::Index;
pub use multi_property::*;
pub use property::{
    define_derived_property, define_multi_property, define_person_property,
    define_person_property_with_default, PersonProperty,
};
pub use query::Query;
use seq_macro::seq;
use serde::{Deserialize, Serialize};

use crate::context::Context;
use crate::{define_data_plugin, HashMap, HashMapExt, HashSet, HashSetExt};

pub type HashValueType = u128;

define_data_plugin!(
    PeoplePlugin,
    PeopleData,
    PeopleData {
        is_initializing: false,
        current_population: 0,
        methods: RefCell::new(HashMap::new()),
        properties_map: RefCell::new(HashMap::new()),
        registered_properties: RefCell::new(HashSet::new()),
        dependency_map: RefCell::new(HashMap::new()),
        property_indexes: RefCell::new(HashMap::new()),
        people_types: RefCell::new(HashMap::new()),
    }
);

/// Represents a unique person.
//  The id refers to that person's index in the range 0 to population
// - 1 in the PeopleData container.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonId(pub(crate) usize);

impl Display for PersonId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Debug for PersonId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Person {}", self.0)
    }
}

/// A trait that contains the initialization values for a
/// new person. Do not use this directly, but instead use
/// the tuple syntax.
pub trait InitializationList {
    fn has_property(&self, t: TypeId) -> bool;
    fn set_properties(&self, context: &mut Context, person_id: PersonId);
}

// Implement the query version with 0 and 1 parameters
impl InitializationList for () {
    fn has_property(&self, _: TypeId) -> bool {
        false
    }
    fn set_properties(&self, _context: &mut Context, _person_id: PersonId) {}
}

impl<T1: PersonProperty> InitializationList for (T1, T1::Value) {
    fn has_property(&self, t: TypeId) -> bool {
        t == TypeId::of::<T1>()
    }

    fn set_properties(&self, context: &mut Context, person_id: PersonId) {
        context.set_person_property(person_id, T1::get_instance(), self.1);
    }
}

// Implement the versions with 1..20 parameters.
macro_rules! impl_initialization_list {
    ($ct:expr) => {
        seq!(N in 0..$ct {
            impl<
                #(
                    T~N : PersonProperty,
                )*
            > InitializationList for (
                #(
                    (T~N, T~N::Value),
                )*
            )
            {
                fn has_property(&self, t: TypeId) -> bool {
                    #(
                        if t == TypeId::of::<T~N>() { return true; }
                    )*
                    return false
                }

                fn set_properties(&self, context: &mut Context, person_id: PersonId)  {
                    #(
                       context.set_person_property(person_id, T~N::get_instance(), self.N.1 );
                    )*
                }
            }
        });
    }
}

seq!(Z in 1..20 {
    impl_initialization_list!(Z);
});
