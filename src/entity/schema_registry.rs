//! Process-wide registration for entity and property schema.
//!
//! Startup constructors populate a mutable builder. The first runtime consumer freezes that
//! builder into dense immutable slices used to construct context-owned stores. Registration code
//! prepares every value that can recursively request another ID before entering the builder lock.

use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};

use super::property::Property;
use super::property_store::PropertyStore;
use super::property_store_core::PropertyStoreCore;
use super::property_value_store::PropertyValueStore;
use super::property_value_store_core::PropertyValueStoreCore;
use super::Entity;

pub(in crate::entity) type PropertyStoreConstructor = fn() -> Box<dyn PropertyStore>;

// The destination vector depends on E and therefore cannot appear in this non-generic function
// pointer type. The installer recovers the typed vector once during PropertyStoreCore<E>
// construction; property access and mutation never perform this downcast.
pub(in crate::entity) type PropertyValueStoreInstaller = fn(&mut dyn Any);

#[derive(Clone, Copy)]
struct EntityDescriptor {
    concrete_entity_type_id: TypeId,
    property_store_constructor: PropertyStoreConstructor,
}

#[derive(Clone, Copy)]
struct PropertyDescriptor {
    concrete_entity_type_id: TypeId,
    concrete_property_type_id: TypeId,
    property_type_id: TypeId,
    name: &'static str,
    required: bool,
    value_store_installer: PropertyValueStoreInstaller,
}

struct EntityRegistrationBuilder {
    concrete_entity_type_id: TypeId,
    property_store_constructor: PropertyStoreConstructor,
    properties: Vec<PropertyRegistrationBuilder>,
}

struct PropertyRegistrationBuilder {
    concrete_property_type_id: TypeId,
    property_type_id: TypeId,
    name: &'static str,
    required: bool,
    value_store_installer: PropertyValueStoreInstaller,
    dependent_property_ids: Vec<usize>,
}

#[derive(Default)]
struct SchemaRegistryBuilder {
    entities: Vec<EntityRegistrationBuilder>,
}

struct FrozenSchemaRegistry {
    entities: Box<[EntityRegistration]>,
}

pub(in crate::entity) struct EntityRegistration {
    pub(in crate::entity) property_store_constructor: PropertyStoreConstructor,
    pub(in crate::entity) properties: Box<[PropertyRegistration]>,
    pub(in crate::entity) property_type_ids: Box<[TypeId]>,
    pub(in crate::entity) required_property_type_ids: Box<[TypeId]>,
}

pub(in crate::entity) struct PropertyRegistration {
    #[cfg(feature = "profiling")]
    pub(in crate::entity) property_type_id: TypeId,
    #[cfg(feature = "profiling")]
    pub(in crate::entity) name: &'static str,
    pub(in crate::entity) value_store_installer: PropertyValueStoreInstaller,
    pub(in crate::entity) dependent_property_ids: Box<[usize]>,
}

pub(in crate::entity) struct SchemaRegistry {
    builder: Mutex<Option<SchemaRegistryBuilder>>,
    frozen: OnceLock<FrozenSchemaRegistry>,
}

impl SchemaRegistry {
    const fn new() -> Self {
        Self {
            builder: Mutex::new(Some(SchemaRegistryBuilder {
                entities: Vec::new(),
            })),
            frozen: OnceLock::new(),
        }
    }

