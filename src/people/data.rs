use crate::people::context_extension::{ContextPeopleExt, ContextPeopleExtInternal};
use crate::people::index::IndexMap;
use crate::people::methods::Methods;
use crate::people::InitializationList;
use crate::{type_of, Context, IxaError, PersonId, PersonProperty, PersonPropertyChangeEvent};
use crate::{HashMap, HashSet};
use std::any::{Any, TypeId};
use std::cell::{RefCell, RefMut};

type ContextCallback = dyn FnOnce(&mut Context);

// PeopleData represents each unique person in the simulation with an id ranging
// from 0 to population - 1. Person properties are associated with a person
// via their id.
pub(super) struct StoredPeopleProperties {
    is_required: bool,
    values: Box<dyn Any>,
}

impl StoredPeopleProperties {
    fn new<T: PersonProperty>() -> Self {
        StoredPeopleProperties {
            is_required: T::is_required(),
            values: Box::<Vec<Option<T::Value>>>::default(),
        }
    }
}

pub(crate) struct PeopleData {
    pub(super) is_initializing: bool,
    pub(super) current_population: usize,
    pub(super) methods: RefCell<HashMap<TypeId, Methods>>,
    pub(super) properties_map: RefCell<HashMap<TypeId, StoredPeopleProperties>>,
    pub(super) registered_derived_properties: RefCell<HashSet<TypeId>>,
    pub(super) dependency_map: RefCell<HashMap<TypeId, Vec<Box<dyn PersonPropertyHolder>>>>,
    pub(super) property_indexes: RefCell<IndexMap>,
    pub(super) people_types: RefCell<HashMap<&'static str, TypeId>>,
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

    fn property_type_id(&self) -> TypeId {
        type_of::<T>()
    }
}

impl PeopleData {
    /// Adds a person and returns a `PersonId` that can be used to reference them.
    /// This will increment the current population by 1.
    pub(super) fn add_person(&mut self) -> PersonId {
        let id = self.current_population;
        self.current_population += 1;
        PersonId(id)
    }

    /// Retrieves a specific property of a person by their `PersonId`. If the value has not yet been
    /// set, returns `None`.
    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn get_person_property<T: PersonProperty>(
        &self,
        person_id: PersonId,
        _property: T,
    ) -> Option<T::Value> {
        let properties_map = self.properties_map.borrow();
        let index = person_id.0;

        match properties_map.get(&type_of::<T>()) {
            Some(properties) => {
                let values: &Vec<Option<T::Value>> = properties
                    .values
                    .downcast_ref()
                    .expect("Type mismatch in properties_map");

                if index >= values.len() {
                    None
                } else {
                    values[index]
                }
            }

            None => None,
        }
    }

    /// Sets the value of a property for a person
    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn set_person_property<T: PersonProperty>(
        &self,
        person_id: PersonId,
        _property: T,
        value: T::Value,
    ) {
        let mut properties_map = self.properties_map.borrow_mut();
        let index = person_id.0;
        let properties = properties_map
            .entry(type_of::<T>())
            .or_insert_with(|| StoredPeopleProperties::new::<T>());

        let values: &mut Vec<Option<T::Value>> = properties
            .values
            .downcast_mut()
            .expect("Type mismatch in properties_map");
        if index >= values.len() {
            values.resize(index + 1, None);
        }

        values[index] = Some(value);
    }

    pub(super) fn register_nonderived_property<T: PersonProperty>(&self) {
        self.methods
            .borrow_mut()
            .insert(type_of::<T>(), Methods::new::<T>());

        self.people_types
            .borrow_mut()
            .insert(T::name(), type_of::<T>());

        self.registered_derived_properties
            .borrow_mut()
            .insert(type_of::<T>());
    }

    pub(super) fn add_to_index_maybe<T: PersonProperty>(
        &self,
        context: &Context,
        person_id: PersonId,
        _property: T,
    ) {
        let method_map = self.methods.borrow();
        let methods = method_map.get(&type_of::<T>()).unwrap();
        let mut indexes = self.property_indexes.borrow_mut();
        let index = indexes.get_container_mut::<T>();
        if index.lookup.is_some() {
            index.add_person(context, methods, person_id);
        }
    }

    pub(super) fn remove_from_index_maybe<T: PersonProperty>(
        &self,
        context: &Context,
        person_id: PersonId,
        _property: T,
    ) {
        let method_map = self.methods.borrow();
        let methods = method_map.get(&type_of::<T>()).unwrap();
        let mut indexes = self.property_indexes.borrow_mut();
        let index = indexes.get_container_mut::<T>();
        if index.lookup.is_some() {
            index.remove_person(context, methods, person_id);
        }
    }

    pub(super) fn index_property<T: PersonProperty>(&self) {
        let mut indexes = self.property_indexes.borrow_mut();
        let index = indexes.get_container_mut::<T>();
        index.lookup.get_or_insert_with(HashMap::default);
    }

    pub(super) fn index_property_by_id(&self, type_id: TypeId) -> Result<(), IxaError> {
        let mut indexes = self.property_indexes.borrow_mut();
        let type_name = self
            .lookup_type_name(type_id)
            .ok_or_else(|| IxaError::IxaError("Unknown type".to_string()))?;

        let index = indexes.get_container_by_id_mut(type_id, type_name);
        index.lookup.get_or_insert_with(HashMap::default);
        Ok(())
    }

    pub(super) fn index_unindexed_people<T: PersonProperty>(&self, context: &Context) {
        let mut index_map = self.property_indexes.borrow_mut();
        let methods_map = self.methods.borrow();
        // Only called from contexts in which `T` has been registered, thus methods exist.
        let methods = methods_map.get(&type_of::<T>()).unwrap();
        index_map
            .get_container_mut::<T>()
            .index_unindexed_people(context, methods);
    }

    pub(super) fn index_unindexed_people_by_id(
        &self,
        context: &Context,
        type_id: TypeId,
    ) -> Result<(), IxaError> {
        let mut index_map = self.property_indexes.borrow_mut();
        let methods_map = self.methods.borrow();
        let type_name = self
            .lookup_type_name(type_id)
            .ok_or_else(|| IxaError::IxaError("unknown type".to_string()))?;
        let methods = methods_map
            .get(&type_id)
            .ok_or_else(|| IxaError::IxaError("unregistered type".to_string()))?;

        index_map
            .get_container_by_id_mut(type_id, type_name)
            .index_unindexed_people(context, methods);
        Ok(())
    }

    pub(super) fn property_is_indexed<T: PersonProperty>(&self) -> bool {
        let indexes = self.property_indexes.borrow();
        if let Some(index) = indexes.get_container_ref::<T>() {
            index.lookup.is_some()
        } else {
            false
        }
    }

    fn lookup_type_name(&self, type_id: TypeId) -> Option<&'static str> {
        self.people_types
            .borrow()
            .iter()
            .find_map(|(&s, t)| if *t == type_id { Some(s) } else { None })
    }

    /// Convenience function to iterate over the current population.
    /// Note that this doesn't hold a reference to `PeopleData`, so if
    /// you change the population while using it, it won't notice.
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
