use crate::{
    context::{Context, IxaEvent},
    define_data_plugin,
    error::IxaError,
};
use ixa_derive::IxaEvent;
use seq_macro::seq;
use serde::{Deserialize, Serialize};
use std::{
    any::{Any, TypeId},
    cell::{Ref, RefCell, RefMut},
    collections::{HashMap, HashSet},
    fmt::{self, Debug},
    hash::{Hash, Hasher},
    iter::Iterator,
};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
// The lookup key for entries in the index. This is a serialized
// version of the value. If that serialization fits in 128 bits, we
// store it in Fixed to avoid the allocation of the Vec. Otherwise it
// goes in Variable.
pub enum IndexValue {
    Fixed(u128),
    Variable(Vec<u8>),
}

impl IndexValue {
    pub fn compute<T: Hash>(val: &T) -> IndexValue {
        let mut hasher = IndexValueHasher::new();
        val.hash(&mut hasher);
        if hasher.buf.len() <= 16 {
            let mut tmp: [u8; 16] = [0; 16];
            tmp[..hasher.buf.len()].copy_from_slice(&hasher.buf[..]);
            return IndexValue::Fixed(u128::from_le_bytes(tmp));
        }
        IndexValue::Variable(hasher.buf)
    }
}

// Encapsulates a person query, allowing the user to call query_people()
// with tuple syntax, like so:
//
//   query_people((Age, 50), (IsInfected, true))
//
// query_people actually takes an instance of Query, but because
// we implement Query for tuples of up to size 20, that's invisible
// to the caller.
pub trait Query {
    fn setup(context: &Context);
    fn get_query(&self) -> Vec<(TypeId, IndexValue)>;
}

// Implement the query version with one parameter.
impl<T1: PersonProperty + 'static> Query for (T1, T1::Value) {
    fn setup(context: &Context) {
        context.register_property::<T1>();
        context.register_indexer::<T1>();
    }

    fn get_query(&self) -> Vec<(TypeId, IndexValue)> {
        vec![(std::any::TypeId::of::<T1>(), IndexValue::compute(&self.1))]
    }
}

// Implement the versions with 1..20 parameters.
macro_rules! impl_query {
    ($ct:expr) => {
        seq!(N in 0..$ct {
            impl<
                #(
                    T~N : PersonProperty + 'static,
                )*
            > Query for (
                #(
                    (T~N, T~N::Value),
                )*
            )
            {
                fn setup(context: &Context) {
                    #(
                        context.register_property::<T~N>();
                        context.register_indexer::<T~N>();
                    )*
                }

                fn get_query(&self) -> Vec<(TypeId, IndexValue)> {
                    vec![
                    #(
                        (std::any::TypeId::of::<T~N>(), IndexValue::compute(&self.N.1)),
                    )*
                    ]
                }
            }
        });
    }
}

seq!(Z in 1..20 {
    impl_query!(Z);
});

// Implementation of the Hasher interface for IndexValue, used
// for serialization. We're actually abusing this interface
// because you can't call finish().
struct IndexValueHasher {
    buf: Vec<u8>,
}

impl IndexValueHasher {
    fn new() -> Self {
        IndexValueHasher { buf: Vec::new() }
    }
}

impl Hasher for IndexValueHasher {
    fn write(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    fn finish(&self) -> u64 {
        panic!("Unimplemented")
    }
}

type Indexer = dyn Fn(&Context, PersonId) -> IndexValue;

// An index for a single property.
struct Index {
    // Primarily for debugging purposes
    #[allow(dead_code)]
    name: &'static str,
    // The hash of the property value maps to a list of PersonIds
    // or None if we're not indexing
    lookup: Option<HashMap<IndexValue, HashSet<PersonId>>>,
    // A callback that calculates the IndexValue of a person's current property value
    indexer: Box<Indexer>,
    // The largest person ID that has been indexed. Used so that we
    // can lazily index when a person is added.
    max_indexed: usize,
}

impl Index {
    fn new<T: PersonProperty + 'static>(_context: &Context, property: T) -> Self {
        Self {
            name: std::any::type_name::<T>(),
            lookup: None,
            indexer: Box::new(move |context: &Context, person_id: PersonId| {
                let value = context.get_person_property(person_id, property);
                IndexValue::compute(&value)
            }),
            max_indexed: 0,
        }
    }
    fn add_person(&mut self, context: &Context, person_id: PersonId) {
        let hash = (self.indexer)(context, person_id);
        self.lookup
            .as_mut()
            .unwrap()
            .entry(hash)
            .or_default()
            .insert(person_id);
    }
    fn remove_person(&mut self, context: &Context, person_id: PersonId) {
        let hash = (self.indexer)(context, person_id);
        self.lookup
            .as_mut()
            .unwrap()
            .entry(hash)
            .or_default()
            .remove(&person_id);
    }
    fn index_unindexed_people(&mut self, context: &Context) {
        if self.lookup.is_none() {
            return;
        }
        let current_pop = context.get_current_population();
        for id in self.max_indexed..current_pop {
            let person_id = PersonId { id };
            self.add_person(context, person_id);
        }
        self.max_indexed = current_pop;
    }
}

// PeopleData represents each unique person in the simulation with an id ranging
// from 0 to population - 1. Person properties are associated with a person
// via their id.
struct StoredPeopleProperties {
    is_required: bool,
    values: Box<dyn Any>,
}

impl StoredPeopleProperties {
    fn new<T: PersonProperty + 'static>() -> Self {
        StoredPeopleProperties {
            is_required: T::is_required(),
            values: Box::new(Vec::<Option<T::Value>>::new()),
        }
    }
}

struct PeopleData {
    is_initializing: bool,
    current_population: usize,
    properties_map: RefCell<HashMap<TypeId, StoredPeopleProperties>>,
    registered_derived_properties: RefCell<HashSet<TypeId>>,
    dependency_map: RefCell<HashMap<TypeId, Vec<Box<dyn PersonPropertyHolder>>>>,
    property_indexes: RefCell<HashMap<TypeId, Index>>,
}