    /// Installs or validates one complete entity registration and returns its dense ID.
    ///
    /// Callers must prepare the descriptor before entering this method. Nothing executed while
    /// the builder mutex is held may call an entity/property ID method or another registry.
    fn register_entity(&self, entity_index: &AtomicUsize, descriptor: EntityDescriptor) -> usize {
        // Lock only after all potentially user-defined or recursive work has completed.
        let mut guard = self.builder.lock().unwrap();
        let builder = guard.as_mut().unwrap_or_else(|| {
            panic!("Ixa internal error: entity registration attempted after schema freeze")
        });

        // Recheck under the registry lock: another thread may have completed this registration
        // after the generated `id()` fast path observed the uninitialized sentinel.
        let existing_id = entity_index.load(Ordering::Acquire);
        if existing_id != usize::MAX {
            Self::validate_entity(builder, existing_id, descriptor);
            return existing_id;
        }

        if let Some(existing_id) = builder
            .entities
            .iter()
            .position(|entry| entry.concrete_entity_type_id == descriptor.concrete_entity_type_id)
        {
            Self::validate_entity(builder, existing_id, descriptor);
            entity_index.store(existing_id, Ordering::Release);
            return existing_id;
        }

        let id = builder.entities.len();
        builder.entities.push(EntityRegistrationBuilder {
            concrete_entity_type_id: descriptor.concrete_entity_type_id,
            property_store_constructor: descriptor.property_store_constructor,
            properties: Vec::new(),
        });

        // Publish only after the slot is complete. Runtime schema visibility comes from the
        // registry mutex and freeze boundary; this atomic is the generated ID cache.
        entity_index.store(id, Ordering::Release);
        id
    }

    fn validate_entity(
        builder: &SchemaRegistryBuilder,
        entity_id: usize,
        descriptor: EntityDescriptor,
    ) {
        let registration = builder.entities.get(entity_id).unwrap_or_else(|| {
            panic!("Ixa internal error: entity ID {entity_id} has no schema registration")
        });
        assert_eq!(
            registration.concrete_entity_type_id, descriptor.concrete_entity_type_id,
            "Ixa internal error: multiple entity types registered at index {entity_id}"
        );
    }

    /// Installs or validates one complete property registration and returns its dense entity-local
    /// ID.
    ///
    /// The entity and dependency IDs and all descriptor fields must be prepared before this call.
    /// Nothing executed while the builder mutex is held may call an entity/property ID method or
    /// another registry.
    fn register_property(
        &self,
        entity_id: usize,
        property_index: &AtomicUsize,
        descriptor: PropertyDescriptor,
        dependency_ids: Vec<usize>,
    ) -> usize {
        // Dependency discovery can recursively register other properties, so it must have
        // completed before this lock is acquired.
        let mut guard = self.builder.lock().unwrap();
        let builder = guard.as_mut().unwrap_or_else(|| {
            panic!("Ixa internal error: property registration attempted after schema freeze")
        });
        let entity = builder.entities.get_mut(entity_id).unwrap_or_else(|| {
            panic!("Ixa internal error: property registration used unknown entity ID {entity_id}")
        });
        assert_eq!(
            entity.concrete_entity_type_id, descriptor.concrete_entity_type_id,
            "Ixa internal error: property registered with the wrong entity ID"
        );

        // Recheck under the lock for a concurrent registration that won the race.
        let existing_id = property_index.load(Ordering::Acquire);
        if existing_id != usize::MAX {
            Self::validate_property(entity, existing_id, descriptor);
            return existing_id;
        }

        if let Some(existing_id) = entity.properties.iter().position(|entry| {
            entry.concrete_property_type_id == descriptor.concrete_property_type_id
        }) {
            Self::validate_property(entity, existing_id, descriptor);
            property_index.store(existing_id, Ordering::Release);
            return existing_id;
        }

        // Dependency discovery is generic over this entity type, so the prepared IDs belong to
        // this entity's namespace. Bare entity-local IDs retain no additional provenance here; the
        // locked mutation can validate only bounds and uniqueness.
        for (position, &dependency_id) in dependency_ids.iter().enumerate() {
            assert!(
                dependency_id < entity.properties.len(),
                "Ixa internal error: property dependency ID {dependency_id} is not registered for entity ID {entity_id}"
            );
            assert!(
                !dependency_ids[..position].contains(&dependency_id),
                "Ixa internal error: duplicate property dependency ID {dependency_id} for entity ID {entity_id}"
            );
        }

        let id = entity.properties.len();
        entity.properties.push(PropertyRegistrationBuilder {
            concrete_property_type_id: descriptor.concrete_property_type_id,
            property_type_id: descriptor.property_type_id,
            name: descriptor.name,
            required: descriptor.required,
            value_store_installer: descriptor.value_store_installer,
            dependent_property_ids: Vec::new(),
        });

        for dependency_id in dependency_ids {
            entity.properties[dependency_id]
                .dependent_property_ids
                .push(id);
        }

        // Publish only after the property slot and all reverse dependency edges are complete.
        property_index.store(id, Ordering::Release);
        id
    }

