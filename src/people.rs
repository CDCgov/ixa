use crate::{
    context::{Context, IxaEvent},
    define_data_plugin,
};
use ixa_derive::IxaEvent;
use serde::{Deserialize, Serialize};
use std::{
    any::{Any, TypeId},
    borrow::BorrowMut,
    cell::{RefCell, RefMut},
    collections::{HashMap, HashSet},
    fmt::{self, Debug},
    hash::{Hash, Hasher},
    iter::Iterator,
};

type Indexer = dyn FnMut(&Context, PersonId) -> IndexValue;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum IndexValue {
    Fixed(u128),
    Variable(Vec<u8>)
}

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

impl IndexValue {
    pub fn compute<T: Hash>(val: &T) -> IndexValue {
        let mut hasher = IndexValueHasher::new();
        val.hash(&mut hasher);
        if hasher.buf.len() <= 16 {
            let mut tmp: [u8; 16] = [0; 16];
            tmp[..hasher.buf.len()].copy_from_slice(&hasher.buf[..]);
            return IndexValue::Fixed(u128::from_le_bytes(tmp))
        }
        IndexValue::Variable(hasher.buf)
    }
}

struct Index {
    // Primarily for debugging purposes
    #[allow(dead_code)]
    name: &'static str,
    // The hash of the property value maps to a list of PersonIds
    // or None if we're not indexing
    lookup: Option<HashMap<IndexValue, HashSet<PersonId>>>,
    // A callback that calculates the hash of a person's current property value
    indexer: Box<Indexer>,
    // The largest person ID that has been indexed.
    max_indexed: usize
}

impl Index {
    fn new<T: PersonProperty + 'static>(_context: &Context, property: T) -> Self {
        let index = Self {
            name: std::any::type_name::<T>(),
            lookup: None,
            indexer: Box::new(move |context: &Context, person_id: PersonId| {
                let value = context.get_person_property(person_id, property);
                IndexValue::compute(&value)
            }),
            max_indexed: 0,
        };
        index
    }
    fn add_index(&mut self, context: &Context, person_id: PersonId) {
        let hash = (self.indexer)(context, person_id);
        self.lookup
            .as_mut()
            .unwrap()
            .entry(hash)
            .or_default()
            .insert(person_id);
    }
    fn remove_index(&mut self, context: &Context, person_id: PersonId) {
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
            self.add_index(context, person_id);
        }
        self.max_indexed = current_pop;
    }
}

// PeopleData represents each unique person in the simulation with an id ranging
// from 0 to population - 1. Person properties are associated with a person
// via their id.
struct PeopleData {
    current_population: usize,
    properties_map: RefCell<HashMap<TypeId, Box<dyn Any>>>,
    registered_derived_properties: RefCell<HashSet<TypeId>>,
    dependency_map: RefCell<HashMap<TypeId, Vec<Box<dyn PersonPropertyHolder>>>>,
    property_indexes: RefCell<HashMap<TypeId, Index>>,
}