define_data_plugin!(
    PeoplePlugin,
    PeopleData,
    PeopleData {
        is_initializing: false,
        current_population: 0,
        properties_map: RefCell::new(HashMap::new()),
        registered_derived_properties: RefCell::new(HashSet::new()),
        dependency_map: RefCell::new(HashMap::new()),
        property_indexes: RefCell::new(HashMap::new()),
    }
);

// Represents a unique person - the id refers to that person's index in the range
// 0 to population - 1 in the PeopleData container.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonId {
    pub(crate) id: usize,
}

impl fmt::Display for PersonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl fmt::Debug for PersonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Person {}", self.id)
    }
}

// Individual characteristics or states related to a person, such as age or
// disease status, are represented as "person properties". These properties
// * are represented by a struct type that implements the PersonProperty trait,
// * specify a Value type to represent the data associated with the property,
// * specify an initializer, which returns the initial value
// They may be defined with the define_person_property! macro.
pub trait PersonProperty: Copy {
    type Value: Copy + Debug + PartialEq + Hash;
    #[must_use]
    fn is_derived() -> bool {
        false
    }
    #[must_use]
    fn is_required() -> bool {
        false
    }
    #[must_use]
    fn dependencies() -> Vec<Box<dyn PersonPropertyHolder>> {
        panic!("Dependencies not implemented");
    }
    fn compute(context: &Context, person_id: PersonId) -> Self::Value;
    fn get_instance() -> Self;
}

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

impl<T1: PersonProperty + 'static> InitializationList for (T1, T1::Value) {
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
                    T~N : PersonProperty + 'static,
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

type ContextCallback = dyn FnOnce(&mut Context);

// The purpose of this trait is to enable storing a Vec of different
// `PersonProperty` types. While `PersonProperty`` is *not* object safe,
// primarily because it stores a different Value type for each kind of property,
// `PersonPropertyHolder` is, meaning we can treat different types of properties
// uniformly at runtime.
// Note: this has to be pub because `PersonProperty` (which is pub) implements
// a dependency method that returns `PersonPropertyHolder` instances.
pub trait PersonPropertyHolder {
    // Registers a callback in the provided `callback_vec` that is invoked when
    // a dependency of a derived property is updated for the given person
    //
    // Parameters:
    // - `context`: The mutable reference to the current execution context
    // - `person`: The PersonId of the person for whom the property change event will be emitted.
    // - `callback_vec`: A vector of boxed callback functions that will be called
    // - when a property is updated
    fn dependency_changed(
        &self,
        context: &mut Context,
        person: PersonId,
        callback_vec: &mut Vec<Box<ContextCallback>>,
    );
    fn is_derived(&self) -> bool;
    fn dependencies(&self) -> Vec<Box<dyn PersonPropertyHolder>>;
    fn non_derived_dependencies(&self) -> Vec<TypeId>;
    fn collect_non_derived_dependencies(&self, result: &mut HashSet<TypeId>);
    fn property_type_id(&self) -> TypeId;
}

impl<T> PersonPropertyHolder for T
where
    T: PersonProperty + 'static,
{
    fn dependency_changed(
        &self,
        context: &mut Context,
        person: PersonId,
        callback_vec: &mut Vec<Box<ContextCallback>>,
    ) {
        let previous = context.get_person_property(person, T::get_instance());
        context.remove_from_index_maybe(person, T::get_instance());

        // Captures the current value of the person property and defers the actual event
        // emission to when we have access to the new value.
        callback_vec.push(Box::new(move |ctx| {
            let current = ctx.get_person_property(person, T::get_instance());
            let change_event: PersonPropertyChangeEvent<T> = PersonPropertyChangeEvent {
                person_id: person,
                current,
                previous,
            };
            ctx.add_to_index_maybe(person, T::get_instance());
            ctx.emit_event(change_event);
        }));
    }

    fn is_derived(&self) -> bool {
        T::is_derived()
    }

    fn dependencies(&self) -> Vec<Box<dyn PersonPropertyHolder>> {
        T::dependencies()
    }

    fn property_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    /// Returns of dependencies, where any derived dependencies
    /// are recursively expanded to their non-derived dependencies.
    /// If the property is not derived, the Vec will be empty.
    fn non_derived_dependencies(&self) -> Vec<TypeId> {
        let mut result = HashSet::new();
        self.collect_non_derived_dependencies(&mut result);
        result.into_iter().collect()
    }

    fn collect_non_derived_dependencies(&self, result: &mut HashSet<TypeId>) {
        if !self.is_derived() {
            return;
        }
        for dependency in self.dependencies() {
            if dependency.is_derived() {
                dependency.collect_non_derived_dependencies(result);
            } else {
                result.insert(dependency.property_type_id());
            }
        }
    }
}

/// Defines a person property with the following parameters:
/// * `$person_property`: A name for the identifier type of the property
/// * `$value`: The type of the property's value
/// * `$initialize`: (Optional) A function that takes a `Context` and `PersonId` and
///   returns the initial value. If it is not defined, calling `get_person_property`
///   on the property without explicitly setting a value first will panic.
#[macro_export]
macro_rules! define_person_property {
    ($person_property:ident, $value:ty, $initialize:expr) => {
        #[derive(Debug, Copy, Clone)]
        pub struct $person_property;
        impl $crate::people::PersonProperty for $person_property {
            type Value = $value;
            fn compute(
                _context: &$crate::context::Context,
                _person: $crate::people::PersonId,
            ) -> Self::Value {
                $initialize(_context, _person)
            }
            fn get_instance() -> Self {
                $person_property
            }
        }
    };
    ($person_property:ident, $value:ty) => {
        #[derive(Debug, Copy, Clone)]
        pub struct $person_property;
        impl $crate::people::PersonProperty for $person_property {
            type Value = $value;
            fn compute(
                _context: &$crate::context::Context,
                _person: $crate::people::PersonId,
            ) -> Self::Value {
                panic!("Property not initialized when person created.");
            }
            fn is_required() -> bool {
                true
            }
            fn get_instance() -> Self {
                $person_property
            }
        }
    };
}

/// Defines a person property with the following parameters:
/// * `$person_property`: A name for the identifier type of the property
/// * `$value`: The type of the property's value
/// * `$default`: An initial value
#[macro_export]
macro_rules! define_person_property_with_default {
    ($person_property:ident, $value:ty, $default:expr) => {
        define_person_property!($person_property, $value, |_context, _person_id| {
            $default
        });
    };
}