    fn validate_property(
        entity: &EntityRegistrationBuilder,
        property_id: usize,
        descriptor: PropertyDescriptor,
    ) {
        let registration = entity.properties.get(property_id).unwrap_or_else(|| {
            panic!("Ixa internal error: property ID {property_id} has no schema registration")
        });
        assert_eq!(
            registration.concrete_property_type_id, descriptor.concrete_property_type_id,
            "Ixa internal error: multiple property types registered at index {property_id}"
        );
        assert_eq!(
            registration.property_type_id, descriptor.property_type_id,
            "Ixa internal error: conflicting property TypeIds for one registration"
        );
        assert_eq!(
            registration.name, descriptor.name,
            "Ixa internal error: conflicting property names for one registration"
        );
        assert_eq!(
            registration.required, descriptor.required,
            "Ixa internal error: conflicting required status for one property registration"
        );
    }

    /// Returns the immutable schema, freezing startup registration on first access.
    ///
    /// Taking the builder under its mutex is the registration close point. Conversion happens
    /// after releasing that mutex so allocation cannot block registration code; constructors and
    /// installers run only after this method returns.
    fn frozen(&self) -> &FrozenSchemaRegistry {
        self.frozen.get_or_init(|| {
            let builder = {
                let mut guard = self.builder.lock().unwrap();
                // The registry is closed while holding the same mutex used by registration.
                guard.take().unwrap_or_else(|| {
                    panic!("Ixa internal error: schema registry freeze re-entered")
                })
            };

            // No registry lock is held while allocating the immutable view.
            FrozenSchemaRegistry::from_builder(builder)
        })
    }

    pub(in crate::entity) fn entity_registrations(&self) -> &[EntityRegistration] {
        &self.frozen().entities
    }

    pub(in crate::entity) fn entity_registration<E: Entity>(&self) -> &EntityRegistration {
        let entity_id = E::id();
        self.frozen()
            .entities
            .get(entity_id)
            .unwrap_or_else(|| panic!("No registered entity found with index = {entity_id:?}"))
    }

    #[cfg(feature = "profiling")]
    pub(in crate::entity) fn registered_property_name(
        &self,
        entity_id: usize,
        property_type_id: TypeId,
    ) -> &'static str {
        self.frozen()
            .entities
            .get(entity_id)
            .unwrap_or_else(|| panic!("No registered entity found with index = {entity_id:?}"))
            .properties
            .iter()
            .find(|registration| registration.property_type_id == property_type_id)
            .map(|registration| registration.name)
            .unwrap_or_else(|| {
                panic!(
                    "No registered property name for entity ID {entity_id} and property type ID {property_type_id:?}"
                )
            })
    }
}

