use crate::people::context_extension::{ContextPeopleExt, ContextPeopleExtInternal};
use crate::people::index::{BxIndex, Index};
use crate::people::methods::Methods;
use crate::people::property::PropertyInitializationKind;
use crate::people::property_store::{BxPropertyStore, PropertyStore};
use crate::people::{HashValueType, InitializationList};
use crate::{Context, IxaError, PersonId, PersonProperty, PersonPropertyChangeEvent};
use crate::{HashMap, HashSet, HashSetExt};
use std::any::TypeId;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::hash_map::Entry;

type ContextCallback = dyn FnOnce(&mut Context);

pub(super) struct PeopleData {
    pub(super) is_initializing: bool,
    pub(super) current_population: usize,
    pub(super) methods: RefCell<HashMap<TypeId, Methods>>,
    pub(super) properties_map: RefCell<HashMap<TypeId, BxPropertyStore>>,
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
    pub(super) fn index_unindexed_people_for_type_id(&self, context: &Context, type_id: TypeId) {
        let mut indexes = self.property_indexes.borrow_mut();
        let Some(index) = indexes.get_mut(&type_id) else {
            return;
        };
        index.index_unindexed_people(context);
    }
}

// The purpose of this trait is to enable storing a Vec of different
// `PersonProperty` value types. While `PersonProperty`` is *not* object safe,
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
    /// Adds a person and returns a `PersonId` that can be used to reference them.
    /// This will increment the current population by 1.
    pub(super) fn add_person(&mut self) -> PersonId {
        let id = self.current_population;
        self.current_population += 1;
        PersonId(id)
    }

    pub(super) fn get_person_property<P: PersonProperty>(
        &self,
        context: &Context,
        person_id: PersonId,
        _property: P,
    ) -> P::Value {
        if P::is_derived() {
            return P::compute(context, person_id);
        }

        // The outer option says whether the index is in bounds; the inner option says whether the value has been set.
        let value: Option<Option<<P as PersonProperty>::Value>>;

        // The scope of the mutable borrow of `properties_map`. The problem is that
        // `PeopleData::get_person_property()` can call itself recursively indirectly
        // through the call to `P::compute()`. In fact, it is common for `P::compute()`
        // to call `PeopleData::get_person_property()`. Therefore, we need to make
        // sure we are not holding a reference to `self.properties_map` at the time
        // `P::compute()` is called in order to avoid a double borrow.
        {
            let mut properties_map = self.properties_map.borrow_mut();

            // Fetch existing or initialize a new property store
            let property_store: &mut PropertyStore<P> = match properties_map.entry(P::type_id()) {
                Entry::Occupied(entry) => {
                    let property_store = entry.into_mut();
                    <dyn std::any::Any>::downcast_mut::<PropertyStore<P>>(property_store.as_mut())
                        .expect("Type mismatch in properties_map")
                }

                Entry::Vacant(entry) => match P::property_initialization_kind() {
                    PropertyInitializationKind::Normal => {
                        panic!(
                            "Property {} accessed before it was initialized for Person ID {}",
                            P::name(),
                            person_id
                        );
                    }
                    PropertyInitializationKind::Constant => {
                        // Initializer is a constant, so this call is safe, and we can just return it.
                        return P::compute(context, person_id);
                    }
                    PropertyInitializationKind::Dynamic => {
                        let property_store = entry.insert(Box::new(PropertyStore::<P>::new()));
                        <dyn std::any::Any>::downcast_mut::<PropertyStore<P>>(
                            property_store.as_mut(),
                        )
                        .expect("Type mismatch in properties_map")
                    }

                    PropertyInitializationKind::Derived => {
                        // Handled at the top of this method.
                        unreachable!();
                    }
                },
            };

            value = property_store.values.get(person_id.0).copied();
        }

        // We either found the value, or we need to compute it. If the vector is long enough to
        // have a slot, or if `PropertyInitializationKind::Dynamic`, we store the computed value.
        match value {
            // In bounds and value set.
            Some(Some(value)) => value,

            // In bounds but value not set.
            Some(None) => {
                // We don't have to pay the cost of resizing the vector, but we
                // are paying the cost of computing the value, so we store it.
                let value = P::compute(context, person_id);
                let mut properties_map = self.properties_map.borrow_mut();

                // The following unwrap is safe, because we inserted the property store during value lookup above.
                let property_store = properties_map.get_mut(&P::type_id()).unwrap();
                let property_store =
                    <dyn std::any::Any>::downcast_mut::<PropertyStore<P>>(property_store.as_mut())
                        .expect("Type mismatch in properties_map");
                property_store.values[person_id.0] = Some(value);
                value
            }

            // Not in bounds (and thus the value is not set)
            None => match P::property_initialization_kind() {
                // Accessing a property that has no default initializer before it has been set is an error.
                PropertyInitializationKind::Normal => panic!(
                    "Property {} accessed before it was initialized for Person ID {}",
                    P::name(),
                    person_id
                ),

                // Initial value is a constant, so we can just return it without doing work.
                PropertyInitializationKind::Constant => P::compute(context, person_id),

                // Initial value is dynamic, so we need to compute it and store the result.
                PropertyInitializationKind::Dynamic => {
                    let value = P::compute(context, person_id);
                    let mut properties_map = self.properties_map.borrow_mut();

                    // The following unwrap is safe, because we inserted the property store during value lookup above.
                    let property_store = properties_map.get_mut(&P::type_id()).unwrap();
                    let property_store = <dyn std::any::Any>::downcast_mut::<PropertyStore<P>>(
                        property_store.as_mut(),
                    )
                    .expect("Type mismatch in properties_map");
                    property_store.values.resize(person_id.0 + 1, None);
                    property_store.values[person_id.0] = Some(value);
                    value
                }

                // This case is taken care of at the top of this method.
                PropertyInitializationKind::Derived => unreachable!(),
            },
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
        match properties_map.entry(T::type_id()) {
            Entry::Occupied(mut entry) => {
                let property_store = entry.get_mut();
                // Only a `PropertyStore<T>` can be stored for key `T::type_id()`.
                let property_store =
                    <dyn std::any::Any>::downcast_mut::<PropertyStore<T>>(property_store.as_mut())
                        .expect("Type mismatch in properties_map");
                if index >= property_store.values.len() {
                    property_store.values.resize(index + 1, None);
                }
                property_store.values[index] = Some(value);
            }
            Entry::Vacant(entry) => {
                let mut property_store = Box::new(PropertyStore::<T>::new());
                property_store.values.resize(index + 1, None);
                property_store.values[index] = Some(value);
                entry.insert(property_store);
            }
        }
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
            if property.is_required() && !initialization.has_property(*t) {
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