/// Defines a derived person property with the following parameters:
/// * `$person_property`: A name for the identifier type of the property
/// * `$value`: The type of the property's value
/// * `[$($dependency),+]`: A list of person properties the derived property depends on
/// * $calculate: A closure that takes the values of each dependency and returns the derived value
#[macro_export]
macro_rules! define_derived_property {
    ($derived_property:ident, $value:ty, [$($dependency:ident),+], |$($param:ident),+| $derive_fn:expr) => {
        #[derive(Debug, Copy, Clone)]
        pub struct $derived_property;

        impl $crate::people::PersonProperty for $derived_property {
            type Value = $value;
            fn compute(context: &$crate::context::Context, person_id: $crate::people::PersonId) -> Self::Value {
                #[allow(unused_parens)]
                let ($($param),+) = (
                    $(context.get_person_property(person_id, $dependency)),+
                );
                (|$($param),+| $derive_fn)($($param),+)
            }
            fn is_derived() -> bool { true }
            fn dependencies() -> Vec<Box<dyn $crate::people::PersonPropertyHolder>> {
                vec![$(Box::new($dependency)),+]
            }
            fn get_instance() -> Self {
                $derived_property
            }
        }
    };
}

pub use define_person_property;

impl PeopleData {
    /// Adds a person and returns a `PersonId` that can be used to reference them.
    /// This will increment the current population by 1.
    fn add_person(&mut self) -> PersonId {
        let id = self.current_population;
        self.current_population += 1;
        PersonId { id }
    }

    /// Retrieves a specific property of a person by their `PersonId`.
    ///
    /// Returns `RefMut<Option<T::Value>>`: `Some(value)` if the property exists for the given person,
    /// or `None` if it doesn't.
    #[allow(clippy::needless_pass_by_value)]
    fn get_person_property_ref<T: PersonProperty + 'static>(
        &self,
        person: PersonId,
        _property: T,
    ) -> RefMut<Option<T::Value>> {
        let properties_map = self.properties_map.borrow_mut();
        let index = person.id;
        RefMut::map(properties_map, |properties_map| {
            let properties = properties_map
                .entry(TypeId::of::<T>())
                .or_insert_with(|| StoredPeopleProperties::new::<T>());
            let values: &mut Vec<Option<T::Value>> = properties
                .values
                .downcast_mut()
                .expect("Type mismatch in properties_map");
            if index >= values.len() {
                values.resize(index + 1, None);
            }
            &mut values[index]
        })
    }

    /// Sets the value of a property for a person
    #[allow(clippy::needless_pass_by_value)]
    fn set_person_property<T: PersonProperty + 'static>(
        &self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        let mut property_ref = self.get_person_property_ref(person_id, property);
        *property_ref = Some(value);
    }

    fn get_index_ref_mut(&self, t: TypeId) -> Option<RefMut<Index>> {
        let index_map = self.property_indexes.borrow_mut();
        if index_map.contains_key(&t) {
            Some(RefMut::map(index_map, |map| map.get_mut(&t).unwrap()))
        } else {
            None
        }
    }

    fn get_index_ref(&self, t: TypeId) -> Option<Ref<Index>> {
        let index_map = self.property_indexes.borrow();
        if index_map.contains_key(&t) {
            Some(Ref::map(index_map, |map| map.get(&t).unwrap()))
        } else {
            None
        }
    }

    fn get_index_ref_mut_by_prop<T: PersonProperty + 'static>(
        &self,
        _property: T,
    ) -> Option<RefMut<Index>> {
        let type_id = TypeId::of::<T>();
        self.get_index_ref_mut(type_id)
    }

    // Convenience function to iterate over the current population.
    // Note that this doesn't hold a reference to PeopleData, so if
    // you change the population while using it, it won't notice.
    fn people_iterator(&self) -> PeopleIterator {
        PeopleIterator {
            population: self.current_population,
            person_id: 0,
        }
    }

    fn check_initialization_list<T: InitializationList>(
        &self,
        initialization: &T,
    ) -> Result<(), IxaError> {
        let properties_map = self.properties_map.borrow();
        for (t, property) in properties_map.iter() {
            if property.is_required && !initialization.has_property(*t) {
                return Err(IxaError::IxaError(String::from("Missing initial value")));
            }
        }

        Ok(())
    }
}

struct PeopleIterator {
    population: usize,
    person_id: usize,
}

impl Iterator for PeopleIterator {
    type Item = PersonId;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = if self.person_id < self.population {
            Some(PersonId { id: self.person_id })
        } else {
            None
        };
        self.person_id += 1;

        ret
    }
}

// Emitted when a new person is created
// These should not be emitted outside this module
#[derive(Clone, Copy, IxaEvent)]
#[allow(clippy::manual_non_exhaustive)]
pub struct PersonCreatedEvent {
    pub person_id: PersonId,
}

// Emitted when a person property is updated
// These should not be emitted outside this module
#[derive(Copy, Clone)]
#[allow(clippy::manual_non_exhaustive)]
pub struct PersonPropertyChangeEvent<T: PersonProperty> {
    pub person_id: PersonId,
    pub current: T::Value,
    pub previous: T::Value,
}
impl<T: PersonProperty + 'static> IxaEvent for PersonPropertyChangeEvent<T> {
    fn on_subscribe(context: &mut Context) {
        if T::is_derived() {
            context.register_property::<T>();
        }
    }
}

pub trait ContextPeopleExt {
    /// Returns the current population size
    fn get_current_population(&self) -> usize;

    /// Creates a new person. The caller must supply initial values
    /// for all non-derived properties that don't have a default or an initializer.
    ///
    /// # Errors
    /// Will return `IxaError` if a required initializer is not provided.
    fn add_person<T: InitializationList>(&mut self, props: T) -> Result<PersonId, IxaError>;