impl FrozenSchemaRegistry {
    fn from_builder(builder: SchemaRegistryBuilder) -> Self {
        let entities = builder
            .entities
            .into_iter()
            .map(|entity| {
                let property_type_ids = entity
                    .properties
                    .iter()
                    .map(|property| property.property_type_id)
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                let required_property_type_ids = entity
                    .properties
                    .iter()
                    .filter(|property| property.required)
                    .map(|property| property.property_type_id)
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                let properties = entity
                    .properties
                    .into_iter()
                    .map(|property| PropertyRegistration {
                        #[cfg(feature = "profiling")]
                        property_type_id: property.property_type_id,
                        #[cfg(feature = "profiling")]
                        name: property.name,
                        value_store_installer: property.value_store_installer,
                        dependent_property_ids: property.dependent_property_ids.into_boxed_slice(),
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();

                EntityRegistration {
                    property_store_constructor: entity.property_store_constructor,
                    properties,
                    property_type_ids,
                    required_property_type_ids,
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self { entities }
    }
}

pub(in crate::entity) static SCHEMA_REGISTRY: LazyLock<SchemaRegistry> =
    LazyLock::new(SchemaRegistry::new);

fn install_property_value_store<E, P>(items: &mut dyn Any)
where
    E: Entity,
    P: Property<E>,
{
    let items = items
        .downcast_mut::<Vec<Box<dyn PropertyValueStore<E>>>>()
        .expect("Ixa internal error: property installer received the wrong entity store");
    items.push(PropertyValueStoreCore::<E, P>::new_boxed());
}

/// Ensures that `E` is registered during startup.
///
/// Generated entity ctors call this function so their implementation remains inside Ixa and
/// zero-property entities are registered even when nothing else references them. The function
/// deliberately delegates to `E::id()`: another type's ctor may request the entity ID before
/// `E`'s own ctor runs, so complete registration must also be reachable from the `id()` slow path.
#[doc(hidden)]
pub fn add_to_entity_registry<E: Entity>() {
    let _ = E::id();
}

/// Returns the dense ID for `E`, registering its complete schema entry if necessary.
///
/// This is the slow path for generated `Entity::id()` implementations. Registration, rather than
/// ID allocation alone, happens here because `E::id()` may be called before `E`'s startup ctor.
/// The descriptor is prepared before entering `SchemaRegistry` so the registry mutex protects only
/// non-recursive state mutation.
#[doc(hidden)]
#[cold]
#[inline(never)]
pub fn ensure_entity_registered<E: Entity>(entity_index: &AtomicUsize) -> usize {
    let descriptor = EntityDescriptor {
        concrete_entity_type_id: TypeId::of::<E>(),
        property_store_constructor: PropertyStoreCore::<E>::new_boxed,
    };
    SCHEMA_REGISTRY.register_entity(entity_index, descriptor)
}

/// Ensures that property `P` of entity `E` is registered during startup.
///
/// Generated property ctors call this function to keep registration orchestration inside Ixa. It
/// deliberately delegates to `P::id()`: dependency discovery may request a property's ID before
/// that property's own ctor runs, so the `id()` slow path must be able to install the complete
/// property registration without waiting for this ctor.
#[doc(hidden)]
pub fn add_to_property_registry<E, P>()
where
    E: Entity,
    P: Property<E>,
{
    let _ = P::id();
}

/// Returns the entity-local dense ID for `P`, registering its complete schema entry if necessary.
///
/// This is the slow path for generated `Property::id()` implementations. It resolves the owning
/// entity and all non-derived dependency IDs before entering `SchemaRegistry`, because either step
/// may recursively register another type. The registry receives only prepared data and performs
/// one non-recursive locked mutation.
#[doc(hidden)]
#[cold]
#[inline(never)]
pub fn ensure_property_registered<E, P>(property_index: &AtomicUsize) -> usize
where
    E: Entity,
    P: Property<E>,
{
    // These calls may recursively enter entity/property registration. They must remain before the
    // schema registry call so no recursive attempt can acquire the schema builder mutex.
    let entity_id = E::id();
    let dependency_ids = P::non_derived_dependencies();

    // Evaluate trait-provided metadata outside the lock as well. `register_property` receives only
    // inert data and function pointers and never invokes arbitrary trait implementations.
    let descriptor = PropertyDescriptor {
        concrete_entity_type_id: TypeId::of::<E>(),
        concrete_property_type_id: TypeId::of::<P>(),
        property_type_id: P::type_id(),
        name: P::name(),
        required: P::is_required(),
        value_store_installer: install_property_value_store::<E, P>,
    };

    SCHEMA_REGISTRY.register_property(entity_id, property_index, descriptor, dependency_ids)
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread;

    use super::{
        EntityDescriptor, PropertyDescriptor, PropertyStore, PropertyValueStoreInstaller,
        SchemaRegistry,
    };

    struct DummyStore;

    impl PropertyStore for DummyStore {
        fn allocate_entity_id(&mut self) -> (usize, bool) {
            (0, false)
        }

        fn entity_count(&self) -> usize {
            0
        }
    }

    fn new_dummy_store() -> Box<dyn PropertyStore> {
        Box::new(DummyStore)
    }

    fn ignore_installer(_: &mut dyn std::any::Any) {}

    fn install_marker(items: &mut dyn std::any::Any) {
        items
            .downcast_mut::<Vec<&'static str>>()
            .expect("marker installer received the wrong destination")
            .push("installed");
    }

    fn entity_descriptor<T: 'static>() -> EntityDescriptor {
        EntityDescriptor {
            concrete_entity_type_id: TypeId::of::<T>(),
            property_store_constructor: new_dummy_store,
        }
    }

    fn property_descriptor<E: 'static, P: 'static>(
        name: &'static str,
        required: bool,
    ) -> PropertyDescriptor {
        PropertyDescriptor {
            concrete_entity_type_id: TypeId::of::<E>(),
            concrete_property_type_id: TypeId::of::<P>(),
            property_type_id: TypeId::of::<P>(),
            name,
            required,
            value_store_installer: ignore_installer as PropertyValueStoreInstaller,
        }
    }

    struct EntityA;
    struct EntityB;
    struct PropertyA;
    struct PropertyB;
    struct PropertyC;

    #[test]
    fn concurrent_entity_registration_is_dense_and_idempotent() {
        let registry = Arc::new(SchemaRegistry::new());
        let shared_index = Arc::new(AtomicUsize::new(usize::MAX));
        let barrier = Arc::new(Barrier::new(16));
        let handles = (0..16)
            .map(|_| {
                let registry = Arc::clone(&registry);
                let shared_index = Arc::clone(&shared_index);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    registry.register_entity(&shared_index, entity_descriptor::<EntityA>())
                })
            })
            .collect::<Vec<_>>();

        let ids = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert!(ids.iter().all(|id| *id == 0));

        let second_index = AtomicUsize::new(usize::MAX);
        assert_eq!(
            registry.register_entity(&second_index, entity_descriptor::<EntityA>()),
            0
        );
        assert_eq!(registry.frozen().entities.len(), 1);
    }

    #[test]
    fn concurrent_distinct_entities_receive_unique_dense_ids() {
        let registry = SchemaRegistry::new();
        let entity_a_index = AtomicUsize::new(usize::MAX);
        let entity_b_index = AtomicUsize::new(usize::MAX);
        let barrier = Barrier::new(2);
        let mut ids = thread::scope(|scope| {
            let first = scope.spawn(|| {
                barrier.wait();
                registry.register_entity(&entity_a_index, entity_descriptor::<EntityA>())
            });
            let second = scope.spawn(|| {
                barrier.wait();
                registry.register_entity(&entity_b_index, entity_descriptor::<EntityB>())
            });
            vec![first.join().unwrap(), second.join().unwrap()]
        });
        ids.sort_unstable();
        assert_eq!(ids, [0, 1]);
    }

    #[test]
    fn entity_and_property_namespaces_are_independently_dense() {
        let registry = SchemaRegistry::new();
        let entity_a_index = AtomicUsize::new(usize::MAX);
        let entity_b_index = AtomicUsize::new(usize::MAX);
        let entity_a = registry.register_entity(&entity_a_index, entity_descriptor::<EntityA>());
        let entity_b = registry.register_entity(&entity_b_index, entity_descriptor::<EntityB>());
        assert_eq!((entity_a, entity_b), (0, 1));

        let property_a_for_a = AtomicUsize::new(usize::MAX);
        let property_b_for_a = AtomicUsize::new(usize::MAX);
        let property_a_for_b = AtomicUsize::new(usize::MAX);
        assert_eq!(
            registry.register_property(
                entity_a,
                &property_a_for_a,
                property_descriptor::<EntityA, PropertyA>("PropertyA", true),
                Vec::new(),
            ),
            0
        );
        assert_eq!(
            registry.register_property(
                entity_a,
                &property_b_for_a,
                property_descriptor::<EntityA, PropertyB>("PropertyB", false),
                Vec::new(),
            ),
            1
        );
        assert_eq!(
            registry.register_property(
                entity_b,
                &property_a_for_b,
                property_descriptor::<EntityB, PropertyA>("PropertyA", false),
                Vec::new(),
            ),
            0
        );

        let frozen = registry.frozen();
        assert_eq!(frozen.entities[0].properties.len(), 2);
        assert_eq!(frozen.entities[1].properties.len(), 1);
        assert_eq!(
            frozen.entities[0].required_property_type_ids.as_ref(),
            &[TypeId::of::<PropertyA>()]
        );
    }

    #[test]
    fn freezing_preserves_reverse_dependencies() {
        let registry = SchemaRegistry::new();
        let entity_index = AtomicUsize::new(usize::MAX);
        let entity_id = registry.register_entity(&entity_index, entity_descriptor::<EntityA>());
        let dependency_index = AtomicUsize::new(usize::MAX);
        let derived_index = AtomicUsize::new(usize::MAX);

        let dependency_id = registry.register_property(
            entity_id,
            &dependency_index,
            property_descriptor::<EntityA, PropertyA>("PropertyA", false),
            Vec::new(),
        );
        let derived_id = registry.register_property(
            entity_id,
            &derived_index,
            property_descriptor::<EntityA, PropertyB>("PropertyB", false),
            vec![dependency_id],
        );

        let frozen = registry.frozen();
        assert_eq!(
            frozen.entities[entity_id].properties[dependency_id]
                .dependent_property_ids
                .as_ref(),
            &[derived_id]
        );
    }

    #[test]
    fn concurrent_property_registration_is_idempotent() {
        let registry = Arc::new(SchemaRegistry::new());
        let entity_index = AtomicUsize::new(usize::MAX);
        let entity_id = registry.register_entity(&entity_index, entity_descriptor::<EntityA>());
        let property_index = Arc::new(AtomicUsize::new(usize::MAX));
        let barrier = Arc::new(Barrier::new(16));
        let handles = (0..16)
            .map(|_| {
                let registry = Arc::clone(&registry);
                let property_index = Arc::clone(&property_index);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    registry.register_property(
                        entity_id,
                        &property_index,
                        property_descriptor::<EntityA, PropertyA>("PropertyA", false),
                        Vec::new(),
                    )
                })
            })
            .collect::<Vec<_>>();

        let ids = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert!(ids.iter().all(|id| *id == 0));

        let second_index = AtomicUsize::new(usize::MAX);
        assert_eq!(
            registry.register_property(
                entity_id,
                &second_index,
                property_descriptor::<EntityA, PropertyA>("PropertyA", false),
                Vec::new(),
            ),
            0
        );
        assert_eq!(registry.frozen().entities[entity_id].properties.len(), 1);
    }

