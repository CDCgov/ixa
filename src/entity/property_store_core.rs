/*!

A [`PropertyStoreCore`] implements the registry pattern for property value stores: A [`PropertyStoreCore`]
wraps a vector of `PropertyValueStore`s, one for each concrete property type. The implementor
of [`crate::entity::property::Property`] is the value type. Since there's a 1-1 correspondence between property types
and their value stores, we assign an ID to each property type to make
property lookup fast. The [`PropertyStoreCore`] stores a list of all properties in the form of
boxed `PropertyValueStore` instances, which provide a type-erased interface to the backing
storage (including index) of the property. Storage is only allocated as-needed, so the
instantiation of a `PropertyValueStore` for a property that is never used is negligible.
There's no need, then, for lazy initialization of the `PropertyValueStore`s themselves.

Property schema shared by every [`crate::context::Context`] is registered before `main()` using
`ctor` functions. Generated ctors call
[`add_to_property_registry()`](crate::entity::schema_registry::add_to_property_registry), which
delegates to the property's cached ID. On a cache miss the ID slow path prepares the complete
property descriptor and dependency list, then installs them together in the schema registry. The
first runtime schema read freezes registration into immutable dense slices.

*/

use std::any::{Any, TypeId};
use std::cell::OnceCell;

use crate::data_structures::bit_set::BitSet;
use crate::entity::entity::Entity;
use crate::entity::events::PartialPropertyChangeEventBox;
use crate::entity::index::{IndexCountResult, IndexSetResult, PropertyIndex};
use crate::entity::property::{IndexableProperty, Property};
use crate::entity::property_list::PropertyList;
use crate::entity::property_store::PropertyStore;
use crate::entity::property_value_store::PropertyValueStore;
use crate::entity::property_value_store_core::PropertyValueStoreCore;
use crate::entity::schema_registry::{PropertyRegistration, SCHEMA_REGISTRY};
use crate::entity::value_change_counter::StratifiedValueChangeCounter;
use crate::entity::EntityId;
use crate::{Context, ContextEntitiesExt};

pub(in crate::entity) type IndexNewEntityFn<E> = fn(&mut Context, EntityId<E>);

fn index_new_entity<E, P>(context: &mut Context, entity_id: EntityId<E>)
where
    E: Entity,
    P: IndexableProperty<E>,
{
    // This may compute a derived or multi-property. `P` is copied, so no reference into Context
    // survives into the subsequent mutable borrow.
    let value: P = context.get_property(entity_id);

    let property_value_store = context.get_property_value_store_mut::<E, P>();
    let index = property_value_store
        .index
        .as_mut()
        .expect("Ixa internal error: index_new_entity dispatch invoked for an unindexed property");

    index.add_entity(&value, entity_id);
}

/// A wrapper around a vector of property value stores.
pub struct PropertyStoreCore<E: Entity> {
    /// The total count of all entities of this type (i.e., the next index to assign).
    pub(crate) entity_count: usize,

    /// Whether `EntityCreatedEvent` has any subscribers for this entity type.
    pub(in crate::entity) entity_created_event_subscribed: bool,

    /// Lazily initialized entity instance.
    pub(crate) entity: OnceCell<Box<E>>,

    /// A vector of `Box<PropertyValueStoreCore<E, P>>`, type-erased to `Box<dyn PropertyValueStore<E>>`
    items: Vec<Box<dyn PropertyValueStore<E>>>,

    /// Immutable, dense metadata for the property slots in `items`.
    property_registrations: &'static [PropertyRegistration],

    /// Set of properties that currently have `PropertyInitializedEvent` subscribers.
    ///
    /// `PropertyList::emit_initialized_events` reads this in the hot path to skip
    /// per-property initialization-event work unless a handler is registered as a performance
    /// optimization.
    pub(in crate::entity) property_initialized_event_subscriptions: BitSet,

    /// One entry for every property whose `PropertyValueStoreCore` currently has an index.
    /// The property ID supports removal without relying on deduplicable function addresses.
    pub(in crate::entity) index_new_entity_fns: Vec<(usize, IndexNewEntityFn<E>)>,
}

