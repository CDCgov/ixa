/*!

The `EntityStore` maintains one entity-erased [`PropertyStore`] for each registered [`Entity`].
Each concrete `PropertyStoreCore<E>` owns the count of valid [`EntityId<E>`] values together with
the entity instance and all property storage for `E`.

Although each Entity type may own its own data, client code cannot create or destructure
`EntityId<Entity>` values directly. Instead, `EntityStore` centrally manages entity counts
for all registered types so that only valid (existing) `EntityId<E>` values are ever created.


*/

use std::any::Any;

use crate::entity::property_store::PropertyStore;
use crate::entity::property_store_core::PropertyStoreCore;
use crate::entity::schema_registry::SCHEMA_REGISTRY;
use crate::entity::{Entity, EntityId, PopulationIterator};

/// Returns the number of registered entity types.
pub(crate) fn get_registered_entity_count() -> usize {
    SCHEMA_REGISTRY.entity_registrations().len()
}

/// A wrapper around a vector of entities.
pub struct EntityStore {
    items: Vec<Box<dyn PropertyStore>>,
}

impl Default for EntityStore {
    fn default() -> Self {
        EntityStore::new()
    }
}

impl EntityStore {
    /// Creates a new [`EntityStore`], allocating the exact number of slots as there are
    /// registered [`Entity`]s.
    ///
    /// This method assumes all types implementing `Entity` have been implemented _correctly_.
    /// This is one of the pitfalls of this pattern: there is no guarantee that types
    /// implementing `Entity` followed the rules. We can have at least some confidence,
    /// though, in their correctness by supplying a correct implementation via a macro.
    #[must_use]
    pub fn new() -> Self {
        let registrations = SCHEMA_REGISTRY.entity_registrations();
        // `entity_registrations()` returns only after freeze has completed, so no builder lock or
        // OnceLock initialization closure is active while constructors execute.
        Self {
            items: registrations
                .iter()
                .map(|registration| (registration.property_store_constructor)())
                .collect(),
        }
    }

    /// Fetches an immutable reference to the entity `E` from the registry. This
    /// implementation lazily instantiates the item if it has not yet been instantiated.
    #[must_use]
    pub fn get<E: Entity>(&self) -> &E {
        self.get_property_store::<E>()
            .entity
            .get_or_init(E::new_boxed)
            .as_ref()
    }

    /// Fetches a mutable reference to the item `E` from the registry. This
    /// implementation lazily instantiates the item if it has not yet been instantiated.
    #[must_use]
    pub fn get_mut<E: Entity>(&mut self) -> &mut E {
        let property_store = self.get_property_store_mut::<E>();

        // Initialize if needed
        if property_store.entity.get().is_none() {
            assert!(
                property_store.entity.set(E::new_boxed()).is_ok(),
                "Ixa internal error: entity instance was initialized concurrently"
            );
        }

        // The cell was already initialized or was initialized immediately above.
        property_store.entity.get_mut().unwrap().as_mut()
    }

    /// Creates a new `EntityId` for the given `Entity` type `E`.
    /// Increments the entity counter and returns the next valid ID together with whether
    /// `EntityCreatedEvent<E>` has subscribers.
    pub(crate) fn new_entity_id<E: Entity>(&mut self) -> (EntityId<E>, bool) {
        let (id, entity_created_event_subscribed) = self.items[E::id()].allocate_entity_id();
        (EntityId::new(id), entity_created_event_subscribed)
    }

    /// Returns a total count of all created entities of type `E`.
    #[must_use]
    pub fn get_entity_count<E: Entity>(&self) -> usize {
        self.items[E::id()].entity_count()
    }

    /// Returns a total count of all created entities of type `E`.
    #[must_use]
    pub fn get_entity_count_by_id(&self, id: usize) -> usize {
        self.items[id].entity_count()
    }

    /// Returns an iterator over all valid `EntityId<E>`s
    #[must_use]
    pub fn get_entity_iterator<E: Entity>(&self) -> PopulationIterator<E> {
        let count = self.get_entity_count::<E>();
        PopulationIterator::new(count)
    }

    #[must_use]
    #[inline]
    pub(crate) fn get_property_store<E: Entity>(&self) -> &PropertyStoreCore<E> {
        let index = E::id();
        let property_store = self
            .items
            .get(index)
            .unwrap_or_else(|| panic!("No registered entity found with index = {index:?}. You must use the `define_entity!` macro to create an entity."));
        let property_store: &dyn Any = property_store.as_ref();
        property_store
            .downcast_ref::<PropertyStoreCore<E>>()
            .expect("Entity type does not match the property store registered at its index. You must use the `define_entity!` or `impl_entity!` macro to create an entity.")
    }

