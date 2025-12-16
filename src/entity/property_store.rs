/*!

A [`PropertyStore`] implements the registry pattern for property value stores: A [`PropertyStore`]
wraps a vector of `PropertyValueStore`s, one for each concrete property type. The implementor
of [`crate::entity::property::Property`] is the value type. Since there's a 1-1 correspondence between property types
and their value stores, we implement the `index` method for each property type to make
property lookup fast. The [`PropertyStore`] stores a list of all properties in the form of
boxed `PropertyValueStore` instances, which provide a type-erased interface to the backing
storage (including index) of the property. Storage is only allocated as-needed, so the
instantiation of a `PropertyValueStore` for a property that is never used is negligible.
There's no need, then, for lazy initialization of the `PropertyValueStore`s themselves.

This module also implements the initialization of "static" data associated with a property,
that is, data that is the same across all [`crate::context::Context`] instances, which is computed before `main()`
using `ctor` magic. (Each property implements a ctor that calls [`add_to_property_registry()`].)
For simplicity, a property's ctor implementation, supplied by a macro, just calls
`add_to_property_registry<E: Entity, P: Property<E>>()`, which does all the work. The
`add_to_property_registry` function adds the following metadata to global metadata stores:

Metadata stored on `PROPERTY_METADATA`, which for each property stores:
- a list of dependent (derived) properties, and
- a constructor function to create a new `PropertyValueStore` instance for the property.

Metadata stored on `ENTITY_METADATA`, which for each entity stores:
- a list of properties associated with the entity, and
- a list of _required_ properties for the entity. These are properties for
  which values must be supplied to `add_entity` when creating a new entity.

*/

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

use crate::entity::entity::Entity;
use crate::entity::entity_store::register_property_with_entity;
use crate::entity::events::PartialPropertyChangeEvent;
use crate::entity::property::Property;
use crate::entity::property_value_store::PropertyValueStore;
use crate::entity::property_value_store_core::PropertyValueStoreCore;
use crate::Context;

/// Global item index counter; keeps track of the index that will be assigned to the next entity that
/// requests an index. Equivalently, holds a *count* of the number of entities currently registered.
static NEXT_PROPERTY_INDEX: Mutex<usize> = Mutex::new(0);

/// A container struct to hold the (global) metadata for a single property.
///
/// At program startup (before `main()`, using ctors) we compute metadata for all properties
/// that are linked into the binary, and this data remains unchanged for the life of the program.
#[derive(Default)]
struct PropertyMetadata {
    /// The (derived) properties that depend on this property, as represented by their
    /// `Property::index` value. This list is used to update the index (if applicable)
    /// and emit change events for these properties when this property changes.
    dependents: Vec<usize>,
    /// A function that constructs a new `PropertyValueStoreCore<E, P>` instance in a type-erased
    /// way, used in the constructor of `PropertyStore`. This is an `Option` because this
    /// function pointer is recorded possibly out-of-order from when the `PropertyMetadata`
    /// instance for this property needs to exist (when its dependents are recorded).
    value_store_constructor: Option<fn() -> Box<dyn PropertyValueStore>>,
}

/// This maps `property_type_index` to `(vec_of_transitive_dependents)`. This data is actually
/// written by the property `ctor`s with a call to [`crate::entity::entity_store::register_property_with_entity`()].
static PROPERTY_METADATA: LazyLock<Mutex<HashMap<usize, PropertyMetadata>>> =
    LazyLock::new(|| Mutex::new(HashMap::default()));