impl<E: Entity> Default for PropertyStoreCore<E> {
    fn default() -> Self {
        PropertyStoreCore::new()
    }
}

impl<E: Entity> PropertyStoreCore<E> {
    /// Creates a new [`PropertyStoreCore`].
    #[must_use]
    pub fn new() -> Self {
        let property_registrations = SCHEMA_REGISTRY
            .entity_registration::<E>()
            .properties
            .as_ref();

        // The frozen schema has been fully published before installers are invoked. No schema lock
        // or OnceLock initialization closure is active here.
        let mut items: Vec<Box<dyn PropertyValueStore<E>>> =
            Vec::with_capacity(property_registrations.len());
        for registration in property_registrations {
            (registration.value_store_installer)(&mut items);
        }
        assert_eq!(items.len(), property_registrations.len());
        let num_items = property_registrations.len();

        Self {
            entity_count: 0,
            entity_created_event_subscribed: false,
            entity: OnceCell::new(),
            items,
            property_registrations,
            property_initialized_event_subscriptions: BitSet::new(num_items),
            index_new_entity_fns: Vec::new(),
        }
    }

    pub(crate) fn new_boxed() -> Box<dyn PropertyStore> {
        Box::new(Self::new())
    }

    #[must_use]
    #[inline]
    pub(crate) fn dependent_property_ids(&self, property_id: usize) -> &[usize] {
        &self.property_registrations[property_id].dependent_property_ids
    }

    /// Fetches an immutable reference to the `PropertyValueStoreCore<E, P>`.
    #[must_use]
    pub fn get<P: Property<E>>(&self) -> &PropertyValueStoreCore<E, P> {
        let index = P::id();
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
        let property_value_store_any: &dyn Any = property_value_store.as_ref();
        let property_value_store: &PropertyValueStoreCore<E, P> = property_value_store_any
            .downcast_ref::<PropertyValueStoreCore<E, P>>()
            .unwrap_or_else(||
                {
                    panic!(
                        "Property type at index {:?} does not match registered property type. Found type_id {:?} while getting type_id {:?}. You must use the `define_property!` macro to create a registered property.",
                        index,
                        property_value_store_any.type_id(),
                        TypeId::of::<PropertyValueStoreCore<E, P>>()
                    )
                }
            );
        property_value_store
    }

    /// Fetches a mutable reference to the `PropertyValueStoreCore<E, P>`.
    #[must_use]
    pub fn get_mut<P: Property<E>>(&mut self) -> &mut PropertyValueStoreCore<E, P> {
        let index = P::id();
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
        let property_value_store_any: &mut dyn Any = property_value_store.as_mut();
        let type_id = Any::type_id(&*property_value_store_any); // Only used for error message if error occurs.
        let property_value_store: &mut PropertyValueStoreCore<E, P> = property_value_store_any
            .downcast_mut::<PropertyValueStoreCore<E, P>>()
            .unwrap_or_else(||
                {
                    panic!(
                        "Property type at index {:?} does not match registered property type. Found type_id {:?} while getting type_id {:?}. You must use the `define_property!` macro to create a registered property.",
                        index,
                        type_id,
                        TypeId::of::<PropertyValueStoreCore<E, P>>()
                    )
                }
            );
        property_value_store
    }

    /// Creates a `PartialPropertyChangeEvent` instance for the `entity_id` and `property_index`. This method is only
    /// called for derived dependents of some property that has changed (one of `P`'s non-derived dependencies).
    #[must_use]
    pub(crate) fn create_partial_property_change(
        &self,
        property_index: usize,
        entity_id: EntityId<E>,
        context: &Context,
    ) -> PartialPropertyChangeEventBox {
        let property_value_store = self.items.get(property_index).unwrap_or_else(|| {
            panic!(
                "Ixa internal error: dependent property index {property_index:?} is not registered"
            )
        });

        property_value_store.create_partial_property_change(entity_id, context)
    }

    /// Returns whether the property with `property_index` needs partial change-event processing.
    #[must_use]
    pub(crate) fn should_create_partial_property_change(
        &self,
        property_index: usize,
        context: &Context,
    ) -> bool {
        let property_value_store = self.items.get(property_index).unwrap_or_else(|| {
            panic!(
                "Ixa internal error: dependent property index {property_index:?} is not registered"
            )
        });

        property_value_store.should_create_partial_change(context)
    }