    #[test]
    fn independent_registries_start_each_namespace_at_zero() {
        for registry in [SchemaRegistry::new(), SchemaRegistry::new()] {
            let entity_index = AtomicUsize::new(usize::MAX);
            let entity_id = registry.register_entity(&entity_index, entity_descriptor::<EntityA>());
            let property_index = AtomicUsize::new(usize::MAX);
            let property_id = registry.register_property(
                entity_id,
                &property_index,
                property_descriptor::<EntityA, PropertyA>("PropertyA", false),
                Vec::new(),
            );
            assert_eq!((entity_id, property_id), (0, 0));
        }
    }

    #[test]
    fn freezing_preserves_complete_property_registration() {
        let registry = SchemaRegistry::new();
        let entity_index = AtomicUsize::new(usize::MAX);
        let entity_id = registry.register_entity(&entity_index, entity_descriptor::<EntityA>());
        let property_index = AtomicUsize::new(usize::MAX);
        let mut descriptor = property_descriptor::<EntityA, PropertyA>("PropertyA", true);
        descriptor.value_store_installer = install_marker;
        registry.register_property(entity_id, &property_index, descriptor, Vec::new());

        let frozen = registry.frozen();
        let entity = &frozen.entities[entity_id];
        assert_eq!(
            entity.property_type_ids.as_ref(),
            &[TypeId::of::<PropertyA>()]
        );
        assert_eq!(
            entity.required_property_type_ids.as_ref(),
            &[TypeId::of::<PropertyA>()]
        );
        assert!(entity.properties[0].dependent_property_ids.is_empty());
        #[cfg(feature = "profiling")]
        {
            assert_eq!(
                entity.properties[0].property_type_id,
                TypeId::of::<PropertyA>()
            );
            assert_eq!(entity.properties[0].name, "PropertyA");
        }

        let mut installed = Vec::<&'static str>::new();
        (entity.properties[0].value_store_installer)(&mut installed);
        assert_eq!(installed, ["installed"]);
    }

