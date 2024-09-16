use crate::{
    context::Context, define_data_plugin, indexset_person_container::IndexSetPersonContainer,
};
use std::{
    any::{Any, TypeId},
    cell::{RefCell, RefMut},
    collections::HashMap,
    hash::{Hash, Hasher},
};

// PeopleData represents each unique person in the simulation with an id ranging
// from 0 to population - 1. Person properties are associated with a person
// via their id.
struct PeopleData {
    current_population: usize,
    properties_map: RefCell<HashMap<TypeId, Box<dyn Any>>>,
    index_data_map: HashMap<Vec<TypeId>, IndexData>,
    index_sensitivities: HashMap<TypeId, Vec<Vec<TypeId>>>,
}

define_data_plugin!(
    PeoplePlugin,
    PeopleData,
    PeopleData {
        current_population: 0,
        properties_map: RefCell::new(HashMap::new()),
        index_data_map: HashMap::new(),
        index_sensitivities: HashMap::new()
    }
);

// Represents a unique person - the id refers to that person's index in the range
// 0 to population - 1 in the PeopleData container.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct PersonId {
    pub id: usize,
}

// Individual characteristics or states related to a person, such as age or
// disease status, are represented as "person properties". These properties
// * are represented by a struct type that implements the PersonProperty trait,
// * specify a Value type to represent the data associated with the property,
// * specify an initializer, which returns the initial value
// They may be defined with the define_person_property! macro.
pub trait PersonProperty: Copy {
    type Value: Copy;
    fn initialize(context: &Context, person_id: PersonId) -> Self::Value;
}