    /// Returns whether or not the property `P` is indexed.
    ///
    #[cfg(test)]
    #[must_use]
    pub fn is_property_indexed<P: Property<E>>(&self) -> bool {
        self.get::<P>().index.is_some()
    }

    pub(in crate::entity) fn install_property_index<P>(
        &mut self,
        new_index: Option<Box<dyn PropertyIndex<E, P>>>,
    ) where
        P: IndexableProperty<E>,
    {
        let property_id = P::id();
        let was_indexed = self.get::<P>().index.is_some();
        let will_be_indexed = new_index.is_some();

        // Property IDs are stable dispatcher identities; function-pointer addresses are not,
        // because the linker may deduplicate distinct monomorphizations.
        let dispatcher_position = self
            .index_new_entity_fns
            .iter()
            .position(|(id, _)| *id == property_id);

        if was_indexed {
            assert!(
                dispatcher_position.is_some(),
                "Ixa internal error: indexed property is missing its index_new_entity dispatcher",
            );
        } else {
            debug_assert!(
                dispatcher_position.is_none(),
                "Ixa internal error: unindexed property unexpectedly has an index_new_entity dispatcher",
            );
        }

        // Reserve before replacing a valid index so allocation failure cannot leave the index
        // installed without its dispatcher.
        if !was_indexed && will_be_indexed {
            self.index_new_entity_fns.reserve(1);
        }

        // All invariant checks and potentially allocating preparation are complete. Replacing the
        // installed index is the commit point.
        self.get_mut::<P>().index = new_index;

        match (was_indexed, will_be_indexed) {
            (false, true) => {
                self.index_new_entity_fns
                    .push((property_id, index_new_entity::<E, P> as IndexNewEntityFn<E>));
            }
            (true, false) => {
                // The invariant check above proves that an indexed property has a dispatcher.
                self.index_new_entity_fns
                    .swap_remove(dispatcher_position.unwrap());
            }
            _ => {}
        }
    }

    /// Creates a stratified value change counter for tracked property `P` with strata `PL`.
    ///
    /// Returns the counter ID.
    #[must_use]
    pub fn create_value_change_counter<PL, P>(&mut self) -> usize
    where
        PL: PropertyList<E> + Eq + std::hash::Hash,
        P: Property<E> + Eq + std::hash::Hash,
    {
        let property_value_store = self.get_mut::<P>();
        property_value_store.add_value_change_counter(Box::new(StratifiedValueChangeCounter::<
            E,
            PL,
            P,
        >::new()))
    }

    #[must_use]
    pub fn get_index_set_for_query_parts(
        &self,
        property_id: usize,
        query_parts: &[&dyn Any],
    ) -> IndexSetResult<'_, E> {
        self.items[property_id].get_index_set_for_query_parts(query_parts)
    }

    #[must_use]
    pub fn get_index_count_for_query_parts(
        &self,
        property_id: usize,
        query_parts: &[&dyn Any],
    ) -> IndexCountResult {
        self.items[property_id].get_index_count_for_query_parts(query_parts)
    }
}

impl<E: Entity> PropertyStore for PropertyStoreCore<E> {
    fn allocate_entity_id(&mut self) -> (usize, bool) {
        let id = self.entity_count;
        self.entity_count += 1;
        (id, self.entity_created_event_subscribed)
    }

