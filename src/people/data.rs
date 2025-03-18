use crate::people::context_extension::{ContextPeopleExt, ContextPeopleExtInternal};
use crate::people::index::Index;
use crate::people::methods::Methods;
use crate::people::InitializationList;
use crate::{Context, IxaError, PersonId, PersonProperty, PersonPropertyChangeEvent};
use crate::{HashMap, HashSet, HashSetExt};
use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::time::Duration;

type ContextCallback = dyn FnOnce(&mut Context);

// PeopleData represents each unique person in the simulation with an id ranging
// from 0 to population - 1. Person properties are associated with a person
// via their id.
pub(super) struct StoredPeopleProperties {
    is_required: bool,
    values: Box<dyn Any>,
}

impl StoredPeopleProperties {
    fn new<T: PersonProperty + 'static>() -> Self {
        StoredPeopleProperties {
            is_required: T::is_required(),
            values: Box::<Vec<Option<T::Value>>>::default(),
        }
    }
}

pub(super) struct PeopleData {
    pub(super) is_initializing: bool,
    pub(super) current_population: usize,
    pub(super) methods: RefCell<HashMap<TypeId, Methods>>,
    pub(super) properties_map: RefCell<HashMap<TypeId, StoredPeopleProperties>>,
    pub(super) registered_derived_properties: RefCell<HashSet<TypeId>>,
    pub(super) dependency_map: RefCell<HashMap<TypeId, Vec<Box<dyn PersonPropertyHolder>>>>,
    pub(super) property_indexes: RefCell<HashMap<TypeId, Index>>,
    pub(super) people_types: RefCell<HashMap<String, TypeId>>,
    pub(super) query_profile: RefCell<HashMap<u64, (usize, Duration)>>,
}

// The purpose of this trait is to enable storing a Vec of different
// `PersonProperty` types. While `PersonProperty`` is *not* object safe,
// primarily because it stores a different Value type for each kind of property,
// `PersonPropertyHolder` is, meaning we can treat different types of properties
// uniformly at runtime.
// Note: this has to be pub because `PersonProperty` (which is pub) implements
// a dependency method that returns `PersonPropertyHolder` instances.
#[doc(hidden)]
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

impl PeopleData {
    /// Prints the accumulated query statistics in a nicely formatted table.
    ///
    /// The table displays:
    /// - The query hash (in hexadecimal)
    /// - The number of times the query was run
    /// - The total time spent running the query (in seconds)
    /// - The average time per query (in seconds)
    pub fn print_query_profile(&self) {
        let query_profile = self.query_profile.borrow();

        // Column widths
        let col1 = 18; // Query Hash
        let col2 = 8; // Count
        let col3 = 12; // Total Time
        let col4 = 18; // Avg Time

        // Top border with rounded corners
        println!(
            "╭{0:─^1$}┬{0:─^2$}┬{0:─^3$}┬{0:─^4$}╮",
            "", col1, col2, col3, col4,
        );

        // Header row
        println!(
            "│{:>4$}│{:>5$}│{:>6$}│{:>7$}│",
            "Query Hash", "Count", "Total Time", "Avg Time", col1, col2, col3, col4,
        );

        // Header separator
        println!(
            "├{0:─^1$}┼{0:─^2$}┼{0:─^3$}┼{0:─^4$}┤",
            "", col1, col2, col3, col4,
        );

        // Data rows
        for (hash, (count, duration)) in query_profile.iter() {
            let total_secs = duration.as_secs_f64();
            let avg_secs = if *count > 0 {
                total_secs / (*count as f64)
            } else {
                0.0
            };

            println!(
                "│{:018x}│{:>4$}│{:>5$.3}│{:>6$.9}│",
                hash, count, total_secs, avg_secs, col2, col3, col4,
            );
        }

        // Bottom border with rounded corners
        println!(
            "╰{0:─^1$}┴{0:─^2$}┴{0:─^3$}┴{0:─^4$}╯",
            "", col1, col2, col3, col4,
        );
    }