    /// Given a `PersonId` returns the value of a defined person property,
    /// initializing it if it hasn't been set yet. If no initializer is
    /// provided, and the property is not set this will panic, as long
    /// as the property has been set or subscribed to at least once before.
    /// Otherwise, Ixa doesn't know about the property.
    fn get_person_property<T: PersonProperty + 'static>(
        &self,
        person_id: PersonId,
        _property: T,
    ) -> T::Value;

    fn register_property<T: PersonProperty + 'static>(&self);

    /// Given a `PersonId`, sets the value of a defined person property
    /// Panics if the property is not initialized. Fires a change event.
    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        _property: T,
        value: T::Value,
    );

    // Returns a PersonId for a usize
    fn get_person_id(&self, person_id: usize) -> PersonId;
    fn index_property<T: PersonProperty + 'static>(&mut self, property: T);
    fn query_people<T: Query>(&self, q: T) -> Vec<PersonId>;
    fn match_person<T: Query>(&self, person_id: PersonId, q: T) -> bool;
}

impl ContextPeopleExt for Context {
    fn get_current_population(&self) -> usize {
        self.get_data_container(PeoplePlugin)
            .map_or(0, |data_container| data_container.current_population)
    }

    fn add_person<T: InitializationList>(&mut self, props: T) -> Result<PersonId, IxaError> {
        let data_container = self.get_data_container_mut(PeoplePlugin);
        // Verify that every property that was supposed to be provided
        // actually was.
        data_container.check_initialization_list(&props)?;

        // Actually add the person. Nothing can fail after this point because
        // it would leave the person in an inconsistent state.
        let person_id = data_container.add_person();

        // Initialize the properties. We set |is_initializing| to prevent
        // set_person_property() from generating an event.
        data_container.is_initializing = true;
        props.set_properties(self, person_id);
        let data_container = self.get_data_container_mut(PeoplePlugin);
        data_container.is_initializing = false;

        self.emit_event(PersonCreatedEvent { person_id });
        Ok(person_id)
    }

    fn get_person_property<T: PersonProperty + 'static>(
        &self,
        person_id: PersonId,
        property: T,
    ) -> T::Value {
        let data_container = self.get_data_container(PeoplePlugin)
            .expect("PeoplePlugin is not initialized; make sure you add a person before accessing properties");

        if T::is_derived() {
            return T::compute(self, person_id);
        }

        // Attempt to retrieve the existing value
        if let Some(value) = *data_container.get_person_property_ref(person_id, property) {
            return value;
        }

        // Initialize the property. This does not fire a change event
        let initialized_value = T::compute(self, person_id);
        data_container.set_person_property(person_id, property, initialized_value);

        initialized_value
    }

    #[allow(clippy::single_match_else)]
    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        assert!(!T::is_derived(), "Cannot set a derived property");

        // This function can be called in two separate modes:
        //
        // 1. As a regular API function, in which case we want to
        //    emit an event and notify dependencies.
        // 2. Internally as part of initialization during add_person()
        //    in which case no events are emitted.
        //
        // Which mode it is is determined by the data_container.is_initializing
        // property, which is set by add_person. This is complicated but
        // necessary because the initialization functions are called by
        // a per-PersonProperty closure generated by a macro and so are
        // outside of the crate, but we don't want to expose a public
        // initialize_person_property() function.
        //
        // Temporarily remove dependency properties since we need mutable references
        // to self during callback execution
        let initializing = self
            .get_data_container(PeoplePlugin)
            .unwrap()
            .is_initializing;

        let (previous_value, deps_temp) = if initializing {
            (None, None)
        } else {
            let previous_value = self.get_person_property(person_id, property);
            if previous_value != value {
                self.remove_from_index_maybe(person_id, property);
            }

            (
                Some(previous_value),
                self.get_data_container(PeoplePlugin)
                    .unwrap()
                    .dependency_map
                    .borrow_mut()
                    .get_mut(&TypeId::of::<T>())
                    .map(std::mem::take),
            )
        };

        let mut dependency_event_callbacks = Vec::new();
        if let Some(mut deps) = deps_temp {
            // If there are dependencies, set up a bunch of callbacks with the
            // current value
            for dep in &mut deps {
                dep.dependency_changed(self, person_id, &mut dependency_event_callbacks);
            }

            // Put the dependency list back in
            let data_container = self.get_data_container(PeoplePlugin).unwrap();
            let mut dependencies = data_container.dependency_map.borrow_mut();
            dependencies.insert(TypeId::of::<T>(), deps);
        }

        // Update the main property and send a change event
        let data_container = self.get_data_container(PeoplePlugin).unwrap();
        data_container.set_person_property(person_id, property, value);

        if !initializing {
            if previous_value.unwrap() != value {
                self.add_to_index_maybe(person_id, property);
            }

            let change_event: PersonPropertyChangeEvent<T> = PersonPropertyChangeEvent {
                person_id,
                current: value,
                previous: previous_value.unwrap(), // This muse be Some() of !initializing
            };
            self.emit_event(change_event);
        }

        for callback in dependency_event_callbacks {
            callback(self);
        }
    }

    fn get_person_id(&self, person_id: usize) -> PersonId {
        assert!(
            person_id < self.get_current_population(),
            "Person does not exist"
        );
        PersonId { id: person_id }
    }

    fn index_property<T: PersonProperty + 'static>(&mut self, property: T) {
        // Ensure that the data container exists
        {
            let _ = self.get_data_container_mut(PeoplePlugin);
        }

        self.register_property::<T>();
        self.register_indexer::<T>();

        let data_container = self.get_data_container(PeoplePlugin).unwrap();
        let mut index = data_container.get_index_ref_mut_by_prop(property).unwrap();
        if index.lookup.is_none() {
            index.lookup = Some(HashMap::new());
        }
    }

    fn query_people<T: Query>(&self, q: T) -> Vec<PersonId> {
        // Special case the situation where nobody exists.
        if self.get_data_container(PeoplePlugin).is_none() {
            return Vec::new();
        }

        T::setup(self);
        self.query_people_internal(q.get_query())
    }

    fn match_person<T: Query>(&self, person_id: PersonId, q: T) -> bool {
        T::setup(&self);
        // This cannot fail because someone must have been made by now.
        let data_container = self.get_data_container(PeoplePlugin).unwrap();

        let query = q.get_query();

        for (t, hash) in &query {
            let index = data_container.get_index_ref(*t).unwrap();
            if *hash != (*index.indexer)(self, person_id) {
                return false;
            }
        }
        true
    }

    fn register_property<T: PersonProperty + 'static>(&self) {
        let data_container = self.get_data_container(PeoplePlugin).unwrap();
        if !data_container
            .registered_derived_properties
            .borrow()
            .contains(&TypeId::of::<T>())
        {
            let instance = T::get_instance();
            let dependencies = instance.non_derived_dependencies();
            for dependency in dependencies {
                let mut dependency_map = data_container.dependency_map.borrow_mut();
                let derived_prop_list = dependency_map.entry(dependency).or_default();
                derived_prop_list.push(Box::new(instance));
            }
            data_container
                .registered_derived_properties
                .borrow_mut()
                .insert(TypeId::of::<T>());
        }
    }
}