/// Defines a person property with the following parameters:
/// * `$person_property`: A name for the identifier type of the property
/// * `$value`: The type of the property's value
/// * `$default`: (Optional) A default value
#[macro_export]
macro_rules! define_person_property {
    ($person_property:ident, $value:ty, $default: expr) => {
        #[derive(Copy, Clone)]
        pub struct $person_property;

        impl $crate::people::PersonProperty for $person_property {
            type Value = $value;
            fn initialize(
                _context: &$crate::context::Context,
                _person_id: $crate::people::PersonId,
            ) -> Self::Value {
                $default
            }
        }
    };
    ($person_property:ident, $value:ty) => {
        #[derive(Copy, Clone)]
        pub struct $person_property;

        impl $crate::people::PersonProperty for $person_property {
            type Value = $value;
            fn initialize(
                _context: &$crate::context::Context,
                _person_id: $crate::people::PersonId,
            ) -> Self::Value {
                panic!("Property not initialized");
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
        let properies_map = self.properties_map.borrow_mut();
        let index = person.id;
        RefMut::map(properies_map, |properties_map| {
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
}

// Emitted when a new person is created
// These should not be emitted outside this module
#[derive(Clone, Copy)]
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

// Object safe hash trait
trait DynHash {
    fn dyn_hash(&self, state: &mut dyn Hasher);
}

impl<T: Hash + ?Sized> DynHash for T {
    fn dyn_hash(&self, mut state: &mut dyn Hasher) {
        self.hash(&mut state);
    }
}

// Object safe equality trait
trait DynEq {
    fn dyn_eq(&self, other: &dyn Any) -> bool;
}

impl<T: Eq + Any> DynEq for T {
    fn dyn_eq(&self, other: &dyn Any) -> bool {
        if let Some(other) = other.downcast_ref::<Self>() {
            self == other
        } else {
            false
        }
    }
}

// Object safe trait for Any + Hash + Eq
trait AnyHashEq: Any + DynHash + DynEq {
    fn as_any(&self) -> &dyn Any;
}

impl Hash for dyn AnyHashEq {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.dyn_hash(state);
    }
}

impl PartialEq for dyn AnyHashEq {
    fn eq(&self, other: &Self) -> bool {
        self.dyn_eq(other.as_any())
    }
}

impl Eq for dyn AnyHashEq {}

impl<T> AnyHashEq for T
where
    T: Any + Hash + Eq,
{
    fn as_any(&self) -> &dyn Any {
        self
    }
}

type PropertySetter = dyn Fn(&mut Vec<Box<dyn AnyHashEq>>, &Context, PersonId);
//type MutationCallbackSetter = fn(&mut Context, Vec<TypeId>);
type Predicate = dyn Fn(PersonId, &Context) -> bool;

pub struct Query {
    properties: Vec<TypeId>,
    property_values: Vec<Box<dyn AnyHashEq>>,
    predicates: Vec<Box<Predicate>>,
}

impl Default for Query {
    fn default() -> Self {
        Self::new()
    }
}

impl Query {
    #[must_use]
    pub fn new() -> Query {
        Query {
            properties: Vec::new(),
            property_values: Vec::new(),
            predicates: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_person_property<T: PersonProperty + 'static>(
        mut self,
        property: T,
        value: T::Value,
    ) -> Query
    where
        T::Value: Hash + Eq,
    {
        let type_id = TypeId::of::<T>();
        match self.properties.binary_search(&type_id) {
            Ok(index) => self.property_values[index] = Box::new(value),
            Err(index) => {
                self.properties.insert(index, type_id);
                self.property_values.insert(index, Box::new(value));
                self.predicates.insert(
                    index,
                    Box::new(move |person_id, context| {
                        context.get_person_property(person_id, property) == value
                    }),
                );
            }
        }
        self
    }
}

struct IndexData {
    property_setters: Vec<Box<PropertySetter>>,
    index_cells: HashMap<Vec<Box<dyn AnyHashEq>>, IndexSetPersonContainer>,
}

impl IndexData {
    fn get_person_cell(&self, context: &Context, person_id: PersonId) -> Vec<Box<dyn AnyHashEq>> {
        let mut index_cell = Vec::new();
        for property_setter in &self.property_setters {
            property_setter(&mut index_cell, context, person_id);
        }
        index_cell
    }
}
pub struct Index {
    properties: Vec<TypeId>,
    property_setters: Vec<Box<PropertySetter>>,
}

impl Default for Index {
    fn default() -> Self {
        Self::new()
    }
}

impl Index {
    #[must_use]
    pub fn new() -> Index {
        Index {
            properties: Vec::new(),
            property_setters: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_person_property<T: PersonProperty + 'static>(mut self, property: T) -> Index
    where
        T::Value: Hash + Eq,
    {
        let type_id = TypeId::of::<T>();
        match self.properties.binary_search(&type_id) {
            Ok(_) => {}
            Err(index) => {
                self.properties.insert(index, type_id);

                // Add setter for person property
                self.property_setters.insert(
                    index,
                    Box::new(move |index_cell, context, person_id| {
                        let value = context.get_person_property::<T>(person_id, property);
                        index_cell.push(Box::new(value));
                    }),
                );
            }
        }
        self
    }
}

trait ContextPeopleInternalExt {
    fn add_person_to_indexes(&mut self, person_id: PersonId);

    fn update_indexes<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        _property: T,
        current_value: T::Value,
    ) where
        T::Value: AnyHashEq;
}

impl ContextPeopleInternalExt for Context {
    fn add_person_to_indexes(&mut self, person_id: PersonId) {
        let data_container = self.get_data_container(PeoplePlugin).unwrap();
        let index_keys: Vec<Vec<TypeId>> = data_container
            .index_data_map
            .keys()
            .map(Clone::clone)
            .collect();
        for key in index_keys {
            let data_container = self.get_data_container(PeoplePlugin).unwrap();
            let cell = data_container
                .index_data_map
                .get(&key)
                .unwrap()
                .get_person_cell(self, person_id);

            let data_container = self.get_data_container_mut(PeoplePlugin);
            data_container
                .index_data_map
                .get_mut(&key)
                .unwrap()
                .index_cells
                .entry(cell)
                .or_default()
                .insert(person_id);
        }
    }

    fn update_indexes<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        _property: T,
        old_value: T::Value,
    ) where
        T::Value: AnyHashEq,
    {
        let data_container = self.get_data_container(PeoplePlugin).unwrap();
        if let Some(index_keys) = data_container
            .index_sensitivities
            .get(&TypeId::of::<T>())
            .map(Clone::clone)
        {
            for index_key in index_keys {
                let data_container = self.get_data_container(PeoplePlugin).unwrap();
                // First find the person's new cell
                let mut index_cell = data_container
                    .index_data_map
                    .get(&index_key)
                    .unwrap()
                    .get_person_cell(self, person_id);
                let property_index = index_key.binary_search(&TypeId::of::<T>()).unwrap();

                // Move from old to new cell
                let data_map = &mut self.get_data_container_mut(PeoplePlugin).index_data_map;
                let index_data = data_map.get_mut(&index_key).unwrap();
                let index_cells = &mut index_data.index_cells;
                let new_value =
                    std::mem::replace(&mut index_cell[property_index], Box::new(old_value));
                index_cells.get_mut(&index_cell).unwrap().remove(&person_id);
                index_cell[property_index] = new_value;
                index_cells.entry(index_cell).or_default().insert(person_id);
            }
        }
    }
}

pub trait ContextPeopleExt {
    /// Returns the current population size
    fn get_current_population(&self) -> usize;

    /// Creates a new person with no assigned person properties
    fn add_person(&mut self) -> PersonId;

    fn add_person_with_overrides(
        &mut self,
        overrides: impl FnOnce(&mut Context, PersonId),
    ) -> PersonId;

    /// Given a `PersonId` returns the value of a defined person property
    /// Panics if it's not set
    fn get_person_property<T: PersonProperty + 'static>(
        &self,
        person_id: PersonId,
        _property: T,
    ) -> T::Value;

    /// Given a `PersonId`, sets the value of a defined person property
    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        _property: T,
        value: T::Value,
    );

    // Get the current number of people in the simulation
    fn get_population(&self) -> usize;

    /// Find people who match a given query of property values
    fn query_people(&self, query: Query) -> IndexSetPersonContainer;

    /// Add an index to make future queries more efficient
    fn add_index(&mut self, index: Index);
}

impl ContextPeopleExt for Context {
    fn get_current_population(&self) -> usize {
        self.get_data_container(PeoplePlugin)
            .map_or(0, |data_container| data_container.current_population)
    }

    fn add_person(&mut self) -> PersonId {
        self.add_person_with_overrides(|_context, _person_id| {})
    }

    fn add_person_with_overrides(
        &mut self,
        overrides: impl FnOnce(&mut Context, PersonId),
    ) -> PersonId {
        let data_container = self.get_data_container_mut(PeoplePlugin);
        let person_id = data_container.add_person();
        // Execute property overrides
        overrides(self, person_id);
        // Add person to indexes
        self.add_person_to_indexes(person_id);
        self.emit_event(PersonCreatedEvent { person_id });
        person_id
    }

    fn get_person_property<T: PersonProperty + 'static>(
        &self,
        person_id: PersonId,
        property: T,
    ) -> T::Value {
        let data_container = self.get_data_container(PeoplePlugin)
            .expect("PeoplePlugin is not initialized; make sure you add a person before accessing properties");

        // Attempt to retrieve the existing value
        if let Some(value) = *data_container.get_person_property_ref(person_id, property) {
            return value;
        }

        // Initialize the property. This does not fire a change event
        let initialized_value = T::initialize(self, person_id);
        data_container.set_person_property(person_id, property, initialized_value);

        initialized_value
    }

    fn set_person_property<T: PersonProperty + 'static>(
        &mut self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        let data_container = self.get_data_container_mut(PeoplePlugin);
        let current_value = *data_container.get_person_property_ref(person_id, property);
        match current_value {
            // The person property is already set, so we emit a change event
            Some(current_value) => {
                let change_event: PersonPropertyChangeEvent<T> = PersonPropertyChangeEvent {
                    person_id,
                    current: value,
                    previous: current_value,
                };
                data_container.set_person_property(person_id, property, value);
                // Update indexes
                // TODO: This doesn't work as T::Value is not necessarily Hash + Eq
                // self.update_indexes(person_id, property, current_value);
                self.emit_event(change_event);
            }
            // The person property is not yet initialized, so we don't emit any events.
            None => {
                data_container.set_person_property(person_id, property, value);
                // TODO: Note, for indexes this branch should not happen or
                //   else how could this property have been indexed?
            }
        }
    }

    fn get_population(&self) -> usize {
        if let Some(data_container) = self.get_data_container(PeoplePlugin) {
            data_container.current_population
        } else {
            0
        }
    }

    fn query_people(&self, query: Query) -> IndexSetPersonContainer {
        // If index exists, use that to service the query
        if let Some(index_data) = self
            .get_data_container(PeoplePlugin)
            .and_then(|data_container| data_container.index_data_map.get(&query.properties))
        {
            match index_data.index_cells.get(&query.property_values) {
                // Cell matching the query exists
                Some(cell) => cell.clone(),
                // Nobody matches the query
                None => IndexSetPersonContainer::new(),
            }
        } else {
            // No index exists, so just iterate over the population
            let mut container = IndexSetPersonContainer::new();
            let population = self.get_population();
            for id in 0..population {
                let person_id = PersonId { id };
                if query
                    .predicates
                    .iter()
                    .all(|predicate| predicate(person_id, self))
                {
                    container.insert(person_id);
                }
            }
            container
        }
    }

    fn add_index(&mut self, index: Index) {
        // TODO: check index does not already exist

        // First build the index data
        let mut index_data = IndexData {
            index_cells: HashMap::new(),
            property_setters: index.property_setters,
        };

        // Iterate over population and add them to their appropriate cell
        let population = self.get_population();
        for i in 0..population {
            let person_id = PersonId { id: i };
            let index_cell = index_data.get_person_cell(self, person_id);
            index_data
                .index_cells
                .entry(index_cell)
                .or_default()
                .insert(person_id);
        }

        // Store the index data
        let data_container = self.get_data_container_mut(PeoplePlugin);
        data_container
            .index_data_map
            .insert(index.properties.clone(), index_data);

        // Store the properties that this index is sensitive to
        index.properties.clone().iter().for_each(|type_id| {
            data_container
                .index_sensitivities
                .entry(*type_id)
                .or_default()
                .push(index.properties.clone());
        });
    }
}

#[cfg(test)]
mod test {
    use rand::{rngs::StdRng, seq::index::sample, Rng, SeedableRng};

    use super::{
        ContextPeopleExt, PersonCreatedEvent, PersonProperty, PersonPropertyChangeEvent, Query,
    };
    use crate::{
        context::Context,
        people::{Index, PersonId},
    };
    use std::{cell::RefCell, rc::Rc};

    define_person_property!(Age, u8);
    #[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
    pub enum RiskCategory {
        High,
        Low,
    }
    define_person_property!(RiskCategoryType, RiskCategory);
    define_person_property!(IsRunner, bool, false);

    #[derive(Copy, Clone)]
    struct RunningShoes;
    impl PersonProperty for RunningShoes {
        type Value = u8;
        fn initialize(context: &Context, person_id: super::PersonId) -> Self::Value {
            let is_runner = context.get_person_property(person_id, IsRunner);
            if is_runner {
                4
            } else {
                0
            }
        }
    }

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
        context.set_person_property(person, Age, 42);
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

    #[test]
    fn set_property_resize() {
        let mut context = Context::new();

        // Create a bunch of people and don't initialize Age
        let first_person = context.add_person();
        for _ in 1..9 {
            let _person = context.add_person();
        }
        let tenth_person = context.add_person();

        // Get a person property for a person > index 0
        context.set_person_property(tenth_person, Age, 42);

        // Now we set up a listener for change events
        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(move |_context, _event: PersonPropertyChangeEvent<Age>| {
            *flag_clone.borrow_mut() = true;
        });

        // This is the first time we're setting the Age property for the first person,
        // so it shouldn't emit a change event.
        context.set_person_property(first_person, Age, 42);
        context.execute();
        assert!(!*flag.borrow());

        // Now the change event should fire
        context.set_person_property(first_person, Age, 43);
        context.execute();
        assert!(*flag.borrow());
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
        context.set_person_property(person_id, Age, 42);
        context.set_person_property(person_id, RiskCategoryType, RiskCategory::Low);
        assert_eq!(context.get_person_property(person_id, Age), 42);
        assert_eq!(
            context.get_person_property(person_id, RiskCategoryType),
            RiskCategory::Low
        );
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
        context.set_person_property(person, Age, 42);
        let _is_runner = context.get_person_property(person, IsRunner);
        let _is_runner = context.get_person_property(person, RunningShoes);
        context.execute();
        assert!(!*flag.borrow());
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
        context.set_person_property(person_id, RiskCategoryType, RiskCategory::Low);
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
        // Innitializer wasn't called, so don't fire an event
        context.set_person_property(person_id, RunningShoes, 42);
        context.execute();
        assert!(!*flag.borrow());
    }

    #[test]
    fn query_without_index() {
        let mut context = Context::new();
        let person_zero = context.add_person();
        context.set_person_property(person_zero, Age, 10);
        context.set_person_property(person_zero, RiskCategoryType, RiskCategory::Low);

        let person_one = context.add_person();
        context.set_person_property(person_one, Age, 10);
        context.set_person_property(person_one, RiskCategoryType, RiskCategory::High);

        let person_two = context.add_person();
        context.set_person_property(person_two, Age, 20);
        context.set_person_property(person_two, RiskCategoryType, RiskCategory::Low);

        let person_three = context.add_person();
        context.set_person_property(person_three, Age, 20);
        context.set_person_property(person_three, RiskCategoryType, RiskCategory::Low);

        let age_10 = context.query_people(Query::new().with_person_property(Age, 10));
        assert_eq!(age_10.len(), 2);
        assert!(age_10.contains(&person_zero));
        assert!(age_10.contains(&person_one));

        let low_risk_age_10 = context.query_people(
            Query::new()
                .with_person_property(Age, 10)
                .with_person_property(RiskCategoryType, RiskCategory::Low),
        );
        assert_eq!(low_risk_age_10.len(), 1);
        assert!(low_risk_age_10.contains(&person_zero));

        let low_risk_age_20 = context.query_people(
            Query::new()
                .with_person_property(Age, 20)
                .with_person_property(RiskCategoryType, RiskCategory::Low),
        );
        assert_eq!(low_risk_age_20.len(), 2);
        assert!(low_risk_age_20.contains(&person_two));
        assert!(low_risk_age_20.contains(&person_three));
    }

    #[test]
    fn query_with_index() {
        let mut context = Context::new();
        context.add_index(Index::new().with_person_property(Age));
        context.add_index(
            Index::new()
                .with_person_property(Age)
                .with_person_property(RiskCategoryType),
        );
        let person_zero = context.add_person_with_overrides(|context, person_id| {
            context.set_person_property(person_id, Age, 10);
            context.set_person_property(person_id, RiskCategoryType, RiskCategory::Low);
        });

        let person_one = context.add_person_with_overrides(|context, person_id| {
            context.set_person_property(person_id, Age, 10);
            context.set_person_property(person_id, RiskCategoryType, RiskCategory::High);
        });

        let person_two = context.add_person_with_overrides(|context, person_id| {
            context.set_person_property(person_id, Age, 20);
            context.set_person_property(person_id, RiskCategoryType, RiskCategory::Low);
        });

        let person_three = context.add_person_with_overrides(|context, person_id| {
            context.set_person_property(person_id, Age, 20);
            context.set_person_property(person_id, RiskCategoryType, RiskCategory::Low);
        });

        let age_10 = context.query_people(Query::new().with_person_property(Age, 10));
        assert_eq!(age_10.len(), 2);
        assert!(age_10.contains(&person_zero));
        assert!(age_10.contains(&person_one));

        let low_risk_age_10 = context.query_people(
            Query::new()
                .with_person_property(Age, 10)
                .with_person_property(RiskCategoryType, RiskCategory::Low),
        );
        assert_eq!(low_risk_age_10.len(), 1);
        assert!(low_risk_age_10.contains(&person_zero));

        let low_risk_age_20 = context.query_people(
            Query::new()
                .with_person_property(Age, 20)
                .with_person_property(RiskCategoryType, RiskCategory::Low),
        );
        assert_eq!(low_risk_age_20.len(), 2);
        assert!(low_risk_age_20.contains(&person_two));
        assert!(low_risk_age_20.contains(&person_three));
    }

    #[test]
    fn index_change_properties() {
        let population = 1000;
        let n_to_change_properties = 100;

        let mut context = Context::new();

        context.add_index(
            Index::new()
                .with_person_property(Age)
                .with_person_property(RiskCategoryType),
        );

        for _ in 0..population {
            context.add_person_with_overrides(|context, person_id| {
                context.set_person_property(person_id, Age, 10);
                context.set_person_property(person_id, RiskCategoryType, RiskCategory::Low);
            });
        }

        let mut rng = StdRng::seed_from_u64(8_675_309);
        let people_to_change = sample(&mut rng, population, n_to_change_properties);

        let mut n_with_age_20 = 0;
        for person_id in people_to_change {
            if rng.gen_bool(0.5) {
                context.set_person_property(PersonId { id: person_id }, Age, 20);
                n_with_age_20 += 1;
            } else {
                context.set_person_property(
                    PersonId { id: person_id },
                    RiskCategoryType,
                    RiskCategory::High,
                );
            }
        }

        assert_eq!(
            context
                .query_people(
                    Query::new()
                        .with_person_property(Age, 20)
                        .with_person_property(RiskCategoryType, RiskCategory::Low)
                )
                .len(),
            n_with_age_20
        );

        assert_eq!(
            context
                .query_people(
                    Query::new()
                        .with_person_property(Age, 10)
                        .with_person_property(RiskCategoryType, RiskCategory::High)
                )
                .len(),
            n_to_change_properties - n_with_age_20
        );

        assert_eq!(
            context
                .query_people(
                    Query::new()
                        .with_person_property(Age, 20)
                        .with_person_property(RiskCategoryType, RiskCategory::High)
                )
                .len(),
            0
        );
    }
}