    pub(crate) fn get_property_store_mut<E: Entity>(&mut self) -> &mut PropertyStoreCore<E> {
        let index = E::id();
        let property_store = self
            .items
            .get_mut(index)
            .unwrap_or_else(|| panic!("No registered entity found with index = {index:?}. You must use the `define_entity!` macro to create an entity."));
        let property_store: &mut dyn Any = property_store.as_mut();
        property_store
            .downcast_mut::<PropertyStoreCore<E>>()
            .expect("Entity type does not match the property store registered at its index. You must use the `define_entity!` or `impl_entity!` macro to create an entity.")
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;

    use crate::entity::entity_store::EntityStore;
    use crate::entity::schema_registry::add_to_entity_registry;
    use crate::entity::Entity;
    use crate::{impl_entity, with, Context, ContextEntitiesExt};
    // Test item types
    #[derive(Debug, Clone, PartialEq)]
    pub struct TestItem1 {
        value: usize,
    }
    impl Default for TestItem1 {
        fn default() -> Self {
            Self { value: 42 }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct TestItem2 {
        name: String,
    }
    impl Default for TestItem2 {
        fn default() -> Self {
            TestItem2 {
                name: "test".to_string(),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct TestItem3 {
        data: Vec<u8>,
    }
    impl Default for TestItem3 {
        fn default() -> Self {
            TestItem3 {
                data: vec![1, 2, 3],
            }
        }
    }

    // Implement RegisteredItem manually for testing without macro
    impl_entity!(TestItem1);
    impl_entity!(TestItem2);
    impl_entity!(TestItem3);

    // Registering items is idempotent
    #[test]
    fn test_add_to_registry_idempotent() {
        let index1 = TestItem1::id();
        let index2 = TestItem2::id();
        let index3 = TestItem3::id();

        // All should be initialized (uninitialized indices are `usize::MAX`)
        assert_ne!(index1, usize::MAX);
        assert_ne!(index2, usize::MAX);
        assert_ne!(index3, usize::MAX);

        // Each should have a unique index
        assert_ne!(index1, index2);
        assert_ne!(index2, index3);
        assert_ne!(index1, index3);

        // Adding the same type multiple times should return the same index.
        add_to_entity_registry::<TestItem1>();
        add_to_entity_registry::<TestItem1>();
        add_to_entity_registry::<TestItem1>();

        let index_from_registry_1 = TestItem1::id();
        let index_from_registry_2 = TestItem2::id();
        let index_from_registry_3 = TestItem3::id();

        assert_eq!(index1, index_from_registry_1);
        assert_eq!(index2, index_from_registry_2);
        assert_eq!(index3, index_from_registry_3);
    }

    // Getting items lazily initializes `Entity` instances
    #[test]
    fn test_registered_items_get() {
        // Test mutable `EntityStore::get_mut`
        {
            let mut items = EntityStore::new();

            let item1 = items.get_mut::<TestItem1>();
            assert_eq!(item1.value, 42);
            assert_eq!(TestItem1::name(), "TestItem1");

            let item2 = items.get_mut::<TestItem2>();
            assert_eq!(item2.name, "test");

            let item3 = items.get_mut::<TestItem3>();
            assert_eq!(item3.data, vec![1, 2, 3]);
        }

        // Test immutable `EntityStore::get`
        {
            let items = EntityStore::new();

            let item1 = items.get::<TestItem1>();
            assert_eq!(item1.value, 42);
            assert_eq!(TestItem1::name(), "TestItem1");

            let item2 = items.get::<TestItem2>();
            assert_eq!(item2.name, "test");

            let item3 = items.get::<TestItem3>();
            assert_eq!(item3.data, vec![1, 2, 3]);
        }
    }

    // Initialization happens once
    #[test]
    fn test_registered_items_get_cached() {
        // Test immutable `EntityStore::get`
        {
            let items = EntityStore::new();

            // Get the item twice
            let item1_ref1 = items.get::<TestItem1>();
            let item1_ref2 = items.get::<TestItem1>();

            // Both should point to the same instance
            assert!(std::ptr::eq(item1_ref1, item1_ref2));
        }

        // Test mutable `EntityStore::get_mut`
        {
            let mut items = EntityStore::new();

            // Get the item twice. We can safely get multiple mutable pointers so long as we don't dereference them.
            let item1_ptr1: *mut TestItem1 = items.get_mut::<TestItem1>();
            let item1_ptr2: *mut TestItem1 = items.get_mut::<TestItem1>();

            // Both should point to the same instance
            assert!(std::ptr::eq(item1_ptr1, item1_ptr2));
        }
    }

    #[test]
    fn test_registered_items_get_mut() {
        let mut items = EntityStore::new();

        // Get mutable reference and modify
        let item = items.get_mut::<TestItem1>();
        assert_eq!(item.value, 42);
        item.value = 100;

        // Verify the change persisted
        let item = items.get::<TestItem1>();
        assert_eq!(item.value, 100);
    }

    #[test]
    #[should_panic(
        expected = "Entity type does not match the property store registered at its index"
    )]
    fn mismatched_entity_store_slot_panics() {
        let mut items = EntityStore::new();
        items.items.swap(TestItem1::id(), TestItem2::id());
        let _ = items.get_property_store::<TestItem1>();
    }

    #[test]
    fn test_registered_items_multiple_items_mutated() {
        let mut items = EntityStore::new();

        // Read and mutate multiple items
        let item1 = items.get_mut::<TestItem1>();
        assert_eq!(item1.value, 42);
        item1.value = 10;

        let item2 = items.get_mut::<TestItem2>();
        assert_eq!(item2.name, "test");
        item2.name = "modified".to_string();

        let item3 = items.get_mut::<TestItem3>();
        assert_eq!(item3.data, vec![1, 2, 3]);
        item3.data = vec![9, 8, 7];

        // Verify all changes
        assert_eq!(items.get::<TestItem1>().value, 10);
        assert_eq!(items.get::<TestItem2>().name, "modified");
        assert_eq!(items.get::<TestItem3>().data, vec![9, 8, 7]);
    }

    #[test]
    #[should_panic(expected = "No registered entity found with index")]
    fn test_registered_items_invalid_index() {
        #[derive(Debug, Default)]
        struct UnregisteredEntity;

        // Intentionally implement `RegisteredItem` incorrectly.
        impl Entity for UnregisteredEntity {
            fn name() -> &'static str
            where
                Self: Sized,
            {
                "UnregisteredItem"
            }

            fn id() -> usize
            where
                Self: Sized,
            {
                87000 // An invalid index
            }

            fn as_any(&self) -> &dyn Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn Any {
                self
            }
        }

        // Create items container with insufficient capacity
        let items = EntityStore::new();

        // This should panic because TestItem1's index doesn't exist
        let _ = items.get::<UnregisteredEntity>();
    }

    #[test]
    fn test_registered_item_trait_name() {
        assert_eq!(TestItem1::name(), "TestItem1");
        assert_eq!(TestItem2::name(), "TestItem2");
        assert_eq!(TestItem3::name(), "TestItem3");
    }

    #[test]
    fn test_registered_item_new_boxed() {
        let boxed1 = TestItem1::new_boxed();
        assert_eq!(boxed1.value, 42);

        let boxed2 = TestItem2::new_boxed();
        assert_eq!(boxed2.name, "test");

        let boxed3 = TestItem3::new_boxed();
        assert_eq!(boxed3.data, vec![1, 2, 3]);
    }

    #[test]
    fn test_box_dyn_registered_item_type_alias() {
        let item = TestItem1::new_boxed();
        assert_eq!(
            (item as Box<dyn Any>)
                .downcast_ref::<TestItem1>()
                .unwrap()
                .value,
            42
        );
    }

    #[test]
    fn test_entity_iterator() {
        let mut context = Context::new();

        // Add different numbers of entities for each type
        // Note: add_entity returns Result<EntityId<E>, ...>, we unwrap for the test.
        for _ in 0..5 {
            context
                .add_entity::<TestItem1, _>(with!(TestItem1))
                .unwrap();
        }
        for _ in 0..3 {
            context
                .add_entity::<TestItem2, _>(with!(TestItem2))
                .unwrap();
        }
        // TestItem3 remains at 0 for now

        // 1. Verify counts
        assert_eq!(context.get_entity_count::<TestItem1>(), 5);
        assert_eq!(context.get_entity_count::<TestItem2>(), 3);
        assert_eq!(context.get_entity_count::<TestItem3>(), 0);

        // 2. Verify iterators
        let iter1 = context.get_entity_iterator::<TestItem1>();
        let results1: Vec<_> = iter1.collect();
        assert_eq!(results1.len(), 5);
        // Verify ID sequence (starts at 0)
        for (i, id) in results1.into_iter().enumerate() {
            assert_eq!(id.0, i);
        }

        let iter2 = context.get_entity_iterator::<TestItem2>();
        assert_eq!(iter2.count(), 3);

        let mut iter3 = context.get_entity_iterator::<TestItem3>();
        assert!(iter3.next().is_none());

        // 3. Verify iterator snapshot behavior
        // Iterators created now should not see entities added later
        let snapshot_iter = context.get_entity_iterator::<TestItem1>();

        context
            .add_entity::<TestItem1, _>(with!(TestItem1))
            .unwrap();

        assert_eq!(context.get_entity_count::<TestItem1>(), 6);
        assert_eq!(snapshot_iter.count(), 5); // Still sees original population
        assert_eq!(context.get_entity_iterator::<TestItem1>().count(), 6); // New iterator sees 6
    }
}