/// The public getter for the dependents of a property with index `property_index` (as stored in
/// `PROPERTY_METADATA`). The `Property<E: Entity>::dependents()` method defers to this.
///
/// This function should only be called once `main()` starts, that is, not in `ctors` constructors,
/// as it assumes `PROPERTY_METADATA` has been correctly initialized. Hence, the "static" suffix.
///
/// # Safety
/// This function assumes that `PROPERTY_METADATA` will never again be mutated after initial
/// construction. Mutating the `Vec`s after taking these references would cause undefined behavior.
pub(super) unsafe fn get_property_dependents_static(property_index: usize) -> &'static [usize] {
    let map = PROPERTY_METADATA.lock().unwrap();

    // This method should only be called after program start.
    let property_metadata = map.get(&property_index)
                               .unwrap_or_else(|| panic!("No registered property found with index = {property_index:?}. You must use the `define_property!` macro to create a registered property."));

    // ToDo(RobertJacobsonCDC): There are various ways to eliminate the following uses of `unsafe`, but this is by far
    //        the simplest way to implement this. Make a decision either way about whether additional complexity is
    //        worth eliminating this use of `unsafe`.
    // Transmute to `'static` slice. This assumes the `Vec` will never move or reallocate.
    let dependents_static: &'static [usize] = unsafe {
        std::mem::transmute::<&[usize], &'static [usize]>(property_metadata.dependents.as_slice())
    };

    dependents_static
}

/// Adds a new item to the registry. The job of this method is to create whatever "singleton"
/// data/metadata is associated with the [`crate::entity::property::Property`] if it doesn't already exist. In
/// our use case, this method is called in the `ctor` function of each `Property<E>` type.
pub fn add_to_property_registry<E: Entity, P: Property<E>>() {
    // Initializes the index for the property type.
    let property_index = P::index();

    // Registers the property with the entity type.
    register_property_with_entity(
        <E as Entity>::type_id(),
        <P as Property<E>>::type_id(),
        P::is_required(),
    );

    let mut property_metadata = PROPERTY_METADATA.lock().unwrap();

    // Register the `PropertyValueStoreCore<E, P>` constructor.
    {
        let metadata = property_metadata
            .entry(property_index)
            .or_insert_with(PropertyMetadata::default);
        metadata.value_store_constructor = Some(PropertyValueStoreCore::<E, P>::new_boxed)
    }

    // Construct the dependency graph
    for dependency in P::non_derived_dependencies() {
        // Add `property_index` as a dependent of the dependency
        let dependency_meta = property_metadata
            .entry(dependency)
            .or_insert_with(PropertyMetadata::default);
        dependency_meta.dependents.push(property_index);
    }
}

/// A convenience getter for `NEXT_ENTITY_INDEX`.
pub fn get_registered_property_count() -> usize {
    *NEXT_PROPERTY_INDEX.lock().unwrap()
}

/// Encapsulates the synchronization logic for initializing an item's index.
///
/// Acquires a global lock on the next available item index, but only increments
/// it if we successfully initialize the provided index. The `index` of a registered
/// item is assigned at runtime but only once per type. It's possible for a single
/// type to attempt to initialize its index multiple times from different threads,
/// which is why all this synchronization is required. However, the overhead
/// is negligible, as this initialization only happens once upon first access.
///
/// In fact, for our use case we know we are calling this function
/// once for each type in each `Property`'s `ctor` function, which
/// should be the only time this method is ever called for the type.
pub fn initialize_property_index(index: &AtomicUsize) -> usize {
    // Acquire a global lock.
    let mut guard = NEXT_PROPERTY_INDEX.lock().unwrap();
    let candidate = *guard;

    // Try to claim the candidate index. Here we guard against the potential race condition that
    // another instance of this plugin in another thread just initialized the index prior to us
    // obtaining the lock. If the index has been initialized beneath us, we do not update
    // NEXT_PROPERTY_INDEX, we just return the value `index` was initialized to.
    // For a justification of the data ordering, see:
    //     https://github.com/CDCgov/ixa/pull/477#discussion_r2244302872
    match index.compare_exchange(usize::MAX, candidate, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => {
            // We won the race — increment the global next plugin index and return the new index
            *guard += 1;
            candidate
        }
        Err(existing) => {
            // Another thread beat us — don’t increment the global next plugin index,
            // just return existing
            existing
        }
    }
}

