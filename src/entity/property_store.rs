/*!

A `PropertyStore` implements the registry pattern for property value stores: A `PropertyStore`
wraps a vector of `PropertyValueStore`s, one for each concrete property type. The implementor
of `Property` is the value type. Sincere there's a 1-1 correspondence between property
types and their value stores, we implement the `index` method for each property type.

*/
use std::any::Any;
use std::cell::OnceCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use super::entity::Entity;
use super::entity_store::register_property_with_entity;
use super::property::Property;
use super::property_value_store::PropertyValueStore;

/// Global item index counter; keeps track of the index that will be assigned to the next entity that
/// requests an index. Equivalently, holds a *count* of the number of entities currently registered.
static NEXT_PROPERTY_INDEX: Mutex<usize> = Mutex::new(0);

/// Adds a new item to the registry. The job of this method is to create whatever
/// "singleton" data/metadata is associated with the [`Entity`] if it doesn't already
/// exist.
///
/// In our use case, this method is called in the `ctor` function of each `Entity`
/// type and ultimately exists only so that we know how many `OnceCell`s to
/// construct in the constructor of `EntityStore`, so that we never have to mutate
/// `EntityStore` itself when an `Entity` is accessed for the first time. (The
/// `OnceCell` itself handles the interior mutability required for initialization.)
pub fn add_to_property_registry<E: Entity, P: Property<E>>() {
    // Initializes the index for the property type.
    let _ = P::index();
    // Registers the property with the entity type.
    register_property_with_entity(
        <E as Entity>::type_id(),
        <P as Property<E>>::type_id(),
        P::is_required(),
    );
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
    items: Vec<OnceCell<Box<dyn Any>>>,
}

impl Default for PropertyStore {
    fn default() -> Self {
        PropertyStore::new()
    }
}

impl PropertyStore {
    /// Creates a new [`PropertyStore`], allocating the exact number of slots as there are registered
    /// [`Property`]s.
    ///
    /// This method assumes all types implementing [`Property`] have been implemented
    /// _correctly_. This is one of the pitfalls of this pattern: there is
    /// no guarantee that types implementing [`Property`] followed the rules. We can
    /// have at least some confidence, though, in their correctness by supplying a
    /// correct implementation via a macro.
    ///
    /// Observe that we create an empty `OnceCell` in each slot in this implementation, but
    /// we could just as easily eagerly initialize the "RegisteredItem" instances here
    /// instead (assuming we collected constructors somewhere).
    pub fn new() -> Self {
        let num_items = get_registered_property_count();
        Self {
            items: (0..num_items).map(|_| OnceCell::new()).collect(),
        }
    }

    /// Fetches an immutable reference to the `PropertyValueStore<P>`. This
    /// implementation lazily instantiates the item if it has not yet been instantiated.
    #[must_use]
    pub fn get<E: Entity, P: Property<E>>(&self) -> &PropertyValueStore<E, P> {
        let index = P::index();
        self.items
        .get(index)
        .unwrap_or_else(|| panic!("No registered property found with index = {index:?}. You must use the `define_property!` macro to create a registered property."))
        .get_or_init(|| Box::new(PropertyValueStore::<E, P>::new()))
        .downcast_ref::<PropertyValueStore::<E, P>>()
        .expect("TypeID does not match registered property type. You must use the `define_property!` macro to create a registered property.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityId;
    use crate::{define_entity, define_property, impl_property};

    define_entity!(Person);

    // The primary advantage of the `define_property!` macro is that you don't have to remember the list of traits you
    // need to put in the `derive` clause for a property.
    define_property!(struct Age(u8), Person);

    // The `define_property` macro also gives you a cute syntax for specifying the default value, although it's not
    // much harder to specify a default using other macros.
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
            let ages: &PropertyValueStore<_, Age> = property_store.get();
            ages.set(EntityId::<Person>::new(0), Age(12));
            ages.set(EntityId::<Person>::new(1), Age(33));
            ages.set(EntityId::<Person>::new(2), Age(44));

            let infection_statuses: &PropertyValueStore<_, InfectionStatus> = property_store.get();
            infection_statuses.set(EntityId::<Person>::new(0), InfectionStatus::Susceptible);
            infection_statuses.set(EntityId::<Person>::new(1), InfectionStatus::Susceptible);
            infection_statuses.set(EntityId::<Person>::new(2), InfectionStatus::Infected);

            let vaccine_status: &PropertyValueStore<_, Vaccinated> = property_store.get();
            vaccine_status.set(EntityId::<Person>::new(0), Vaccinated(true));
            vaccine_status.set(EntityId::<Person>::new(1), Vaccinated(false));
            vaccine_status.set(EntityId::<Person>::new(2), Vaccinated(true));
        }

        // Verify that `get` returns the expected values
        {
            let ages: &PropertyValueStore<_, Age> = property_store.get();
            assert_eq!(ages.get(EntityId::<Person>::new(0)), Some(Age(12)));
            assert_eq!(ages.get(EntityId::<Person>::new(1)), Some(Age(33)));
            assert_eq!(ages.get(EntityId::<Person>::new(2)), Some(Age(44)));

            let infection_statuses: &PropertyValueStore<_, InfectionStatus> = property_store.get();
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

            let vaccine_status: &PropertyValueStore<_, Vaccinated> = property_store.get();
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
