/*!

The `EntityStore` maintains all registered entities in the form of [`EntityRecord`]s,
`EntityRecord`s track the count of the instances of the [`Entity`] (valid [`EntityId<Entity>`]
values) and owns the [`PropertyStore<E>`], which manages the entity's properties.

Although each Entity type may own its own data, client code cannot create or destructure
`EntityId<Entity>` values directly. Instead, `EntityStore` centrally manages entity counts
for all registered types so that only valid (existing) `EntityId<E>` values are ever created.


*/

use std::any::{Any, TypeId};
use std::cell::OnceCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};

use crate::entity::property_store::PropertyStore;
use crate::entity::{Entity, EntityId, EntityIterator};
use crate::HashMap;

/// Global entity index counter; keeps track of the index that will be assigned to the next entity that
/// requests an index. Equivalently, holds a *count* of the number of entities currently registered.
static NEXT_ENTITY_INDEX: Mutex<usize> = Mutex::new(0);

/// For each entity we keep track of the properties associated with it. This maps
/// `entity_type_id` to `(vec_of_all_property_type_ids, vec_of_required_property_type_ids)`.
/// This data is actually written by the property ctors with a call to
/// [`register_property_with_entity()`].
#[allow(clippy::type_complexity)]
static ENTITY_METADATA_BUILDER: LazyLock<Mutex<HashMap<TypeId, (Vec<TypeId>, Vec<TypeId>)>>> =
    LazyLock::new(|| Mutex::new(HashMap::default()));

/// The frozen entity->property registry, created exactly once on first read.
///
/// This is derived from `ENTITY_METADATA_BUILDER` by moving the builder `HashMap` out and
/// converting the `Vec`s to boxed slices to prevent further mutation.
#[allow(clippy::type_complexity)]
static ENTITY_METADATA: OnceLock<HashMap<TypeId, (Box<[TypeId]>, Box<[TypeId]>)>> = OnceLock::new();

/// Private helper to fetch or initialize the frozen metadata.
fn entity_metadata() -> &'static HashMap<TypeId, (Box<[TypeId]>, Box<[TypeId]>)> {
    ENTITY_METADATA.get_or_init(|| {
        let mut builder = ENTITY_METADATA_BUILDER.lock().unwrap();
        let builder = std::mem::take(&mut *builder);
        builder
            .into_iter()
            .map(|(entity_type_id, (props, reqs))| {
                (
                    entity_type_id,
                    (props.into_boxed_slice(), reqs.into_boxed_slice()),
                )
            })
            .collect()
    })
}

/// The public setter interface to `ENTITY_METADATA`.
pub fn register_property_with_entity(
    entity_type_id: TypeId,
    property_type_id: TypeId,
    required: bool,
) {
    let mut builder = ENTITY_METADATA_BUILDER.lock().unwrap();
    if ENTITY_METADATA.get().is_some() {
        panic!(
            "`register_property_with_entity()` called after entity metadata was frozen; registration must occur during startup/ctors."
        );
    }

    let (property_type_ids, required_property_type_ids) = builder
        .entry(entity_type_id)
        .or_insert_with(|| (Vec::new(), Vec::new()));
    property_type_ids.push(property_type_id);
    if required {
        required_property_type_ids.push(property_type_id);
    }
}

/// Returns the pre-computed, frozen metadata for an entity type.
///
/// This registry is built during startup by property ctors calling
/// [`register_property_with_entity()`], then frozen exactly once on first read.
#[must_use]
pub fn get_entity_metadata_static(entity_type_id: TypeId) -> (&'static [TypeId], &'static [TypeId]) {
    match entity_metadata().get(&entity_type_id) {
        Some((props, reqs)) => (props.as_ref(), reqs.as_ref()),
        None => (&[], &[]),
    }
}

/// Adds a new entity to the registry. The job of this method is to create whatever
/// "singleton" data/metadata is associated with the [`Entity`] if it doesn't already
/// exist, which in this case is only the value of `Entity::id()`.
///
/// In our use case, this method is called in the `ctor` function of each `Entity`
/// type and ultimately exists only so that we know how many `EntityRecord`s to
/// construct in the constructor of `EntityStore`, so that we never have to mutate
/// `EntityStore` itself when an `Entity` is accessed for the first time. (The
/// `OnceCell`s handle the interior mutability required for initialization.)
pub fn add_to_entity_registry<R: Entity>() {
    let _ = R::id();
}