/// A wrapper around a vector of property value stores.
pub struct PropertyStore {
    /// A vector of `Box<PropertyValueStoreCore<E, P>>`, type-erased to `Box<dyn Any>` and downcast-able to
    /// `Box<PropertyValueStore<E>>`.
    items: Vec<Box<dyn PropertyValueStore>>,
}

impl Default for PropertyStore {
    fn default() -> Self {
        PropertyStore::new()
    }
}

impl PropertyStore {
    /// Creates a new [`PropertyStore`].
    pub fn new() -> Self {
        let num_items = get_registered_property_count();
        // The constructors for each `PropertyValueStoreCore<E, P>` are stored in the `PROPERTY_METADATA` global.
        let property_metadata = PROPERTY_METADATA.lock().unwrap();

        // We construct the correct concrete `PropertyValueStoreCore<E, P>` value for each index (=`P::index()`).
        let items = (0..num_items)
            .map(|idx| {
                let metadata = property_metadata
                    .get(&idx)
                    .unwrap_or_else(|| panic!("No property metadata entry for index {idx}"));
                let constructor = metadata
                    .value_store_constructor
                    .unwrap_or_else(|| panic!("No PropertyValueStore constructor for index {idx}"));
                constructor()
            })
            .collect();

        Self { items }
    }

    /// Fetches an immutable reference to the `PropertyValueStoreCore<E, P>`.
    #[must_use]
    pub fn get<E: Entity, P: Property<E>>(&self) -> &PropertyValueStoreCore<E, P> {
        let index = P::index();
        let property_value_store =
            self.items
                .get(index)
                .unwrap_or_else(||
                    panic!(
                        "No registered property found with index = {:?} while trying to get property {}. You must use the `define_property!` macro to create a registered property.",
                        index,
                        P::name()
                    )
                );
        // .as_ref(); // The `as_ref` is important, as otherwise this is a trait upcast, not a dereference.
        let property_value_store: &PropertyValueStoreCore<E, P> = property_value_store
            .as_any()
            .downcast_ref::<PropertyValueStoreCore<E, P>>()
            .unwrap_or_else(||
                {
                    panic!(
                        "Property type at index {index:?} does not match registered property type. Found type_id {:?} while getting type_id {:?}. You must use the `define_property!` macro to create a registered property.",
                        property_value_store.type_id(),
                        TypeId::of::<PropertyValueStoreCore<E, P>>()
                    )
                }
            );
        property_value_store
    }

    /// Fetches a mutable reference to the `PropertyValueStoreCore<E, P>`.
    #[must_use]
    pub fn get_mut<E: Entity, P: Property<E>>(&mut self) -> &mut PropertyValueStoreCore<E, P> {
        let index = P::index();
        let property_value_store =
            self.items
                .get_mut(index)
                .unwrap_or_else(||
                    panic!(
                        "No registered property found with index = {:?} while trying to get property {}. You must use the `define_property!` macro to create a registered property.",
                        index,
                        P::name()
                    )
                );
        let type_id = (&*property_value_store).type_id(); // Only used for error message if error occurs.
        let property_value_store: &mut PropertyValueStoreCore<E, P> = property_value_store
            .as_any_mut()
            .downcast_mut::<PropertyValueStoreCore<E, P>>()
            .unwrap_or_else(||
                {
                    panic!(
                        "Property type at index {index:?} does not match registered property type. Found type_id {:?} while getting type_id {:?}. You must use the `define_property!` macro to create a registered property.",
                        type_id,
                        TypeId::of::<PropertyValueStoreCore<E, P>>()
                    )
                }
            );
        property_value_store
    }