define_data_plugin!(
    PeoplePlugin,
    PeopleData,
    PeopleData {
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
    id: usize,
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
    fn dependencies() -> Vec<Box<dyn PersonPropertyHolder>> {
        panic!("Dependencies not implemented");
    }
    fn compute(context: &Context, person_id: PersonId) -> Self::Value;
    fn get_instance() -> Self;
}

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
        define_person_property!($person_property, $value, |_context, _person_id| {
            panic!("Property not initialized");
        });
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
                .or_insert_with(|| Box::new(Vec::<Option<T::Value>>::with_capacity(index)));
            let values: &mut Vec<Option<T::Value>> = properties
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

    fn get_index_ref(&self, t: TypeId) -> Option<RefMut<Index>> {
        let index_map = self.property_indexes.borrow_mut();
        if index_map.contains_key(&t) {
            Some(RefMut::map(index_map, |map| map.get_mut(&t).unwrap()))
        } else {
            None
        }
    }

    fn get_index_ref_by_prop<T: PersonProperty + 'static>(
        &self,
        _property: T,
    ) -> Option<RefMut<Index>> {
        let type_id = TypeId::of::<T>();
        self.get_index_ref(type_id)
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

    /// Creates a new person with no assigned person properties
    fn add_person(&mut self) -> PersonId;

    /// Given a `PersonId` returns the value of a defined person property,
    /// initializing it if it hasn't been set yet. If no initializer is
    /// provided, and the property is not set this will panic
    fn get_person_property<T: PersonProperty + 'static>(
        &self,
        person_id: PersonId,
        _property: T,
    ) -> T::Value;

    fn register_property<T: PersonProperty + 'static>(&mut self);

    /// Given a `PersonId`, initialize the value of a defined person property.
    /// Once the the value is set using this API, any initializer will
    /// not run.
    /// Panics if the property is already initialized. Does not fire a change
    /// event.
    fn initialize_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        _property: T,
        value: T::Value,
    );

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
    fn register_indexer<T: PersonProperty + 'static>(&mut self, property: T);
    fn index_property<T: PersonProperty + 'static>(&mut self, property: T);
    fn query_people(&self, property_hashes: Vec<(TypeId, IndexValue)>) -> Vec<PersonId>;
}

impl ContextPeopleExt for Context {
    fn get_current_population(&self) -> usize {
        self.get_data_container(PeoplePlugin)
            .map_or(0, |data_container| data_container.current_population)
    }

    fn add_person(&mut self) -> PersonId {
        let person_id = self.get_data_container_mut(PeoplePlugin).add_person();
        self.add_person_to_indexes(person_id);
        self.emit_event(PersonCreatedEvent { person_id });
        person_id
    }

    fn register_property<T: PersonProperty + 'static>(&mut self) {
        let data_container = self.get_data_container(PeoplePlugin)
            .expect("PeoplePlugin is not initialized; make sure you add a person before accessing properties");
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

    fn initialize_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        assert!(!T::is_derived(), "Cannot initialize a derived property");
        let data_container = self.get_data_container(PeoplePlugin)
            .expect("PeoplePlugin is not initialized; make sure you add a person before accessing properties");
        