    /// Prints a table of property indexes.
    ///
    /// The table displays:
    /// - The `TypeId` of each property (using `Debug` formatting)
    /// - The number of times that property was accessed (`access_count`)
    /// - The total time spent accessing the property (in seconds)
    pub fn print_property_indexes(&self) {
        let indexes = self.property_indexes.borrow();
        // Define column widths
        let col1 = 20; // Property name
        let col2 = 12; // Access Count
        let col3 = 19; // Indexing Time (s)
        let col4 = 19; // Lookup Time (s)
        let col5 = 10; // Hits
        let col6 = 10; // Misses

        // Top border with rounded corners
        println!(
            "╭{0:─^1$}┬{0:─^2$}┬{0:─^3$}┬{0:─^4$}┬{0:─^5$}┬{0:─^6$}╮",
            "", col1, col2, col3, col4, col5, col6,
        );

        // Header row
        println!(
            "│{:<6$}│{:>7$}│{:>8$}│{:>9$}│{:>10$}│{:>11$}│",
            "Property",
            "Access Count",
            "Indexing Time (s)",
            "Lookup Time (s)",
            "Hits",
            "Misses",
            col1,
            col2,
            col3,
            col4,
            col5,
            col6,
        );

        // Header separator
        println!(
            "├{0:─^1$}┼{0:─^2$}┼{0:─^3$}┼{0:─^4$}┼{0:─^5$}┼{0:─^6$}┤",
            "", col1, col2, col3, col4, col5, col6,
        );

        // Iterate over the entries in the HashMap and print each row
        for (type_id, index) in indexes.iter() {
            // Assume `get_property_name` returns a String from a TypeId
            let name = self.get_property_name(*type_id);
            let data = index.get_profile_data();
            println!(
                "│{:<6$}│{:>7$}│{:>8$.9}│{:>9$.9}│{:>10$}│{:>11$}│",
                name,
                data.access_count,
                data.indexing_time,
                data.lookup_time,
                data.hits,
                data.misses,
                col1,
                col2,
                col3,
                col4,
                col5,
                col6,
            );
        }

        // Bottom border with rounded corners
        println!(
            "╰{0:─^1$}┴{0:─^2$}┴{0:─^3$}┴{0:─^4$}┴{0:─^5$}┴{0:─^6$}╯",
            "", col1, col2, col3, col4, col5, col6,
        );
    }

    /// Adds a person and returns a `PersonId` that can be used to reference them.
    /// This will increment the current population by 1.
    pub(super) fn add_person(&mut self) -> PersonId {
        let id = self.current_population;
        self.current_population += 1;
        PersonId(id)
    }

    /// Retrieves a specific property of a person by their `PersonId`.
    ///
    /// Returns `RefMut<Option<T::Value>>`: `Some(value)` if the property exists for the given person,
    /// or `None` if it doesn't.
    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn get_person_property_ref<T: PersonProperty + 'static>(
        &self,
        person: PersonId,
        _property: T,
    ) -> RefMut<Option<T::Value>> {
        let properties_map = self.properties_map.borrow_mut();
        let index = person.0;
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
    pub(super) fn set_person_property<T: PersonProperty + 'static>(
        &self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        let mut property_ref = self.get_person_property_ref(person_id, property);
        *property_ref = Some(value);
    }

    pub(super) fn get_index_ref_mut(&self, t: TypeId) -> Option<RefMut<Index>> {
        let index_map = self.property_indexes.borrow_mut();
        if index_map.contains_key(&t) {
            Some(RefMut::map(index_map, |map| map.get_mut(&t).unwrap()))
        } else {
            None
        }
    }

    pub(super) fn get_index_ref(&self, t: TypeId) -> Option<Ref<Index>> {
        let index_map = self.property_indexes.borrow();
        if index_map.contains_key(&t) {
            Some(Ref::map(index_map, |map| map.get(&t).unwrap()))
        } else {
            None
        }
    }

    pub(super) fn get_index_ref_mut_by_prop<T: PersonProperty + 'static>(
        &self,
        _property: T,
    ) -> Option<RefMut<Index>> {
        let type_id = TypeId::of::<T>();
        self.get_index_ref_mut(type_id)
    }

    // Convenience function to iterate over the current population.
    // Note that this doesn't hold a reference to PeopleData, so if
    // you change the population while using it, it won't notice.
    pub(super) fn people_iterator(&self) -> PeopleIterator {
        PeopleIterator {
            population: self.current_population,
            person_id: 0,
        }
    }

    pub(super) fn check_initialization_list<T: InitializationList>(
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

    pub(super) fn get_methods(&self, t: TypeId) -> RefMut<'_, Methods> {
        let x = self.methods.borrow_mut();
        RefMut::map(x, |a| a.get_mut(&t).unwrap())
    }

    pub(crate) fn get_property_name(&self, type_id: TypeId) -> String {
        for (name, tid) in self.people_types.borrow().iter() {
            if *tid == type_id {
                return name.clone();
            }
        }
        "UNKNOWN".to_string()
    }
}

pub(super) struct PeopleIterator {
    population: usize,
    person_id: usize,
}

impl Iterator for PeopleIterator {
    type Item = PersonId;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = if self.person_id < self.population {
            Some(PersonId(self.person_id))
        } else {
            None
        };
        self.person_id += 1;

        ret
    }
}