    /// Creates a `PartialPropertyChangeEvent` instance for the `entity_id` and `property_index`. This method is only
    /// called for derived dependents of some property that has changed (one of `P`'s non-derived dependencies).
    pub(crate) fn create_partial_property_change(
        &self,
        property_index: usize,
        // This `entity_id` is guaranteed by the caller to be a valid `EntityId<E>` for the current entity `E`.
        entity_id: usize,
        context: &Context,
    ) -> Box<dyn PartialPropertyChangeEvent> {
        let property_value_store = self.items
                                       .get(property_index)
            .unwrap_or_else(|| panic!("No registered property found with index = {property_index:?}. You must use the `define_property!` macro to create a registered property."));

        property_value_store.create_partial_property_change(entity_id, context)
    }
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]
    use super::*;
    use crate::entity::EntityId;
    use crate::{define_entity, define_property, impl_property};

    define_entity!(Person);

    // The primary advantage of the `define_property!` macro is that you don't have to remember the list of traits you
    // need to put in the `derive` clause for a property.
    define_property!(struct Age(u8), Person);

    // The `define_property` macro also lets you specify the default value.
    define_property!(
        enum InfectionStatus {
            Susceptible,
            Infected,
            Recovered,
        },
        Person,
        default_const = InfectionStatus::Susceptible
    );

    // If the property type has, for example, a complicated `derive` clause or
    // other proc macro attribute magic, it might not be parsable by the simplistic
    // `define_property!` macro. In that case, you can use the `impl_property!` macro for
    // a type that has already been defined. The downside is that you have to manually
    // specify the traits that all properties need to implement in the `derive` clause.
    #[derive(Copy, Clone, Debug, PartialEq, crate::serde::Serialize)]
    struct Vaccinated(bool);
    impl_property!(Vaccinated, Person, default_const = Vaccinated(false));

    #[test]
    fn test_get_property_store() {
        let property_store = PropertyStore::new();

        {
            let ages: &PropertyValueStoreCore<_, Age> = property_store.get();
            ages.set(EntityId::<Person>::new(0), Age(12));
            ages.set(EntityId::<Person>::new(1), Age(33));
            ages.set(EntityId::<Person>::new(2), Age(44));

            let infection_statuses: &PropertyValueStoreCore<_, InfectionStatus> =
                property_store.get();
            infection_statuses.set(EntityId::<Person>::new(0), InfectionStatus::Susceptible);
            infection_statuses.set(EntityId::<Person>::new(1), InfectionStatus::Susceptible);
            infection_statuses.set(EntityId::<Person>::new(2), InfectionStatus::Infected);

            let vaccine_status: &PropertyValueStoreCore<_, Vaccinated> = property_store.get();
            vaccine_status.set(EntityId::<Person>::new(0), Vaccinated(true));
            vaccine_status.set(EntityId::<Person>::new(1), Vaccinated(false));
            vaccine_status.set(EntityId::<Person>::new(2), Vaccinated(true));
        }

        // Verify that `get` returns the expected values
        {
            let ages: &PropertyValueStoreCore<_, Age> = property_store.get();
            assert_eq!(ages.get(EntityId::<Person>::new(0)), Some(Age(12)));
            assert_eq!(ages.get(EntityId::<Person>::new(1)), Some(Age(33)));
            assert_eq!(ages.get(EntityId::<Person>::new(2)), Some(Age(44)));

            let infection_statuses: &PropertyValueStoreCore<_, InfectionStatus> =
                property_store.get();
            assert_eq!(
                infection_statuses.get(EntityId::<Person>::new(0)),
                Some(InfectionStatus::Susceptible)
            );
            assert_eq!(
                infection_statuses.get(EntityId::<Person>::new(1)),
                Some(InfectionStatus::Susceptible)
            );
            assert_eq!(
                infection_statuses.get(EntityId::<Person>::new(2)),
                Some(InfectionStatus::Infected)
            );

            let vaccine_status: &PropertyValueStoreCore<_, Vaccinated> = property_store.get();
            assert_eq!(
                vaccine_status.get(EntityId::<Person>::new(0)),
                Some(Vaccinated(true))
            );
            assert_eq!(
                vaccine_status.get(EntityId::<Person>::new(1)),
                Some(Vaccinated(false))
            );
            assert_eq!(
                vaccine_status.get(EntityId::<Person>::new(2)),
                Some(Vaccinated(true))
            );
        }
    }
}