        let current_value = *data_container.get_person_property_ref(person_id, property);
        assert!(current_value.is_none(), "Property already initialized");
        data_container.set_person_property(person_id, property, value);
        self.add_to_index_maybe(person_id, property);
    }

    #[allow(clippy::single_match_else)]
    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        assert!(!T::is_derived(), "Cannot set a derived property");
        let previous_value = self.get_person_property(person_id, property);

        if previous_value != value {
            self.remove_from_index_maybe(person_id, property);
        }

        // Temporarily remove dependency properties since we need mutable references
        // to self during callback execution
        let deps_temp = {
            self.get_data_container(PeoplePlugin)
                .unwrap()
                .dependency_map
                .borrow_mut()
                .get_mut(&TypeId::of::<T>())
                .map(std::mem::take)
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
        if previous_value != value {
            self.add_to_index_maybe(person_id, property);
        }

        let change_event: PersonPropertyChangeEvent<T> = PersonPropertyChangeEvent {
            person_id,
            current: value,
            previous: previous_value,
        };
        self.emit_event(change_event);

        for callback in dependency_event_callbacks {
            callback(self);
        }
    }

    fn get_person_id(&self, person_id: usize) -> PersonId {
        if person_id >= self.get_current_population() {
            panic!("Person does not exist");
        }
        PersonId { id: person_id }
    }

    fn register_indexer<T: PersonProperty + 'static>(&mut self, property: T) {
        {
            let data_container = self.get_data_container_mut(PeoplePlugin);
            let property_indexes = data_container.property_indexes.borrow_mut();
            if property_indexes.contains_key(&TypeId::of::<T>()) {
                return; // Index already exists, do nothing
            }
        }

        // If it doesn't exist, insert the new index
        let index = Index::new(self, property);
        let data_container = self.get_data_container_mut(PeoplePlugin);
        let mut property_indexes = data_container.property_indexes.borrow_mut();
        property_indexes.insert(TypeId::of::<T>(), index);
    }

    fn index_property<T: PersonProperty + 'static>(&mut self, property: T) {
        self.register_indexer(property);

        let data_container = self.get_data_container(PeoplePlugin)
            .expect("PeoplePlugin is not initialized; make sure you add a person before accessing properties");

        let mut index = data_container.get_index_ref_by_prop(property).unwrap();
        if index.lookup.is_none() {
            index.lookup = Some(HashMap::new());
        }
    }

    fn query_people(&self, property_hashes: Vec<(TypeId, IndexValue)>) -> Vec<PersonId> {
        let mut indexes = Vec::<HashSet<PersonId>>::new();
        let mut unindexed = Vec::<(TypeId, IndexValue)>::new();
        let data_container = self.get_data_container(PeoplePlugin)
            .expect("PeoplePlugin is not initialized; make sure you add a person before accessing properties");

        // 1. Walk through each property and collect the index entry
        // corresponding to the value.
        for (t, hash) in property_hashes.into_iter() {
            let mut index = data_container.get_index_ref(t).unwrap();
            index.index_unindexed_people(self);
            
            // Update the index.
            if let Some(lookup) = &index.borrow_mut().lookup {
                if let Some(matching_people) = lookup.get(&hash) {
                    indexes.push(matching_people.clone()); // UGH
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

        // 2. Create an iterator over people, based one either:
        //    (1) the smallest index if there is one.
        //    (2) the overall population if there are no indices.

        let holder: HashSet<PersonId>;
        let to_check: Box<dyn Iterator<Item = PersonId>> = if indexes.len() != 0 {
            indexes.sort_by_key(|x| x.len());

            holder = indexes.remove(0);
            Box::new(holder.iter().cloned())
        } else {
            Box::new(data_container.people_iterator())
        };

        // 3. Walk over the iterator and add people to the result
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
                let mut index = data_container.get_index_ref(*t).unwrap();
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

trait ContextPeopleExtInternal {
    fn add_to_index_maybe<T: PersonProperty + 'static>(&mut self, person_id: PersonId, property: T);
    fn add_person_to_indexes(&mut self, person_id: PersonId);
    fn remove_from_index_maybe<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
    );
}

impl ContextPeopleExtInternal for Context {
    fn add_to_index_maybe<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
    ) {
        if let Some(mut index) = self
            .get_data_container(PeoplePlugin)
            .unwrap()
            .get_index_ref_by_prop(property)
        {
            if index.lookup.is_some() {
                index.add_index(self, person_id);
            }
        }
    }
    fn add_person_to_indexes(&mut self, person_id: PersonId) {
        let data_container = self.get_data_container(PeoplePlugin).unwrap();
        for (_, index) in (data_container.property_indexes.borrow_mut()).iter_mut() {
            if index.lookup.is_some() {
                index.add_index(self, person_id);
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
            .get_index_ref_by_prop(property)
        {
            if index.lookup.is_some() {
                index.remove_index(self, person_id);
            }
        }
    }
}

#[allow(clippy::module_name_repetitions)]
#[macro_export]
macro_rules! people_query {
    ( $ctx: expr, $( [ $k:ident = $v: expr ] ),* ) => {
        {
            // Set up any indexes that don't exist yet
            $(
                if <$k as $crate::people::PersonProperty>::is_derived() {
                    $ctx.register_property::<$k>();
                }
                $ctx.register_indexer($k);
            )*
            // Do the query
            $ctx.query_people(vec![
                $((
                    std::any::TypeId::of::<$k>(),
                    crate::people::IndexValue::compute(&($v as <$k as $crate::people::PersonProperty>::Value))
                ),)*
            ])
        }
    }
}

#[cfg(test)]
mod test {
    use super::{ContextPeopleExt, PersonCreatedEvent, PersonId, PersonPropertyChangeEvent};
    use crate::{
        context::Context,
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

        let _ = context.add_person();
        context.execute();
        assert!(*flag.borrow());
    }
    #[test]
    fn set_get_properties() {
        let mut context = Context::new();

        let person = context.add_person();
        context.initialize_person_property(person, Age, 42);
        assert_eq!(context.get_person_property(person, Age), 42);
    }

    #[allow(clippy::should_panic_without_expect)]
    #[test]
    #[should_panic]
    fn get_uninitialized_property_panics() {
        let mut context = Context::new();
        let person = context.add_person();
        context.get_person_property(person, Age);
    }

    // Tests that if we try to set or access a property for an index greater than
    // the current size of the property Vec, the vector will be resized.
    #[test]
    fn set_property_resize() {
        let mut context = Context::new();

        // Add a person and set a property, instantiating the Vec
        let person = context.add_person();
        context.initialize_person_property(person, Age, 8);

        // Create a bunch of people and don't initialize Age
        for _ in 1..9 {
            let _person = context.add_person();
        }
        let tenth_person = context.add_person();

        // Set a person property for a person > index 0
        context.initialize_person_property(tenth_person, Age, 42);
        // Call an initializer
        assert!(!context.get_person_property(tenth_person, IsRunner));
    }

    #[test]
    fn get_current_population() {
        let mut context = Context::new();
        assert_eq!(context.get_current_population(), 0);
        for _ in 0..3 {
            context.add_person();
        }
        assert_eq!(context.get_current_population(), 3);
    }

    #[test]
    fn add_person() {
        let mut context = Context::new();

        let person_id = context.add_person();
        context.initialize_person_property(person_id, Age, 42);
        context.initialize_person_property(person_id, RiskCategoryType, RiskCategory::Low);
        assert_eq!(context.get_person_property(person_id, Age), 42);
        assert_eq!(
            context.get_person_property(person_id, RiskCategoryType),
            RiskCategory::Low
        );
    }

    #[test]
    fn person_debug_display() {
        let mut context = Context::new();

        let person_id = context.add_person();
        assert_eq!(format!("{person_id}"), "0");
        assert_eq!(format!("{person_id:?}"), "Person 0");
    }

    #[test]
    fn add_person_initializers() {
        let mut context = Context::new();
        let person_id = context.add_person();

        assert_eq!(context.get_person_property(person_id, RunningShoes), 0);
        assert!(!context.get_person_property(person_id, IsRunner));
    }

    #[test]
    fn property_initialization_should_not_emit_events() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, _event: PersonPropertyChangeEvent<RiskCategoryType>| {
                *flag_clone.borrow_mut() = true;
            },
        );

        let person = context.add_person();
        // This should not emit a change event
        context.initialize_person_property(person, Age, 42);
        context.get_person_property(person, IsRunner);
        context.get_person_property(person, RunningShoes);

        context.execute();

        assert!(!*flag.borrow());
    }

    #[test]
    fn property_initialization_is_lazy() {
        let mut context = Context::new();
        let person = context.add_person();
        let people_data = context.get_data_container_mut(PeoplePlugin);

        // Verify we haven't initialized the property yet
        let has_value = *people_data.get_person_property_ref(person, RunningShoes);
        assert!(has_value.is_none());

        context.initialize_person_property(person, IsRunner, true);

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
        let person_id = context.add_person();
        context.initialize_person_property(person_id, RiskCategoryType, RiskCategory::Low);
        context.set_person_property(person_id, RiskCategoryType, RiskCategory::High);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn observe_person_property_change_with_initializer() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, _event: PersonPropertyChangeEvent<RunningShoes>| {
                *flag_clone.borrow_mut() = true;
            },
        );
        let person_id = context.add_person();
        // Initializer wasn't called, so don't fire an event
        context.initialize_person_property(person_id, RunningShoes, 42);
        context.execute();
        assert!(!*flag.borrow());
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
        let person_id = context.add_person();
        // Initializer called as a side effect of set, so event fires.
        context.set_person_property(person_id, RunningShoes, 42);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    #[should_panic(expected = "Property already initialized")]
    fn calling_initialize_twice_panics() {
        let mut context = Context::new();
        let person_id = context.add_person();
        context.initialize_person_property(person_id, IsRunner, true);
        context.initialize_person_property(person_id, IsRunner, true);
    }

    #[test]
    #[should_panic(expected = "Property already initialized")]
    fn calling_initialize_after_get_panics() {
        let mut context = Context::new();
        let person_id = context.add_person();
        let _ = context.get_person_property(person_id, IsRunner);
        context.initialize_person_property(person_id, IsRunner, true);
    }

    #[test]
    fn initialize_without_initializer_succeeds() {
        let mut context = Context::new();
        let person_id = context.add_person();
        context.initialize_person_property(person_id, RiskCategoryType, RiskCategory::High);
    }

    #[test]
    #[should_panic(expected = "Property not initialized")]
    fn set_without_initializer_panics() {
        let mut context = Context::new();
        let person_id = context.add_person();
        context.set_person_property(person_id, RiskCategoryType, RiskCategory::High);
    }

    #[test]
    #[should_panic(expected = "Person does not exist")]
    fn dont_return_person_id() {
        let mut context = Context::new();
        context.add_person();
        context.get_person_id(1);
    }

    #[test]
    fn get_person_property_returns_correct_value() {
        let mut context = Context::new();
        let person = context.add_person();
        context.initialize_person_property(person, Age, 10);
        assert_eq!(
            context.get_person_property(person, AgeGroup),
            AgeGroupType::Child
        );
    }

    #[test]
    fn get_person_property_changes_correctly() {
        let mut context = Context::new();
        let person = context.add_person();
        context.initialize_person_property(person, Age, 17);
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
        let person = context.add_person();
        context.initialize_person_property(person, Age, 17);

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
        let person = context.add_person();
        context.initialize_person_property(person, Age, 17);
        context.initialize_person_property(person, IsRunner, true);

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
        let person = context.add_person();
        context.initialize_person_property(person, Age, 17);
        context.initialize_person_property(person, IsRunner, true);

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
        let person = context.add_person();
        context.initialize_person_property(person, Age, 88);
        context.initialize_person_property(person, IsRunner, false);

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
        let person = context.add_person();
        context.initialize_person_property(person, Age, 17);
        context.initialize_person_property(person, IsSwimmer, true);

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
    fn query_people_manual() {
        let mut context = Context::new();
        let person1 = context.add_person();
        let person2 = context.add_person();
        let person3 = context.add_person();

        context.initialize_person_property(person1, Age, 42);
        context.initialize_person_property(person2, Age, 40);
        context.initialize_person_property(person3, Age, 41);

        context.register_indexer(Age);
        let hash = IndexValue::compute(&context.get_person_property(person1, Age));
        let people = context.query_people(vec![(TypeId::of::<Age>(), hash)]);
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_macro() {
        let mut context = Context::new();
        let person1 = context.add_person();

        context.initialize_person_property(person1, RiskCategoryType, RiskCategory::High);

        let people = people_query!(context, [RiskCategoryType = RiskCategory::High]);
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_macro_index_first() {
        let mut context = Context::new();
        let person1 = context.add_person();

        context.initialize_person_property(person1, RiskCategoryType, RiskCategory::High);
        context.index_property(RiskCategoryType);
        assert!(property_is_indexed::<RiskCategoryType>(&context));
        let people = people_query!(context, [RiskCategoryType = RiskCategory::High]);
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
        let person1 = context.add_person();

        context.initialize_person_property(person1, RiskCategoryType, RiskCategory::High);
        let people = people_query!(context, [RiskCategoryType = RiskCategory::High]);
        assert!(!property_is_indexed::<RiskCategoryType>(&context));
        assert_eq!(people.len(), 1);
        context.index_property(RiskCategoryType);
        assert!(property_is_indexed::<RiskCategoryType>(&context));
        let people = people_query!(context, [RiskCategoryType = RiskCategory::High]);
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_macro_change() {
        let mut context = Context::new();
        let person1 = context.add_person();

        context.initialize_person_property(person1, RiskCategoryType, RiskCategory::High);

        let people = people_query!(context, [RiskCategoryType = RiskCategory::High]);
        assert_eq!(people.len(), 1);
        let people = people_query!(context, [RiskCategoryType = RiskCategory::Low]);
        assert_eq!(people.len(), 0);

        context.set_person_property(person1, RiskCategoryType, RiskCategory::Low);
        let people = people_query!(context, [RiskCategoryType = RiskCategory::High]);
        assert_eq!(people.len(), 0);
        let people = people_query!(context, [RiskCategoryType = RiskCategory::Low]);
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_index_after_add() {
        let mut context = Context::new();
        let person1 = context.add_person();
        context.initialize_person_property(person1, RiskCategoryType, RiskCategory::High);
        context.index_property(RiskCategoryType);
        assert!(property_is_indexed::<RiskCategoryType>(&context));
        let people = people_query!(context, [RiskCategoryType = RiskCategory::High]);
        assert_eq!(people.len(), 1);
    }

    
    #[test]
    fn query_people_cast_value() {
        let mut context = Context::new();
        let person = context.add_person();

        context.initialize_person_property(person, Age, 42);

        // Age is a u8, by default integer literals are i32; the macro should cast it.
        let people = people_query![context, [Age = 42]];
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_intersection() {
        let mut context = Context::new();
        let person1 = context.add_person();
        let person2 = context.add_person();
        let person3 = context.add_person();

        // Note: because of the way indexes are initialized, all properties without initializers need to be
        // set for all people.
        context.initialize_person_property(person1, Age, 42);
        context.initialize_person_property(person1, RiskCategoryType, RiskCategory::High);
        context.initialize_person_property(person2, Age, 42);
        context.initialize_person_property(person2, RiskCategoryType, RiskCategory::Low);
        context.initialize_person_property(person3, Age, 40);
        context.initialize_person_property(person3, RiskCategoryType, RiskCategory::Low);

        let people = people_query![context, [Age = 42], [RiskCategoryType = RiskCategory::High]];
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_intersection_one_indexed() {
        let mut context = Context::new();
        let person1 = context.add_person();
        let person2 = context.add_person();
        let person3 = context.add_person();

        // Note: because of the way indexes are initialized, all properties without initializers need to be
        // set for all people.
        context.initialize_person_property(person1, Age, 42);
        context.initialize_person_property(person1, RiskCategoryType, RiskCategory::High);
        context.initialize_person_property(person2, Age, 42);
        context.initialize_person_property(person2, RiskCategoryType, RiskCategory::Low);
        context.initialize_person_property(person3, Age, 40);
        context.initialize_person_property(person3, RiskCategoryType, RiskCategory::Low);

        context.index_property(Age);
        let people = people_query![context, [Age = 42], [RiskCategoryType = RiskCategory::High]];
        assert_eq!(people.len(), 1);

    }

    #[test]
    fn query_derived_prop() {
        let mut context = Context::new();
        define_derived_property!(Senior, bool, [Age], |age| age >= 65);

        let person = context.add_person();
        let person2 = context.add_person();

        context.initialize_person_property(person, Age, 64);
        context.initialize_person_property(person2, Age, 88);

        // Age is a u8, by default integer literals are i32; the macro should cast it.
        let not_seniors = people_query![context, [Senior = false]];
        let seniors = people_query![context, [Senior = true]];
        assert_eq!(seniors.len(), 1, "One senior");
        assert_eq!(not_seniors.len(), 1, "One non-senior");

        context.set_person_property(person, Age, 65);

        let not_seniors = people_query![context, [Senior = false]];
        let seniors = people_query![context, [Senior = true]];

        assert_eq!(seniors.len(), 2, "Two seniors");
        assert_eq!(not_seniors.len(), 0, "No non-seniors");
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
        assert_ne!(IndexValue::compute(&value1),
                   IndexValue::compute(&value2));
    }
    
}