    fn entity_count(&self) -> usize {
        self.entity_count
    }
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]
    use std::any::Any;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use super::*;
    use crate::entity::index::{FullIndex, IndexCountResult, IndexSetResult, ValueCountIndex};
    use crate::entity::PropertyIndexType;
    use crate::prelude::*;
    use crate::{define_derived_property, define_entity, define_property, with, Context};

    define_entity!(Person);

    define_property!(struct Age(u8), Person);
    define_property!(
        enum InfectionStatus {
            Susceptible,
            Infected,
            Recovered,
        },
        Person,
        default_const = InfectionStatus::Susceptible
    );
    define_property!(struct Vaccinated(bool), Person, default_const = Vaccinated(false));
    define_property!(struct PanicDependency(u8), Person, default_const = PanicDependency(0));

    define_derived_property!(
        struct PanickingDerived(u8),
        Person,
        [PanicDependency],
        [],
        |dependency| {
            let dependency: PanicDependency = dependency;
            assert_ne!(dependency, PanicDependency(255), "sentinel property value");
            PanickingDerived(dependency.0)
        }
    );

    #[test]
    fn erased_property_store_downcasts_and_reports_count() {
        let mut property_store: Box<dyn PropertyStore> = PropertyStoreCore::<Person>::new_boxed();

        assert_eq!(property_store.entity_count(), 0);
        assert_eq!(property_store.allocate_entity_id(), (0, false));
        assert_eq!(property_store.entity_count(), 1);
        let property_store_any: &dyn Any = property_store.as_ref();
        assert!(property_store_any
            .downcast_ref::<PropertyStoreCore<Person>>()
            .is_some());

        let property_store_any: &mut dyn Any = property_store.as_mut();
        property_store_any
            .downcast_mut::<PropertyStoreCore<Person>>()
            .unwrap()
            .entity_count = 3;
        assert_eq!(property_store.entity_count(), 3);
    }

    #[test]
    fn install_property_index_maintains_active_dispatchers() {
        let mut context = Context::new();

        {
            let property_store = context.entity_store.get_property_store_mut::<Person>();
            assert_eq!(property_store.index_new_entity_fns.len(), 0);

            property_store.install_property_index::<Age>(Some(Box::new(ValueCountIndex::new())));
            assert_eq!(property_store.index_new_entity_fns.len(), 1);

            property_store.install_property_index::<Age>(Some(Box::new(ValueCountIndex::new())));
            assert_eq!(property_store.index_new_entity_fns.len(), 1);

            property_store.install_property_index::<Age>(Some(Box::new(FullIndex::new())));
            assert_eq!(property_store.index_new_entity_fns.len(), 1);

            property_store.install_property_index::<Age>(None);
            assert_eq!(property_store.index_new_entity_fns.len(), 0);

            property_store.install_property_index::<Age>(Some(Box::new(ValueCountIndex::new())));
            property_store.install_property_index::<Vaccinated>(Some(Box::new(FullIndex::new())));
            property_store.install_property_index::<Age>(None);

            assert_eq!(property_store.index_new_entity_fns.len(), 1);
            assert_eq!(property_store.index_new_entity_fns[0].0, Vaccinated::id());
        }

        context.add_entity(with!(Person, Age(10))).unwrap();
        let property_store = context.entity_store.get_property_store::<Person>();
        assert_eq!(
            property_store.get::<Age>().index_type(),
            PropertyIndexType::Unindexed
        );
        assert_eq!(
            property_store.get_index_count_for_query_parts(
                Vaccinated::id(),
                &[&Vaccinated(false) as &dyn Any],
            ),
            IndexCountResult::Count(1),
        );
    }

    #[test]
    #[should_panic(
        expected = "Ixa internal error: indexed property is missing its index_new_entity dispatcher"
    )]
    fn removing_index_without_dispatcher_panics() {
        let mut property_store = PropertyStoreCore::<Person>::new();
        property_store.install_property_index::<Age>(Some(Box::new(FullIndex::new())));
        property_store.index_new_entity_fns.clear();

        property_store.install_property_index::<Age>(None);
    }

    #[test]
    #[should_panic(
        expected = "Ixa internal error: unindexed property unexpectedly has an index_new_entity dispatcher"
    )]
    fn dispatcher_without_index_panics() {
        let mut property_store = PropertyStoreCore::<Person>::new();
        property_store.install_property_index::<Age>(Some(Box::new(FullIndex::new())));
        property_store.get_mut::<Age>().index = None;

        property_store.install_property_index::<Age>(None);
    }

    #[test]
    fn failed_replacement_keeps_old_index_and_dispatcher() {
        let mut context = Context::new();
        let ordinary = context
            .add_entity(with!(Person, Age(10), PanicDependency(10)))
            .unwrap();
        let sentinel = context
            .add_entity(with!(Person, Age(20), PanicDependency(255)))
            .unwrap();

        let mut old_index = ValueCountIndex::<Person, PanickingDerived>::new();
        old_index.add_entity(&PanickingDerived(10), ordinary);
        old_index.add_entity(&PanickingDerived(255), sentinel);
        context
            .entity_store
            .get_property_store_mut::<Person>()
            .install_property_index::<PanickingDerived>(Some(Box::new(old_index)));

        let (old_type, old_dispatcher_count, old_dispatcher_property_id) = {
            let property_store = context.entity_store.get_property_store::<Person>();
            (
                property_store.get::<PanickingDerived>().index_type(),
                property_store.index_new_entity_fns.len(),
                property_store.index_new_entity_fns[0].0,
            )
        };
        assert_eq!(old_type, PropertyIndexType::ValueCountIndex);
        assert_eq!(
            context.query_entity_count(with!(Person, PanickingDerived(10))),
            1
        );
        assert_eq!(
            context.query_entity_count(with!(Person, PanickingDerived(255))),
            1
        );

        let result = catch_unwind(AssertUnwindSafe(|| {
            context.index_property::<Person, PanickingDerived>();
        }));
        assert!(result.is_err());

        let property_store = context.entity_store.get_property_store::<Person>();
        assert_eq!(
            property_store.get::<PanickingDerived>().index_type(),
            old_type
        );
        assert_eq!(
            property_store.index_new_entity_fns.len(),
            old_dispatcher_count
        );
        assert_eq!(
            property_store.index_new_entity_fns[0].0,
            old_dispatcher_property_id
        );
        assert_eq!(
            context.query_entity_count(with!(Person, PanickingDerived(10))),
            1
        );
        assert_eq!(
            context.query_entity_count(with!(Person, PanickingDerived(255))),
            1
        );

        context
            .add_entity(with!(Person, Age(30), PanicDependency(10)))
            .unwrap();
        assert_eq!(
            context.query_entity_count(with!(Person, PanickingDerived(10))),
            2
        );
    }

    #[test]
    fn test_get_property_store() {
        let mut property_store = PropertyStoreCore::new();

        {
            let ages: &mut PropertyValueStoreCore<_, Age> = property_store.get_mut();
            ages.set(EntityId::<Person>::new(0), Age(12));
            ages.set(EntityId::<Person>::new(1), Age(33));
            ages.set(EntityId::<Person>::new(2), Age(44));

            let infection_statuses: &mut PropertyValueStoreCore<_, InfectionStatus> =
                property_store.get_mut();
            infection_statuses.set(EntityId::<Person>::new(0), InfectionStatus::Susceptible);
            infection_statuses.set(EntityId::<Person>::new(1), InfectionStatus::Susceptible);
            infection_statuses.set(EntityId::<Person>::new(2), InfectionStatus::Infected);

            let vaccine_status: &mut PropertyValueStoreCore<_, Vaccinated> =
                property_store.get_mut();
            vaccine_status.set(EntityId::<Person>::new(0), Vaccinated(true));
            vaccine_status.set(EntityId::<Person>::new(1), Vaccinated(false));
            vaccine_status.set(EntityId::<Person>::new(2), Vaccinated(true));
        }

        // Verify that `get` returns the expected values
        {
            let ages: &PropertyValueStoreCore<_, Age> = property_store.get();
            assert_eq!(ages.get(EntityId::<Person>::new(0)), Age(12));
            assert_eq!(ages.get(EntityId::<Person>::new(1)), Age(33));
            assert_eq!(ages.get(EntityId::<Person>::new(2)), Age(44));

            let infection_statuses: &PropertyValueStoreCore<_, InfectionStatus> =
                property_store.get();
            assert_eq!(
                infection_statuses.get(EntityId::<Person>::new(0)),
                InfectionStatus::Susceptible
            );
            assert_eq!(
                infection_statuses.get(EntityId::<Person>::new(1)),
                InfectionStatus::Susceptible
            );
            assert_eq!(
                infection_statuses.get(EntityId::<Person>::new(2)),
                InfectionStatus::Infected
            );

            let vaccine_status: &PropertyValueStoreCore<_, Vaccinated> = property_store.get();
            assert_eq!(
                vaccine_status.get(EntityId::<Person>::new(0)),
                Vaccinated(true)
            );
            assert_eq!(
                vaccine_status.get(EntityId::<Person>::new(1)),
                Vaccinated(false)
            );
            assert_eq!(
                vaccine_status.get(EntityId::<Person>::new(2)),
                Vaccinated(true)
            );
        }
    }

    #[test]
    fn test_index_query_results_for_property_store() {
        let mut context = Context::new();
        context.index_property::<Person, Age>();

        let existing_value = Age(12);
        let missing_value = Age(99);
        let existing_query_parts = [&existing_value as &dyn Any];
        let missing_query_parts = [&missing_value as &dyn Any];

        let _ = context.add_entity(with!(Person, existing_value)).unwrap();
        let _ = context.add_entity(with!(Person, existing_value)).unwrap();

        let property_store = context.entity_store.get_property_store::<Person>();

        // FullIndex + count
        assert_eq!(
            property_store.get_index_count_for_query_parts(Age::id(), &missing_query_parts,),
            IndexCountResult::Count(0)
        );
        assert_eq!(
            property_store.get_index_count_for_query_parts(Age::id(), &existing_query_parts,),
            IndexCountResult::Count(2)
        );

        // FullIndex + set
        assert!(matches!(
            property_store.get_index_set_for_query_parts(Age::id(), &missing_query_parts,),
            IndexSetResult::Empty
        ));
        assert!(matches!(
            property_store.get_index_set_for_query_parts(
                Age::id(),
                &existing_query_parts,
            ),
            IndexSetResult::Set(set) if set.len() == 2
        ));
    }

    #[test]
    fn test_index_query_results_for_property_store_value_count_index() {
        let mut context = Context::new();
        context.index_property_counts::<Person, Age>();

        let existing_value = Age(12);
        let missing_value = Age(99);
        let existing_query_parts = [&existing_value as &dyn Any];
        let missing_query_parts = [&missing_value as &dyn Any];

        let _ = context.add_entity(with!(Person, existing_value)).unwrap();
        let _ = context.add_entity(with!(Person, existing_value)).unwrap();

        let property_store = context.entity_store.get_property_store::<Person>();

        // ValueCountIndex + count
        assert_eq!(
            property_store.get_index_count_for_query_parts(Age::id(), &missing_query_parts,),
            IndexCountResult::Count(0)
        );
        assert_eq!(
            property_store.get_index_count_for_query_parts(Age::id(), &existing_query_parts,),
            IndexCountResult::Count(2)
        );

        // ValueCountIndex + set (unsupported)
        assert!(matches!(
            property_store.get_index_set_for_query_parts(Age::id(), &missing_query_parts,),
            IndexSetResult::Unsupported
        ));
        assert!(matches!(
            property_store.get_index_set_for_query_parts(Age::id(), &existing_query_parts,),
            IndexSetResult::Unsupported
        ));
    }

    #[test]
    fn test_index_query_results_for_property_store_unindexed() {
        let mut context = Context::new();
        let existing_value = Age(12);
        let missing_value = Age(99);
        let existing_query_parts = [&existing_value as &dyn Any];
        let missing_query_parts = [&missing_value as &dyn Any];

        let _ = context.add_entity(with!(Person, existing_value)).unwrap();
        let _ = context.add_entity(with!(Person, existing_value)).unwrap();

        let property_store = context.entity_store.get_property_store::<Person>();

        // Unindexed + count
        assert_eq!(
            property_store.get_index_count_for_query_parts(Age::id(), &missing_query_parts,),
            IndexCountResult::Unsupported
        );
        assert_eq!(
            property_store.get_index_count_for_query_parts(Age::id(), &existing_query_parts,),
            IndexCountResult::Unsupported
        );

        // Unindexed + set
        assert!(matches!(
            property_store.get_index_set_for_query_parts(Age::id(), &missing_query_parts,),
            IndexSetResult::Unsupported
        ));
        assert!(matches!(
            property_store.get_index_set_for_query_parts(Age::id(), &existing_query_parts,),
            IndexSetResult::Unsupported
        ));
    }
}