trait ContextPeopleExtInternal {
    fn register_indexer<T: PersonProperty + 'static>(&self);
    fn add_to_index_maybe<T: PersonProperty + 'static>(&mut self, person_id: PersonId, property: T);
    fn remove_from_index_maybe<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
    );
    fn query_people_internal(&self, property_hashes: Vec<(TypeId, IndexValue)>) -> Vec<PersonId>;
}

impl ContextPeopleExtInternal for Context {
    fn register_indexer<T: PersonProperty + 'static>(&self) {
        {
            let data_container = self.get_data_container(PeoplePlugin).unwrap();

            let property_indexes = data_container.property_indexes.borrow_mut();
            if property_indexes.contains_key(&TypeId::of::<T>()) {
                return; // Index already exists, do nothing
            }
        }

        // If it doesn't exist, insert the new index
        let index = Index::new(self, T::get_instance());
        let data_container = self.get_data_container(PeoplePlugin).unwrap();
        let mut property_indexes = data_container.property_indexes.borrow_mut();
        property_indexes.insert(TypeId::of::<T>(), index);
    }

    fn add_to_index_maybe<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
    ) {
        if let Some(mut index) = self
            .get_data_container(PeoplePlugin)
            .unwrap()
            .get_index_ref_mut_by_prop(property)
        {
            if index.lookup.is_some() {
                index.add_person(self, person_id);
            }
        }
    }

    fn remove_from_index_maybe<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
    ) {
        if let Some(mut index) = self
            .get_data_container(PeoplePlugin)
            .unwrap()
            .get_index_ref_mut_by_prop(property)
        {
            if index.lookup.is_some() {
                index.remove_person(self, person_id);
            }
        }
    }

    fn query_people_internal(&self, property_hashes: Vec<(TypeId, IndexValue)>) -> Vec<PersonId> {
        let mut indexes = Vec::<Ref<HashSet<PersonId>>>::new();
        let mut unindexed = Vec::<(TypeId, IndexValue)>::new();
        let data_container = self.get_data_container(PeoplePlugin)
            .expect("PeoplePlugin is not initialized; make sure you add a person before accessing properties");

        // 1. Walk through each property and update the indexes.
        for (t, _) in &property_hashes {
            let mut index = data_container.get_index_ref_mut(*t).unwrap();
            index.index_unindexed_people(self);
        }

        // 2. Collect the index entry corresponding to the value.
        for (t, hash) in property_hashes {
            let index = data_container.get_index_ref(t).unwrap();
            if let Ok(lookup) = Ref::filter_map(index, |x| x.lookup.as_ref()) {
                if let Ok(matching_people) = Ref::filter_map(lookup, |x| x.get(&hash)) {
                    indexes.push(matching_people);
                } else {
                    // This is empty and so the intersection will
                    // also be empty.
                    return Vec::new();
                }
            } else {
                // No index, so we'll get to this after.
                unindexed.push((t, hash));
            }
        }

        // 3. Create an iterator over people, based one either:
        //    (1) the smallest index if there is one.
        //    (2) the overall population if there are no indices.

        let holder: Ref<HashSet<PersonId>>;
        let to_check: Box<dyn Iterator<Item = PersonId>> = if indexes.is_empty() {
            Box::new(data_container.people_iterator())
        } else {
            indexes.sort_by_key(|x| x.len());

            holder = indexes.remove(0);
            Box::new(holder.iter().copied())
        };

        // 4. Walk over the iterator and add people to the result
        // iff:
        //    (1) they exist in all the indexes
        //    (2) they match the unindexed properties
        let mut result = Vec::<PersonId>::new();
        'outer: for person in to_check {
            // (1) check all the indexes
            for index in &indexes {
                if !index.contains(&person) {
                    continue 'outer;
                }
            }

            // (2) check the unindexed properties
            for (t, hash) in &unindexed {
                let index = data_container.get_index_ref(*t).unwrap();
                if *hash != (*index.indexer)(self, person) {
                    continue 'outer;
                }
            }

            // This matches.
            result.push(person);
        }

        result
    }
}

#[cfg(test)]
mod test {
    use super::{ContextPeopleExt, PersonCreatedEvent, PersonId, PersonPropertyChangeEvent};
    use crate::{
        context::Context,
        error::IxaError,
        people::{Index, IndexValue, PeoplePlugin, PersonPropertyHolder},
    };
    use std::{any::TypeId, cell::RefCell, rc::Rc};

    define_person_property!(Age, u8);
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub enum AgeGroupType {
        Child,
        Adult,
    }
    define_derived_property!(AgeGroup, AgeGroupType, [Age], |age| {
        if age < 18 {
            AgeGroupType::Child
        } else {
            AgeGroupType::Adult
        }
    });

    #[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
    pub enum RiskCategory {
        High,
        Low,
    }

    define_person_property!(RiskCategoryType, RiskCategory);
    define_person_property_with_default!(IsRunner, bool, false);
    define_person_property!(RunningShoes, u8, |context: &Context, person: PersonId| {
        let is_runner = context.get_person_property(person, IsRunner);
        if is_runner {
            4
        } else {
            0
        }
    });
    define_derived_property!(AdultRunner, bool, [IsRunner, Age], |is_runner, age| {
        is_runner && age >= 18
    });
    define_derived_property!(
        SeniorRunner,
        bool,
        [AdultRunner, Age],
        |adult_runner, age| { adult_runner && age >= 65 }
    );
    define_person_property_with_default!(IsSwimmer, bool, false);
    define_derived_property!(AdultSwimmer, bool, [IsSwimmer, Age], |is_swimmer, age| {
        is_swimmer && age >= 18
    });
    define_derived_property!(
        AdultAthlete,
        bool,
        [AdultRunner, AdultSwimmer],
        |adult_runner, adult_swimmer| { adult_runner || adult_swimmer }
    );