    #[test]
    fn concurrent_entity_and_property_registration_remain_dense() {
        let registry = SchemaRegistry::new();
        let entity_a_index = AtomicUsize::new(usize::MAX);
        let entity_a = registry.register_entity(&entity_a_index, entity_descriptor::<EntityA>());
        let entity_b_index = AtomicUsize::new(usize::MAX);
        let property_index = AtomicUsize::new(usize::MAX);
        let barrier = Barrier::new(2);

        let entity_b = thread::scope(|scope| {
            let register_entity = scope.spawn(|| {
                barrier.wait();
                registry.register_entity(&entity_b_index, entity_descriptor::<EntityB>())
            });
            let register_property = scope.spawn(|| {
                barrier.wait();
                registry.register_property(
                    entity_a,
                    &property_index,
                    property_descriptor::<EntityA, PropertyA>("PropertyA", false),
                    Vec::new(),
                )
            });

            assert_eq!(register_property.join().unwrap(), 0);
            register_entity.join().unwrap()
        });

        assert_eq!(entity_b, 1);
        let frozen = registry.frozen();
        assert_eq!(frozen.entities.len(), 2);
        assert_eq!(frozen.entities[entity_a].properties.len(), 1);
        assert!(frozen.entities[entity_b].properties.is_empty());
    }