/// A convenience getter for `NEXT_ENTITY_INDEX`.
pub fn get_registered_entity_count() -> usize {
    *NEXT_ENTITY_INDEX.lock().unwrap()
}

/// Encapsulates the synchronization logic for initializing an entity's index.
///
/// Acquires a global lock on the next available item index, but only increments
/// it if we successfully initialize the provided index. The `index` of a registered
/// item is assigned at runtime but only once per type. It's possible for a single
/// type to attempt to initialize its index multiple times from different threads,
/// which is why all this synchronization is required. However, the overhead
/// is negligible, as this initialization only happens once upon first access.
///
/// In fact, for our use case we know we are calling this function
/// once for each type in each `Entity`'s `ctor` function, which
/// should be the only time this method is ever called for the type.
pub fn initialize_entity_index(plugin_index: &AtomicUsize) -> usize {
    // Acquire a global lock.
    let mut guard = NEXT_ENTITY_INDEX.lock().unwrap();
    let candidate = *guard;

    // Try to claim the candidate index. Here we guard against the potential race condition that
    // another instance of this plugin in another thread just initialized the index prior to us
    // obtaining the lock. If the index has been initialized beneath us, we do not update
    // [`NEXT_ITEM_INDEX`], we just return the value `plugin_index` was initialized to.
    // For a justification of the data ordering, see:
    //     https://github.com/CDCgov/ixa/pull/477#discussion_r2244302872
    match plugin_index.compare_exchange(usize::MAX, candidate, Ordering::AcqRel, Ordering::Acquire)
    {
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

/// We store our own instance data alongside the `Entity` instance itself.
pub struct EntityRecord {
    /// The total count of all entities of this type (i.e., the next index to assign).
    pub(crate) entity_count: usize,
    /// Lazily initialized `Entity` instance.
    pub(crate) entity: OnceCell<Box<dyn Any>>,
    /// A type-erased `Box<PropertyStore<E>>`, lazily initialized.
    pub(crate) property_store: OnceCell<Box<dyn Any>>,
}

impl EntityRecord {
    pub(crate) fn new() -> Self {
        Self {
            entity_count: 0,
            entity: OnceCell::new(),
            property_store: OnceCell::new(),
        }
    }
}

/// A wrapper around a vector of entities.
pub struct EntityStore {
    items: Vec<EntityRecord>,
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
    pub fn new() -> Self {
        let num_items = get_registered_entity_count();
        Self {
            items: (0..num_items).map(|_| EntityRecord::new()).collect(),
        }
    }

    /// Fetches an immutable reference to the entity `E` from the registry. This
    /// implementation lazily instantiates the item if it has not yet been instantiated.
    #[must_use]
    pub fn get<E: Entity>(&self) -> &E {
        let index = E::id();
        self.items
        .get(index)
        .unwrap_or_else(|| panic!("No registered entity found with index = {index:?}. You must use the `define_entity!` macro to create an entity."))
        .entity
        .get_or_init(|| E::new_boxed())
        .downcast_ref::<E>()
        .expect("TypeID does not match registered entity type. You must use the `define_entity!` macro to create an entity.")
    }

    /// Fetches a mutable reference to the item `E` from the registry. This
    /// implementation lazily instantiates the item if it has not yet been instantiated.
    #[must_use]
    pub fn get_mut<E: Entity>(&mut self) -> &mut E {
        let index = E::id();

        // If the item is already initialized, return a mutable reference.
        if self.items[index].entity.get().is_some() {
            return self.items[index]
                .entity
                .get_mut()
                .unwrap()
                .downcast_mut()
                .expect("TypeID does not match registered entity type. You must use the `define_entity!` macro to create an entity.");
        }

        // Initialize the item.
        let record = &mut self.items[index];
        let _ = record.entity.set(E::new_boxed());
        record
            .entity
            .get_mut()
            .unwrap()
            .downcast_mut::<E>()
            .expect("TypeID does not match registered entity type. You must use the `define_entity!` macro to create an entity.")
    }

    /// Creates a new `EntityId` for the given `Entity` type `E`.
    /// Increments the entity counter and returns the next valid ID.
    pub(crate) fn new_entity_id<E: Entity>(&mut self) -> EntityId<E> {
        let index = E::id();
        let record = &mut self.items[index];
        let id = record.entity_count;
        record.entity_count += 1;
        EntityId::new(id)
    }

    /// Returns a total count of all created entities of type `E`.
    #[must_use]
    pub fn get_entity_count<E: Entity>(&self) -> usize {
        let index = E::id();
        let record = &self.items[index];
        record.entity_count
    }

    /// Returns a total count of all created entities of type `E`.
    #[must_use]
    pub fn get_entity_count_by_id(&self, id: usize) -> usize {
        let record = &self.items[id];
        record.entity_count
    }

    /// Returns an iterator over all valid `EntityId<E>`s
    pub fn get_entity_iterator<E: Entity>(&self) -> EntityIterator<E> {
        let count = self.get_entity_count::<E>();
        EntityIterator::new(count)
    }

    pub fn get_property_store<E: Entity>(&self) -> &PropertyStore<E> {
        let index = E::id();
        let record = self.items
                         .get(index)
                         .unwrap_or_else(|| panic!("No registered entity found with index = {index:?}. You must use the `define_entity!` macro to create an entity."));
        let property_store = record
            .property_store
            .get_or_init(|| Box::new(PropertyStore::<E>::new()));
        property_store.downcast_ref::<PropertyStore<E>>()
                      .expect("TypeID does not match registered item type. You must use the `define_registered_item!` macro to create a registered item.")
    }

    pub fn get_property_store_mut<E: Entity>(&mut self) -> &mut PropertyStore<E> {
        let index = E::id();
        let record = self.items
                         .get_mut(index)
                         .unwrap_or_else(|| panic!("No registered entity found with index = {index:?}. You must use the `define_entity!` macro to create an entity."));
        let _ = record
            .property_store
            .get_or_init(|| Box::new(PropertyStore::<E>::new()));
        let property_store = record.property_store.get_mut().unwrap();
        property_store.downcast_mut::<PropertyStore<E>>()
                      .expect("TypeID does not match registered item type. You must use the `define_registered_item!` macro to create a registered item.")
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread;

    use crate::entity::entity_store::{
        add_to_entity_registry, get_registered_entity_count, initialize_entity_index, EntityStore,
    };
    use crate::entity::{impl_entity, ContextEntitiesExt, Entity};
    use crate::{Context, HashMap};
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

    // Test the internal synchronization mechanisms of `initialize_entity_index()`.
    //
    // It is convenient to only have a single test that mutates `NEXT_ENTITY_INDEX`,
    // because we can assume no other thread is incrementing it and can therefore
    // test the value of `NEXT_ENTITY_INDEX` at the beginning and then at the end of
    // the test.
    //
    // Note that this doesn't really interfere with other tests involving `EntityStore`,
    // because at worst `EntityStore` will just allocate addition slots for
    // nonexistent items, which will never be requested with a `get()` call.
    #[test]
    fn test_initialize_item_index_concurrent() {
        // Test 1: Try to initialize a single index from multiple threads simultaneously.
        let initial_registered_items_count = get_registered_entity_count();

        const NUM_THREADS: usize = 100;
        let index = Arc::new(AtomicUsize::new(usize::MAX));
        let barrier = Arc::new(Barrier::new(NUM_THREADS));

        let handles: Vec<_> = (0..NUM_THREADS)
            .map(|_| {
                let index_clone = Arc::clone(&index);
                let barrier_clone = Arc::clone(&barrier);

                thread::spawn(move || {
                    // Wait for all threads to be ready
                    barrier_clone.wait();
                    // All threads try to initialize at once
                    initialize_entity_index(&index_clone)
                })
            })
            .collect();

        let results: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        let first = results[0];

        // The index should be initialized
        assert_ne!(first, usize::MAX);
        // All threads should get the same index
        assert!(results.iter().all(|&r| r == first));
        // And that index should be what was originally the next available index
        assert_eq!(first, initial_registered_items_count);

        // Test 2: Try to initialize multiple indices from multiple threads simultaneously.
        //
        // Creates 5 different entities (each with their own atomic). Initializes
        // each from a separate thread. Verifies they receive sequential,
        // unique indices. Confirms the global counter matches the entity count.

        // W
        let initial_registered_items_count = get_registered_entity_count();

        // Create multiple different entities (each with their own atomic)
        const NUM_ENTITIES: usize = 5;
        let entities: Vec<_> = (0..NUM_ENTITIES)
            .map(|_| Arc::new(AtomicUsize::new(usize::MAX)))
            .collect();

        let mut handles = vec![];

        // Initialize each entity from a different thread
        for entity in entities.iter() {
            let entity_clone = Arc::clone(entity);
            let handle = thread::spawn(move || initialize_entity_index(&entity_clone));
            handles.push(handle);
        }

        // Collect results
        let mut results = vec![];
        for handle in handles {
            results.push(handle.join().unwrap());
        }

        // Each entity should get a unique, sequential index starting with `initial_registered_items_count`.
        results.sort();
        for (i, &result) in results.iter().enumerate() {
            assert_eq!(
                result,
                i + initial_registered_items_count,
                "Entity should have index {}, got {}",
                i,
                result
            );
        }

        // Test 3: Try to initialize multiple entities from multiple threads multiple times.

        // We account for the fact that some entities have been initialized
        // in their `ctors`, so the indices we create don't start with 0.
        let initial_registered_items_count = get_registered_entity_count();

        // Create 3 entities
        let entity1 = Arc::new(AtomicUsize::new(usize::MAX));
        let entity2 = Arc::new(AtomicUsize::new(usize::MAX));
        let entity3 = Arc::new(AtomicUsize::new(usize::MAX));

        let mut handles = vec![];

        // Multiple threads racing on each of entity1, entity2, entity3
        for _ in 0..5 {
            let e1 = Arc::clone(&entity1);
            handles.push(thread::spawn(move || initialize_entity_index(&e1)));

            let e2 = Arc::clone(&entity2);
            handles.push(thread::spawn(move || initialize_entity_index(&e2)));

            let e3 = Arc::clone(&entity3);
            handles.push(thread::spawn(move || initialize_entity_index(&e3)));
        }

        // Collect all results
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Count occurrences of each index
        let mut counts = HashMap::default();
        for &result in &results {
            *counts.entry(result).or_insert(0) += 1;
        }

        // Should have exactly 3 unique indices
        assert_eq!(counts.len(), 3, "Should have 3 unique indices");

        // Each index should appear exactly 5 times (one entity, 5 threads)
        for (&idx, &count) in &counts {
            assert_eq!(
                count, 5,
                "Index {} should appear 5 times, appeared {} times",
                idx, count
            );
        }

        // Global counter should be 3
        assert_eq!(
            get_registered_entity_count() - initial_registered_items_count,
            3
        );

        // Each entity should have one of the indices
        let indices: Vec<_> = vec![
            entity1.load(Ordering::Acquire),
            entity2.load(Ordering::Acquire),
            entity3.load(Ordering::Acquire),
        ];

        let mut sorted_indices = indices.clone();
        sorted_indices.sort_unstable();
        // As before, we account for the fact that some entities have been
        // initialized in their `ctors`, so the indices we created don't start at 0.
        let expected_indices = vec![
            initial_registered_items_count,
            1 + initial_registered_items_count,
            2 + initial_registered_items_count,
        ];
        assert_eq!(sorted_indices, expected_indices);
    }

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
            context.add_entity::<TestItem1, _>(()).unwrap();
        }
        for _ in 0..3 {
            context.add_entity::<TestItem2, _>(()).unwrap();
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

        context.add_entity::<TestItem1, _>(()).unwrap();

        assert_eq!(context.get_entity_count::<TestItem1>(), 6);
        assert_eq!(snapshot_iter.count(), 5); // Still sees original population
        assert_eq!(context.get_entity_iterator::<TestItem1>().count(), 6); // New iterator sees 6
    }
}