    #[test]
    fn observe_person_addition() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(move |_context, event: PersonCreatedEvent| {
            *flag_clone.borrow_mut() = true;
            assert_eq!(event.person_id.id, 0);
        });

        let _ = context.add_person(()).unwrap();
        context.execute();
        assert!(*flag.borrow());
    }
    #[test]
    fn set_get_properties() {
        let mut context = Context::new();

        let person = context.add_person((Age, 42)).unwrap();
        assert_eq!(context.get_person_property(person, Age), 42);
    }

    #[allow(clippy::should_panic_without_expect)]
    #[test]
    #[should_panic]
    fn get_uninitialized_property_panics() {
        let mut context = Context::new();
        let person = context.add_person(()).unwrap();
        context.get_person_property(person, Age);
    }

    #[test]
    fn get_current_population() {
        let mut context = Context::new();
        assert_eq!(context.get_current_population(), 0);
        for _ in 0..3 {
            context.add_person(()).unwrap();
        }
        assert_eq!(context.get_current_population(), 3);
    }

    #[test]
    fn add_person() {
        let mut context = Context::new();

        let person_id = context
            .add_person(((Age, 42), (RiskCategoryType, RiskCategory::Low)))
            .unwrap();
        assert_eq!(context.get_person_property(person_id, Age), 42);
        assert_eq!(
            context.get_person_property(person_id, RiskCategoryType),
            RiskCategory::Low
        );
    }

    #[test]
    fn add_person_with_initialize() {
        let mut context = Context::new();

        let person_id = context
            .add_person(((Age, 42), (RiskCategoryType, RiskCategory::Low)))
            .unwrap();
        assert_eq!(context.get_person_property(person_id, Age), 42);
        assert_eq!(
            context.get_person_property(person_id, RiskCategoryType),
            RiskCategory::Low
        );
    }

    #[test]
    fn add_person_with_initialize_missing() {
        let mut context = Context::new();

        context.add_person((Age, 10)).unwrap();
        // Fails because we don't provide a value for Age
        assert!(matches!(context.add_person(()), Err(IxaError::IxaError(_))));
    }

    #[test]
    fn add_person_with_initialize_missing_first() {
        let mut context = Context::new();

        // Succeeds because context doesn't know about any properties
        // yet.
        context.add_person(()).unwrap();
    }

    #[test]
    fn add_person_with_initialize_missing_with_default() {
        let mut context = Context::new();

        context.add_person((IsRunner, true)).unwrap();
        // Succeeds because |IsRunner| has a default.
        context.add_person(()).unwrap();
    }

    #[test]
    fn person_debug_display() {
        let mut context = Context::new();

        let person_id = context.add_person(()).unwrap();
        assert_eq!(format!("{person_id}"), "0");
        assert_eq!(format!("{person_id:?}"), "Person 0");
    }

    #[test]
    fn add_person_initializers() {
        let mut context = Context::new();
        let person_id = context.add_person(()).unwrap();

        assert_eq!(context.get_person_property(person_id, RunningShoes), 0);
        assert!(!context.get_person_property(person_id, IsRunner));
    }

    #[test]
    fn property_initialization_is_lazy() {
        let mut context = Context::new();
        let person = context.add_person((IsRunner, true)).unwrap();
        let people_data = context.get_data_container_mut(PeoplePlugin);

        // Verify we haven't initialized the property yet
        let has_value = *people_data.get_person_property_ref(person, RunningShoes);
        assert!(has_value.is_none());

        // This should initialize it
        let value = context.get_person_property(person, RunningShoes);
        assert_eq!(value, 4);
    }

    #[test]
    fn observe_person_property_change() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, event: PersonPropertyChangeEvent<RiskCategoryType>| {
                *flag_clone.borrow_mut() = true;
                assert_eq!(event.person_id.id, 0, "Person id is correct");
                assert_eq!(
                    event.previous,
                    RiskCategory::Low,
                    "Previous value is correct"
                );
                assert_eq!(
                    event.current,
                    RiskCategory::High,
                    "Current value is correct"
                );
            },
        );
        let person_id = context
            .add_person((RiskCategoryType, RiskCategory::Low))
            .unwrap();
        context.set_person_property(person_id, RiskCategoryType, RiskCategory::High);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn observe_person_property_change_with_set() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, _event: PersonPropertyChangeEvent<RunningShoes>| {
                *flag_clone.borrow_mut() = true;
            },
        );
        let person_id = context.add_person(()).unwrap();
        // Initializer called as a side effect of set, so event fires.
        context.set_person_property(person_id, RunningShoes, 42);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn initialize_without_initializer_succeeds() {
        let mut context = Context::new();
        context
            .add_person((RiskCategoryType, RiskCategory::High))
            .unwrap();
    }

    #[test]
    #[should_panic(expected = "Property not initialized when person created")]
    fn set_without_initializer_panics() {
        let mut context = Context::new();
        let person_id = context.add_person(()).unwrap();
        context.set_person_property(person_id, RiskCategoryType, RiskCategory::High);
    }

    #[test]
    #[should_panic(expected = "Property not initialized when person created")]
    fn get_without_initializer_panics() {
        let mut context = Context::new();
        let person_id = context.add_person(()).unwrap();
        context.get_person_property(person_id, RiskCategoryType);
    }

    #[test]
    #[should_panic(expected = "Person does not exist")]
    fn dont_return_person_id() {
        let mut context = Context::new();
        context.add_person(()).unwrap();
        context.get_person_id(1);
    }

    #[test]
    fn get_person_property_returns_correct_value() {
        let mut context = Context::new();
        let person = context.add_person((Age, 10)).unwrap();
        assert_eq!(
            context.get_person_property(person, AgeGroup),
            AgeGroupType::Child
        );
    }

    #[test]
    fn get_person_property_changes_correctly() {
        let mut context = Context::new();
        let person = context.add_person((Age, 17)).unwrap();
        assert_eq!(
            context.get_person_property(person, AgeGroup),
            AgeGroupType::Child
        );
        context.set_person_property(person, Age, 18);
        assert_eq!(
            context.get_person_property(person, AgeGroup),
            AgeGroupType::Adult
        );
    }
    #[test]
    fn get_person_property_change_event() {
        let mut context = Context::new();
        let person = context.add_person((Age, 17)).unwrap();

        let flag = Rc::new(RefCell::new(false));

        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, event: PersonPropertyChangeEvent<AgeGroup>| {
                assert_eq!(event.person_id.id, 0);
                assert_eq!(event.previous, AgeGroupType::Child);
                assert_eq!(event.current, AgeGroupType::Adult);
                *flag_clone.borrow_mut() = true;
            },
        );
        context.set_person_property(person, Age, 18);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn get_derived_property_multiple_deps() {
        let mut context = Context::new();
        let person = context.add_person(((Age, 17), (IsRunner, true))).unwrap();
        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, event: PersonPropertyChangeEvent<AdultRunner>| {
                assert_eq!(event.person_id.id, 0);
                assert!(!event.previous);
                assert!(event.current);
                *flag_clone.borrow_mut() = true;
            },
        );
        context.set_person_property(person, Age, 18);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn register_derived_only_once() {
        let mut context = Context::new();
        let person = context.add_person(((Age, 17), (IsRunner, true))).unwrap();

        let flag = Rc::new(RefCell::new(0));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, _event: PersonPropertyChangeEvent<AdultRunner>| {
                *flag_clone.borrow_mut() += 1;
            },
        );
        context.subscribe_to_event(
            move |_context, _event: PersonPropertyChangeEvent<AdultRunner>| {
                // Make sure that we don't register multiple times
            },
        );
        context.set_person_property(person, Age, 18);
        context.execute();
        assert_eq!(*flag.borrow(), 1);
    }

    #[test]
    fn test_resolve_dependencies() {
        let mut actual = SeniorRunner.non_derived_dependencies();
        let mut expected = vec![TypeId::of::<Age>(), TypeId::of::<IsRunner>()];
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
    }

    #[test]
    fn get_derived_property_dependent_on_another_derived() {
        let mut context = Context::new();
        let person = context.add_person(((Age, 88), (IsRunner, false))).unwrap();
        let flag = Rc::new(RefCell::new(0));
        let flag_clone = flag.clone();
        assert!(!context.get_person_property(person, SeniorRunner));
        context.subscribe_to_event(
            move |_context, event: PersonPropertyChangeEvent<SeniorRunner>| {
                assert_eq!(event.person_id.id, 0);
                assert!(!event.previous);
                assert!(event.current);
                *flag_clone.borrow_mut() += 1;
            },
        );
        context.set_person_property(person, IsRunner, true);
        context.execute();
        assert_eq!(*flag.borrow(), 1);
    }

    #[test]
    fn get_derived_property_diamond_dependencies() {
        let mut context = Context::new();
        let person = context.add_person(((Age, 17), (IsSwimmer, true))).unwrap();

        let flag = Rc::new(RefCell::new(0));
        let flag_clone = flag.clone();
        assert!(!context.get_person_property(person, AdultAthlete));
        context.subscribe_to_event(
            move |_context, event: PersonPropertyChangeEvent<AdultAthlete>| {
                assert_eq!(event.person_id.id, 0);
                assert!(!event.previous);
                assert!(event.current);
                *flag_clone.borrow_mut() += 1;
            },
        );
        context.set_person_property(person, Age, 18);
        context.execute();
        assert_eq!(*flag.borrow(), 1);
    }

    #[test]
    fn index_name() {
        let context = Context::new();
        let index = Index::new(&context, Age);
        assert!(index.name.contains("Age"));
    }

    #[test]
    fn query_people() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategoryType, RiskCategory::High))
            .unwrap();

        let people = context.query_people((RiskCategoryType, RiskCategory::High));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_empty() {
        let context = Context::new();

        let people = context.query_people((RiskCategoryType, RiskCategory::High));
        assert_eq!(people.len(), 0);
    }

    #[test]
    fn query_people_macro_index_first() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategoryType, RiskCategory::High))
            .unwrap();
        context.index_property(RiskCategoryType);
        assert!(property_is_indexed::<RiskCategoryType>(&context));
        let people = context.query_people((RiskCategoryType, RiskCategory::High));
        assert_eq!(people.len(), 1);
    }

    fn property_is_indexed<T: 'static>(context: &Context) -> bool {
        context
            .get_data_container(PeoplePlugin)
            .unwrap()
            .get_index_ref(TypeId::of::<T>())
            .unwrap()
            .lookup
            .is_some()
    }

    #[test]
    fn query_people_macro_index_second() {
        let mut context = Context::new();
        let _ = context.add_person((RiskCategoryType, RiskCategory::High));
        let people = context.query_people((RiskCategoryType, RiskCategory::High));
        assert!(!property_is_indexed::<RiskCategoryType>(&context));
        assert_eq!(people.len(), 1);
        context.index_property(RiskCategoryType);
        assert!(property_is_indexed::<RiskCategoryType>(&context));
        let people = context.query_people((RiskCategoryType, RiskCategory::High));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_macro_change() {
        let mut context = Context::new();
        let person1 = context
            .add_person((RiskCategoryType, RiskCategory::High))
            .unwrap();

        let people = context.query_people((RiskCategoryType, RiskCategory::High));
        assert_eq!(people.len(), 1);
        let people = context.query_people((RiskCategoryType, RiskCategory::Low));
        assert_eq!(people.len(), 0);

        context.set_person_property(person1, RiskCategoryType, RiskCategory::Low);
        let people = context.query_people((RiskCategoryType, RiskCategory::High));
        assert_eq!(people.len(), 0);
        let people = context.query_people((RiskCategoryType, RiskCategory::Low));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_index_after_add() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategoryType, RiskCategory::High))
            .unwrap();
        context.index_property(RiskCategoryType);
        assert!(property_is_indexed::<RiskCategoryType>(&context));
        let people = context.query_people((RiskCategoryType, RiskCategory::High));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_add_after_index() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategoryType, RiskCategory::High))
            .unwrap();
        context.index_property(RiskCategoryType);
        assert!(property_is_indexed::<RiskCategoryType>(&context));
        let people = context.query_people((RiskCategoryType, RiskCategory::High));
        assert_eq!(people.len(), 1);

        let _ = context
            .add_person((RiskCategoryType, RiskCategory::High))
            .unwrap();
        let people = context.query_people((RiskCategoryType, RiskCategory::High));
        assert_eq!(people.len(), 2);
    }

    #[test]
    // This is safe because we reindex only when someone queries.
    fn query_people_add_after_index_without_query() {
        let mut context = Context::new();
        let _ = context.add_person(()).unwrap();
        context.index_property(RiskCategoryType);
    }

    #[test]
    #[should_panic(expected = "Property not initialized")]
    // This will panic when we query.
    fn query_people_add_after_index_panic() {
        let mut context = Context::new();
        context.add_person(()).unwrap();
        context.index_property(RiskCategoryType);
        context.query_people((RiskCategoryType, RiskCategory::High));
    }

    #[test]
    fn query_people_cast_value() {
        let mut context = Context::new();
        let _ = context.add_person((Age, 42)).unwrap();

        // Age is a u8, by default integer literals are i32; the macro should cast it.
        let people = context.query_people((Age, 42));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_intersection() {
        let mut context = Context::new();
        let _ = context
            .add_person(((Age, 42), (RiskCategoryType, RiskCategory::High)))
            .unwrap();
        let _ = context
            .add_person(((Age, 42), (RiskCategoryType, RiskCategory::Low)))
            .unwrap();
        let _ = context
            .add_person(((Age, 40), (RiskCategoryType, RiskCategory::Low)))
            .unwrap();

        let people = context.query_people(((Age, 42), (RiskCategoryType, RiskCategory::High)));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_intersection_non_macro() {
        let mut context = Context::new();
        let _ = context
            .add_person(((Age, 42), (RiskCategoryType, RiskCategory::High)))
            .unwrap();
        let _ = context
            .add_person(((Age, 42), (RiskCategoryType, RiskCategory::Low)))
            .unwrap();
        let _ = context
            .add_person(((Age, 40), (RiskCategoryType, RiskCategory::Low)))
            .unwrap();

        let people = context.query_people(((Age, 42), (RiskCategoryType, RiskCategory::High)));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_intersection_one_indexed() {
        let mut context = Context::new();
        let _ = context
            .add_person(((Age, 42), (RiskCategoryType, RiskCategory::High)))
            .unwrap();
        let _ = context
            .add_person(((Age, 42), (RiskCategoryType, RiskCategory::Low)))
            .unwrap();
        let _ = context
            .add_person(((Age, 40), (RiskCategoryType, RiskCategory::Low)))
            .unwrap();

        context.index_property(Age);
        let people = context.query_people(((Age, 42), (RiskCategoryType, RiskCategory::High)));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_derived_prop() {
        let mut context = Context::new();
        define_derived_property!(Senior, bool, [Age], |age| age >= 65);

        let person = context.add_person((Age, 64)).unwrap();
        let _ = context.add_person((Age, 88)).unwrap();

        // Age is a u8, by default integer literals are i32; the macro should cast it.
        let not_seniors = context.query_people((Senior, false));
        let seniors = context.query_people((Senior, true));
        assert_eq!(seniors.len(), 1, "One senior");
        assert_eq!(not_seniors.len(), 1, "One non-senior");

        context.set_person_property(person, Age, 65);

        let not_seniors = context.query_people((Senior, false));
        let seniors = context.query_people((Senior, true));

        assert_eq!(seniors.len(), 2, "Two seniors");
        assert_eq!(not_seniors.len(), 0, "No non-seniors");
    }

    #[test]
    fn query_derived_prop_with_index() {
        let mut context = Context::new();
        define_derived_property!(Senior, bool, [Age], |age| age >= 65);

        context.index_property(Senior);
        let person = context.add_person((Age, 64)).unwrap();
        let _ = context.add_person((Age, 88)).unwrap();

        // Age is a u8, by default integer literals are i32; the macro should cast it.
        let not_seniors = context.query_people((Senior, false));
        let seniors = context.query_people((Senior, true));
        assert_eq!(seniors.len(), 1, "One senior");
        assert_eq!(not_seniors.len(), 1, "One non-senior");

        context.set_person_property(person, Age, 65);

        let not_seniors = context.query_people((Senior, false));
        let seniors = context.query_people((Senior, true));

        assert_eq!(seniors.len(), 2, "Two seniors");
        assert_eq!(not_seniors.len(), 0, "No non-seniors");
    }

    #[test]
    fn text_match_person() {
        let mut context = Context::new();
        let person = context
            .add_person(((Age, 42), (RiskCategoryType, RiskCategory::High)))
            .unwrap();
        assert!(context.match_person(person, ((Age, 42), (RiskCategoryType, RiskCategory::High))));
        assert!(!context.match_person(person, ((Age, 43), (RiskCategoryType, RiskCategory::High))));
        assert!(!context.match_person(person, ((Age, 42), (RiskCategoryType, RiskCategory::Low))));
    }

    #[test]
    fn test_index_value_hasher_finish2_short() {
        let value = 42;
        let index = IndexValue::compute(&value);
        assert!(matches!(index, IndexValue::Fixed(_)));
    }

    #[test]
    fn test_index_value_hasher_finish2_long() {
        let value = "this is a longer string that exceeds 16 bytes";
        let index = IndexValue::compute(&value);
        assert!(matches!(index, IndexValue::Variable(_)));
    }

    #[test]
    fn test_index_value_compute_same_values() {
        let value = "test value";
        let value2 = "test value";
        assert_eq!(IndexValue::compute(&value), IndexValue::compute(&value2));
    }

    #[test]
    fn test_index_value_compute_different_values() {
        let value1 = 42;
        let value2 = 43;
        assert_ne!(IndexValue::compute(&value1), IndexValue::compute(&value2));
    }
}