    #[test]
    fn conflicting_property_slot_associations_fail() {
        let registry = SchemaRegistry::new();
        let entity_index = AtomicUsize::new(usize::MAX);
        let entity_id = registry.register_entity(&entity_index, entity_descriptor::<EntityA>());
        let property_index = AtomicUsize::new(usize::MAX);
        registry.register_property(
            entity_id,
            &property_index,
            property_descriptor::<EntityA, PropertyA>("PropertyA", false),
            Vec::new(),
        );

        let result = std::panic::catch_unwind(|| {
            registry.register_property(
                entity_id,
                &property_index,
                property_descriptor::<EntityA, PropertyB>("PropertyB", false),
                Vec::new(),
            );
        });
        assert!(result.is_err());
    }

    #[test]
    fn conflicting_slot_associations_fail() {
        let registry = SchemaRegistry::new();
        let entity_index = AtomicUsize::new(usize::MAX);
        registry.register_entity(&entity_index, entity_descriptor::<EntityA>());

        let result = std::panic::catch_unwind(|| {
            registry.register_entity(&entity_index, entity_descriptor::<EntityB>());
        });
        assert!(result.is_err());
    }

    #[test]
    fn zero_property_entities_freeze_and_late_registration_fails() {
        let registry = SchemaRegistry::new();
        let entity_index = AtomicUsize::new(usize::MAX);
        registry.register_entity(&entity_index, entity_descriptor::<EntityA>());
        assert!(registry.frozen().entities[0].properties.is_empty());

        let late_index = AtomicUsize::new(usize::MAX);
        let result = std::panic::catch_unwind(|| {
            registry.register_entity(&late_index, entity_descriptor::<EntityB>());
        });
        assert!(result.is_err());
    }

