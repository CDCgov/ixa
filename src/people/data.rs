use crate::people::context_extension::{ContextPeopleExt, ContextPeopleExtInternal};
use crate::people::index::{BxIndex, Index};
use crate::people::methods::Methods;
use crate::people::{HashValueType, InitializationList};
use crate::{Context, IxaError, PersonId, PersonProperty, PersonPropertyChangeEvent};
use crate::{HashMap, HashSet, HashSetExt};
use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};

type ContextCallback = dyn FnOnce(&mut Context);

// PeopleData represents each unique person in the simulation with an id ranging
// from 0 to population - 1. Person properties are associated with a person
// via their id.
pub(super) struct StoredPeopleProperties {
    is_required: bool,
    pub(super) values: Box<dyn Any>,
}

impl StoredPeopleProperties {
    fn new<T: PersonProperty>() -> Self {
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
    pub(super) registered_properties: RefCell<HashSet<TypeId>>,
    pub(super) dependency_map: RefCell<HashMap<TypeId, Vec<Box<dyn PersonPropertyHolder>>>>,
    pub(super) property_indexes: RefCell<HashMap<TypeId, BxIndex>>,
    pub(super) people_types: RefCell<HashMap<String, TypeId>>,
}

impl PeopleData {
    /// Returns `(is_indexed, people_for_id_hash)`.
    pub(crate) fn get_people_for_id_hash(
        &self,
        type_id: TypeId,
        hash: HashValueType,
    ) -> (bool, Option<Ref<HashSet<PersonId>>>) {
        let mut is_indexed = false;
        let people_for_id_hash = Ref::filter_map(self.property_indexes.borrow(), |map| {
            map.get(&type_id).and_then(|index| {
                if index.is_indexed() {
                    is_indexed = true;
                    index.get_with_hash(hash)
                } else {
                    None
                }
            })
        })
        .ok();

        (is_indexed, people_for_id_hash)
    }

    /// Removes a person from the index if the property is being indexed.
    pub(super) fn remove_person_if_indexed<T: PersonProperty>(
        &self,
        value: T::CanonicalValue,
        person_id: PersonId,
    ) {
        let mut indexes = self.property_indexes.borrow_mut();
        indexes.entry(T::type_id()).and_modify(|index| {
            if index.is_indexed() {
                let hash = T::hash_property_value(&value);
                index.remove_person_with_hash(hash, person_id);
            }
        });
    }

    /// Adds a person to the index if the property is being indexed.
    pub(super) fn add_person_if_indexed<T: PersonProperty>(
        &self,
        value: T::CanonicalValue,
        person_id: PersonId,
    ) {
        let mut indexes = self.property_indexes.borrow_mut();
        indexes.entry(T::type_id()).and_modify(|index| {
            if index.is_indexed() {
                let index = index.as_any_mut().downcast_mut::<Index<T>>().unwrap();
                index.add_person(&value, person_id);
            }
        });
    }

    /// Create an index object if it doesn't exist.
    pub(super) fn register_index<T: PersonProperty>(&self) {
        let mut indexes = self.property_indexes.borrow_mut();
        indexes
            .entry(T::type_id())
            .or_insert_with(|| Index::<T>::new());
    }

    /// Refreshes the index for the property with the given type ID, returning `true` if the
    /// property is index and false otherwise.
    pub(super) fn index_unindexed_people_for_type_id(
        &self,
        context: &Context,
        type_id: TypeId,
    ) -> bool {
        let mut indexes = self.property_indexes.borrow_mut();
        let Some(index) = indexes.get_mut(&type_id) else {
            return false;
        };
        index.index_unindexed_people(context)
    }
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
    T: PersonProperty,
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
        T::type_id()
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
    #[allow(unused)]
    /// This method looks up the index for the given `type_id` and `value_hash` and, if one is found, passes
    /// an immutable reference to the index's `HashSet` to the given function. If either the index doesn't exist
    /// or
    pub(super) fn with_index(
        &self,
        type_id: TypeId,
        value_hash: HashValueType,
        function: &mut dyn FnMut(&HashSet<PersonId>),
    ) -> Result<(), ()> {
        if let Some(index) = self.property_indexes.borrow().get(&type_id) {
            if let Some(people_set) = index.get_with_hash(value_hash) {
                (function)(people_set);
            } else {
                // The `value_hash` is not found. We assume this occurs when no people have that value for the
                // property, so we pass an empty set to the function. (This is guaranteed not to allocate.)
                let empty = HashSet::default();
                (function)(&empty);
            }
            return Ok(());
        }
        Err(())
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
    pub(super) fn get_person_property_ref<T: PersonProperty>(
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
    pub(super) fn set_person_property<T: PersonProperty>(
        &self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        let mut property_ref = self.get_person_property_ref(person_id, property);
        *property_ref = Some(value);
    }

    /// Sets the `Index<T>::is_indexed` field for the index entry associated with `T`. Creates
    /// an `Index<T>` instance if one does not exist.
    pub(super) fn set_property_indexed<T: PersonProperty>(&self, is_indexed: bool, property: T) {
        self.property_indexes
            .borrow_mut()
            .entry(property.property_type_id())
            .or_insert_with(|| Index::<T>::new())
            .set_indexed(is_indexed);
    }

    /// Sets the `Index<T>::is_indexed` field for the index entry associated with `T`. If there
    /// is no `Index<T>` instance, this method panics.
    pub(super) fn set_property_indexed_by_type_id(&self, is_indexed: bool, type_id: TypeId) {
        self.property_indexes
            .borrow_mut()
            .get_mut(&type_id)
            .expect("Index instance not found for type")
            .set_indexed(is_indexed);
    }

    /// Returns an immutable reference to the set of `PersonId`s associated to the given type_id
    /// and using the value hash. The caller should ensure the property's index isn't stale.
    pub(super) fn get_index_set_for_hash_type_id(
        &self,
        type_id: TypeId,
        hash_value: HashValueType,
    ) -> Option<Ref<HashSet<PersonId>>> {
        let index_map_ref = self.property_indexes.borrow();

        Ref::filter_map(index_map_ref, |index_map| {
            index_map.get(&type_id).and_then(|index| {
                if index.is_indexed() {
                    index.get_with_hash(hash_value)
                } else {
                    None
                }
            })
        })
        .ok()
    }

    /// Same as above, but the type ID of the property is not given
    /// explicitly. The caller should ensure the property's index isn't stale.
    pub(super) fn get_index_set<P: PersonProperty>(
        &self,
        value: P::Value,
    ) -> Option<Ref<HashSet<PersonId>>> {
        let type_id = P::type_id();
        let index_ref_map = RefCell::borrow(&self.property_indexes);

        Ref::filter_map(index_ref_map, |indexes| {
            if let Some(index) = indexes.get(&type_id) {
                index.get_with_hash(P::hash_property_value(&P::make_canonical(value)))
            } else {
                None
            }
        })
        .ok()
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
}

pub(super) struct PeopleIterator {
    pub population: usize,
    person_id: usize,
}

impl Iterator for PeopleIterator {
    type Item = PersonId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.person_id < self.population {
            self.person_id += 1;
            Some(PersonId(self.person_id - 1))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let k = self.population - self.person_id;
        (k, Some(k))
    }
}