    #[test]
    fn property_registration_after_freeze_fails() {
        let registry = SchemaRegistry::new();
        let entity_index = AtomicUsize::new(usize::MAX);
        let entity_id = registry.register_entity(&entity_index, entity_descriptor::<EntityA>());
        assert!(registry.frozen().entities[entity_id].properties.is_empty());

        let property_index = AtomicUsize::new(usize::MAX);
        let result = std::panic::catch_unwind(|| {
            registry.register_property(
                entity_id,
                &property_index,
                property_descriptor::<EntityA, PropertyA>("PropertyA", false),
                Vec::new(),
            );
        });
        assert!(result.is_err());
    }

    #[test]
    fn freeze_race_either_includes_registration_or_closes_before_it() {
        let registry = Arc::new(SchemaRegistry::new());
        let entity_a_index = AtomicUsize::new(usize::MAX);
        registry.register_entity(&entity_a_index, entity_descriptor::<EntityA>());
        let entity_b_index = Arc::new(AtomicUsize::new(usize::MAX));
        let barrier = Arc::new(Barrier::new(2));

        let registration = {
            let registry = Arc::clone(&registry);
            let entity_b_index = Arc::clone(&entity_b_index);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                std::panic::catch_unwind(|| {
                    registry.register_entity(&entity_b_index, entity_descriptor::<EntityB>())
                })
            })
        };
        barrier.wait();
        let frozen_count = registry.frozen().entities.len();
        let registration = registration.join().unwrap();

        match registration {
            Ok(entity_b_id) => {
                assert_eq!(entity_b_id, 1);
                assert_eq!(frozen_count, 2);
            }
            Err(_) => {
                assert_eq!(entity_b_index.load(Ordering::Acquire), usize::MAX);
                assert_eq!(frozen_count, 1);
            }
        }
    }

    #[test]
    fn invalid_dependency_is_rejected() {
        let registry = SchemaRegistry::new();
        let entity_index = AtomicUsize::new(usize::MAX);
        let entity_id = registry.register_entity(&entity_index, entity_descriptor::<EntityA>());
        let property_index = AtomicUsize::new(usize::MAX);

        let result = std::panic::catch_unwind(|| {
            registry.register_property(
                entity_id,
                &property_index,
                property_descriptor::<EntityA, PropertyC>("PropertyC", false),
                vec![7],
            );
        });
        assert!(result.is_err());
    }

    #[test]
    fn duplicate_dependency_is_rejected() {
        let registry = SchemaRegistry::new();
        let entity_index = AtomicUsize::new(usize::MAX);
        let entity_id = registry.register_entity(&entity_index, entity_descriptor::<EntityA>());
        let dependency_index = AtomicUsize::new(usize::MAX);
        let dependency_id = registry.register_property(
            entity_id,
            &dependency_index,
            property_descriptor::<EntityA, PropertyA>("PropertyA", false),
            Vec::new(),
        );
        let property_index = AtomicUsize::new(usize::MAX);

        let result = std::panic::catch_unwind(|| {
            registry.register_property(
                entity_id,
                &property_index,
                property_descriptor::<EntityA, PropertyB>("PropertyB", false),
                vec![dependency_id, dependency_id],
            );
        });
        assert!(result.is_err());
    }
}
