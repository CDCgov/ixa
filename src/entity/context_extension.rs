use std::any::{Any, TypeId};

use crate::entity::events::{
    EntityCreatedEvent, PartialPropertyChangeEvent, PartialPropertyChangeEventCore,
};
use crate::entity::property::{Property, PropertyInitializationKind};
use crate::entity::property_list::PropertyList;
use crate::entity::query::{Query, QueryResultIterator};
use crate::entity::{Entity, EntityId, EntityIterator};
use crate::rand::Rng;
use crate::{warn, Context, ContextRandomExt, HashSet, RngId};

/// A trait extension for [`Context`] that exposes entity-related
/// functionality.
pub trait ContextEntitiesExt {
    fn add_entity<E: Entity, PL: PropertyList<E>>(
        &mut self,
        property_list: PL,
    ) -> Result<EntityId<E>, String>;

    fn get_property<E: Entity, P: Property<E>>(&self, entity_id: EntityId<E>) -> P;

    /// Sets the value of the given property. This method unconditionally emits a `PropertyChangeEvent`.
    fn set_property<E: Entity, P: Property<E>>(
        &mut self,
        entity_id: EntityId<E>,
        property_value: P,
    );

    /// Enables indexing of property values for the property `P`.
    ///
    /// This method is called with the turbo-fish syntax:
    ///     `context.index_property::<Person, Age>()`
    /// The actual computation of the index is done lazily as needed upon execution of queries,
    /// not when this method is called.
    fn index_property<E: Entity, P: Property<E>>(&mut self);

    /// Checks if a property `P` is indexed.
    ///
    /// This method is called with the turbo-fish syntax:
    ///     `context.index_property::<Person, Age>()`
    ///
    /// This method can return `true` even if `context.index_property::<P>()` has never been called. For example,
    /// if a multi-property is indexed, all equivalent multi-properties are automatically also indexed, as they
    /// share a single index.
    #[cfg(test)]
    fn is_property_indexed<E: Entity, P: Property<E>>(&self) -> bool;

    /// This method gives client code direct immutable access to the fully realized set of
    /// entity IDs. This is especially efficient for indexed queries, as this method reduces
    /// to a simple lookup of a hash bucket. Otherwise, the set is allocated and computed.
    fn with_query_results<E: Entity, Q: Query<E>>(
        &self,
        query: Q,
        callback: &mut dyn FnMut(&HashSet<EntityId<E>>),
    );

    fn query_entity_count<E: Entity, Q: Query<E>>(&self, query: Q) -> usize;

    /// Sample a single entity uniformly from the query results. Returns `None` if the
    /// query's result set is empty.
    ///
    /// To sample from the entire population, pass in the empty query `()`.
    fn sample_entity<R, E, Q>(&self, rng_id: R, query: Q) -> Option<EntityId<E>>
    where
        R: RngId + 'static,
        R::RngType: Rng,
        E: Entity,
        Q: Query<E>;

    /// Sample up to `requested` entities uniformly from the query results. If the
    /// query's result set has fewer than `requested` entities, the entire result
    /// set is returned.
    ///
    /// To sample from the entire population, pass in the empty query `()`.
    fn sample_entities<R, E, Q>(&self, rng_id: R, query: Q, n: usize) -> Vec<EntityId<E>>
    where
        R: RngId + 'static,
        R::RngType: Rng,
        E: Entity,
        Q: Query<E>;

    /// Returns a total count of all created entities of type `E`.
    fn get_entity_count<E: Entity>(&self) -> usize;

    /// Returns an iterator over all created entities of type `E`.
    fn get_entity_iterator<E: Entity>(&self) -> EntityIterator<E>;

    /// Generates an iterator over the results of the query.
    fn query_result_iterator<E: Entity, Q: Query<E>>(&self, query: Q) -> QueryResultIterator<E>;
}

impl ContextEntitiesExt for Context {
    fn add_entity<E: Entity, PL: PropertyList<E>>(
        &mut self,
        property_list: PL,
    ) -> Result<EntityId<E>, String> {
        // Check that the properties in the list are distinct.
        if let Err(msg) = PL::validate() {
            return Err(format!("invalid property list: {}", msg));
        }

        // Check that all required properties are present.
        if !PL::contains_required_properties() {
            return Err("initialization list is missing required properties".to_string());
        }

        // Now that we know we will succeed, we create the entity.
        let new_entity_id = self.entity_store.new_entity_id::<E>();

        // Assign the properties in the list to the new entity.
        // This does not generate a property change event.
        property_list
            .set_values_for_entity(new_entity_id, self.entity_store.get_property_store::<E>());

        // Emit an `EntityCreatedEvent<Entity>`.
        self.emit_event(EntityCreatedEvent::<E>::new(new_entity_id));

        Ok(new_entity_id)
    }

    fn get_property<E: Entity, P: Property<E>>(&self, entity_id: EntityId<E>) -> P {
        // ToDo(RobertJacobsonCDC): An alternative to the following is to always assume
        //       that `None` means "not set" for "explicit" properties, that is, assume
        //       that `get` is infallible for properties with a default constant. We
        //       take a more conservative approach here and check for internal errors.
        match P::initialization_kind() {
            PropertyInitializationKind::Explicit => {
                let property_store = self.get_property_value_store::<E, P>();
                // A user error can cause this unwrap to fail.
                property_store.get(entity_id).expect("attempted to get a property value with \"explicit\" initialization that was not set")
            }

            PropertyInitializationKind::Derived => P::compute_derived(self, entity_id),

            PropertyInitializationKind::Constant => {
                let property_store = self.get_property_value_store::<E, P>();
                // If this unwrap fails, it is an internal ixa error, not a user error.
                property_store.get(entity_id).expect(
                    "getting a property value with \"constant\" initialization should never fail",
                )
            }
        }
    }

    fn set_property<E: Entity, P: Property<E>>(
        &mut self,
        entity_id: EntityId<E>,
        property_value: P,
    ) {
        debug_assert!(
            P::initialization_kind() != PropertyInitializationKind::Derived,
            "cannot set a derived property"
        );

        // The algorithm is as follows
        // 1. Get the previous value of the property.
        //    1.1 If it's the same as `property_value`, exit.
        //    1.2 Otherwise, create a `PartialPropertyChangeEvent<E, P>`.
        // 2. Remove the `entity_id` from the index bucket corresponding to its old value.
        // 3. For each dependent of the property, do the analog of steps 1 & 2:
        //    3.1 Compute the previous value of the dependent property `Q`, creating a
        //        `PartialPropertyChangeEvent<E, Q>` instance if necessary.
        //    3.2 Remove the `entity_id` from the index bucket corresponding to the old value of `Q`.
        // 4. Set the new value of the (main) property in the property store.
        // 5. Update the property index: Insert the `entity_id` into the index bucket corresponding to the new value.
        // 6. Emit the property change event: convert the `PartialPropertyChangeEvent<E, P>` into a
        //    `event: PropertyChangeEvent<E, P>` and call `Context::emit_event(event)`.
        // 7. For each dependent of the property, do the analog of steps 4-6:
        //    7.1 Compute the new value of the dependent property
        //    7.2 Add `entity_id` to the index bucket corresponding to the new value.
        //    7.3 convert the `PartialPropertyChangeEvent<E, Q>` into a
        //        `event: PropertyChangeEvent<E, Q>` and call `Context::emit_event(event)`.

        // We need two passes over the dependents: one pass to compute all the old values and
        // another to compute all the new values. We group the steps for each dependent (and, it
        // turns out, for the main property `P` as well) into two parts:
        //  1. Before setting the main property `P`, factored out into
        //     `self.property_store.create_partial_property_change`
        //  2. After setting the main property `P`, factored out into
        //     `PartialPropertyChangeEvent::emit_in_context`

        let previous_value = { self.get_property_value_store::<E, P>().get(entity_id) };

        if Some(property_value) == previous_value {
            return;
        }

        // If the following unwrap fails, it must be because the value was never set and does not have a default value.
        let previous_value = previous_value.unwrap();
        let mut dependents: Vec<Box<dyn PartialPropertyChangeEvent>> = vec![Box::new(
            PartialPropertyChangeEventCore::new(entity_id, previous_value),
        )];

        for dependent_idx in P::dependents() {
            let property_store = self.entity_store.get_property_store::<E>();
            dependents.push(property_store.create_partial_property_change(
                *dependent_idx,
                entity_id,
                self,
            ));
        }

        let property_value_store = self.get_property_value_store::<E, P>();
        property_value_store.set(entity_id, property_value);

        for dependent in dependents.into_iter() {
            dependent.emit_in_context(self)
        }
    }

    fn index_property<E: Entity, P: Property<E>>(&mut self) {
        let property_store = self.entity_store.get_property_store_mut::<E>();
        property_store.set_property_indexed::<P>(true);
    }
    #[cfg(test)]
    fn is_property_indexed<E: Entity, P: Property<E>>(&self) -> bool {
        let property_store = self.entity_store.get_property_store::<E>();
        property_store.is_property_indexed::<P>()
    }
    fn with_query_results<E: Entity, Q: Query<E>>(
        &self,
        query: Q,
        callback: &mut dyn FnMut(&HashSet<EntityId<E>>),
    ) {
        // The fast path for indexed queries.

        // This mirrors the indexed case in `SourceSet<'a, E>::new()` and `Query:: new_query_result_iterator`.
        // The difference is, we access the index set if we find it.
        if let Some(multi_property_id) = query.multi_property_id() {
            let property_store = self.entity_store.get_property_store::<E>();
            // The `index_unindexed_people` method returns `false` if the property is not indexed.
            if property_store.index_unindexed_entities_for_property_id(self, multi_property_id) {
                // Fetch the right hash bucket from the index and return it.
                let property_value_store = property_store.get_with_id(multi_property_id);
                if let Some(people_set) =
                    property_value_store.get_index_set_with_hash(query.multi_property_value_hash())
                {
                    callback(&people_set);
                } else {
                    // Since we already checked that this multi-property is indexed, it must be that
                    // there are no entities having this property value.
                    let people_set = HashSet::default();
                    callback(&people_set);
                }
                return;
            }
            // If the property is not indexed, we fall through.
        }

        // Special case the empty query, which creates a set containing the entire population.
        if query.type_id() == TypeId::of::<()>() {
            warn!("Called Context::with_query_results() with an empty query. Prefer Context::get_entity_iterator::<E>() for working with the entire population.");
            let entity_set = self.get_entity_iterator::<E>().collect::<HashSet<_>>();
            callback(&entity_set);
            return;
        }

        // The slow path of computing the full query set.
        warn!("Called Context::with_query_results() with an unindexed query. It's almost always better to use Context::query_result_iterator() for unindexed queries.");

        // Fall back to `QueryResultIterator`.
        let people_set = query
            .new_query_result_iterator(self)
            .collect::<HashSet<_>>();
        callback(&people_set);
    }

    fn query_entity_count<E: Entity, Q: Query<E>>(&self, query: Q) -> usize {
        // The fast path for indexed queries.
        //
        // This mirrors the indexed case in `SourceSet<'a, E>::new()` and `Query:: new_query_result_iterator`.
        if let Some(multi_property_id) = query.multi_property_id() {
            let property_store = self.entity_store.get_property_store::<E>();
            // The `index_unindexed_people` method returns `false` if the property is not indexed.
            if property_store.index_unindexed_entities_for_property_id(self, multi_property_id) {
                // Fetch the right hash bucket from the index and return it.
                let property_value_store = property_store.get_with_id(multi_property_id);
                if let Some(people_set) =
                    property_value_store.get_index_set_with_hash(query.multi_property_value_hash())
                {
                    return people_set.len();
                } else {
                    // Since we already checked that this multi-property is indexed, it must be that
                    // there are no entities having this property value.
                    return 0;
                }
            }
            // If the property is not indexed, we fall through.
        }

        self.query_result_iterator(query).count()
    }
    fn sample_entity<R, E, Q>(&self, rng_id: R, query: Q) -> Option<EntityId<E>>
    where
        R: RngId + 'static,
        R::RngType: Rng,
        E: Entity,
        Q: Query<E>,
    {
        let query_result = self.query_result_iterator(query);
        self.sample(rng_id, move |rng| query_result.sample_entity(rng))
    }

    fn sample_entities<R, E, Q>(&self, rng_id: R, query: Q, n: usize) -> Vec<EntityId<E>>
    where
        R: RngId + 'static,
        R::RngType: Rng,
        E: Entity,
        Q: Query<E>,
    {
        let query_result = self.query_result_iterator(query);
        self.sample(rng_id, move |rng| query_result.sample_entities(rng, n))
    }

    fn get_entity_count<E: Entity>(&self) -> usize {
        self.entity_store.get_entity_count::<E>()
    }

    fn get_entity_iterator<E: Entity>(&self) -> EntityIterator<E> {
        self.entity_store.get_entity_iterator::<E>()
    }
    fn query_result_iterator<E: Entity, Q: Query<E>>(&self, query: Q) -> QueryResultIterator<E> {
        query.new_query_result_iterator(self)
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use std::cell::Ref;

    use super::*;
    use crate::{define_entity, define_multi_property, define_property};

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

    define_property!(
        struct Vaccinated(bool),
        Person,
        default_const = Vaccinated(false)
    );

    #[test]
    fn add_an_entity() {
        let mut context = Context::new();
        let person = context.add_entity((Age(12), InfectionStatus::Susceptible, Vaccinated(true)));
        println!("{:?}", person);

        let person = context.add_entity((Age(34), Vaccinated(true)));
        println!("{:?}", person);

        // Age is the only required property
        let person = context.add_entity((Age(120),));
        println!("{:?}", person);
    }

    #[test]
    #[should_panic(expected = "initialization list is missing required properties")]
    fn add_an_entity_without_required_properties() {
        let mut context = Context::new();
        let person1 = context
            .add_entity((InfectionStatus::Susceptible, Vaccinated(true)))
            .unwrap();
        println!("{:?}", person1);
    }

    #[test]
    fn get_and_set_property_explicit() {
        let mut context = Context::new();

        // Create a person with required Age property
        let person = context.add_entity((Age(25),)).unwrap();

        // Retrieve it
        let age: Age = context.get_property(person);
        assert_eq!(age, Age(25));

        // Change it
        context.set_property(person, Age(26));
        let age: Age = context.get_property(person);
        assert_eq!(age, Age(26));
    }

    #[test]
    fn get_property_with_constant_default() {
        let mut context = Context::new();

        // `Vaccinated` has a default value (false)
        let person = context.add_entity((Age(40),)).unwrap();

        // Even though we didn't set Vaccinated, it should exist with its default
        let vaccinated: Vaccinated = context.get_property(person);
        assert_eq!(vaccinated, Vaccinated(false));

        // Now override
        context.set_property(person, Vaccinated(true));
        let vaccinated: Vaccinated = context.get_property(person);
        assert_eq!(vaccinated, Vaccinated(true));
    }

    #[test]
    fn get_property_with_enum_default() {
        let mut context = Context::new();

        // InfectionStatus has a default of Susceptible
        let person = context.add_entity((Age(22),)).unwrap();
        let status: InfectionStatus = context.get_property(person);
        assert_eq!(status, InfectionStatus::Susceptible);
    }

    // Tests related to queries and indexing

    define_multi_property!((InfectionStatus, Vaccinated), Person);
    define_multi_property!((Vaccinated, InfectionStatus), Person);

    #[test]
    fn with_query_results_finds_multi_index() {
        use crate::rand::seq::IndexedRandom;
        let mut rng = crate::rand::rng();
        let mut context = Context::new();

        for _ in 0..10_000usize {
            let infection_status = *[
                InfectionStatus::Susceptible,
                InfectionStatus::Infected,
                InfectionStatus::Recovered,
            ]
            .choose(&mut rng)
            .unwrap();
            let vaccination_status: bool = rng.random_bool(0.5);
            let age: u8 = rng.random_range(0..100);
            context
                .add_entity((Age(age), infection_status, Vaccinated(vaccination_status)))
                .unwrap();
        }
        context.index_property::<Person, InfectionStatusVaccinated>();
        // Force an index build by running a query.
        let _ = context.query_result_iterator((InfectionStatus::Susceptible, Vaccinated(true)));

        // Capture the address of the has set given by `with_query_result`
        let mut address: *const HashSet<EntityId<Person>> = std::ptr::null();
        context.with_query_results(
            (InfectionStatus::Susceptible, Vaccinated(true)),
            &mut |result_set| {
                address = result_set as *const _;
            },
        );

        // Check that the order doesn't matter.
        assert_eq!(
            InfectionStatusVaccinated::index_id(),
            VaccinatedInfectionStatus::index_id()
        );
        assert_eq!(
            InfectionStatusVaccinated::index_id(),
            (InfectionStatus::Susceptible, Vaccinated(true))
                .multi_property_id()
                .unwrap()
        );

        // Check if it matches the expected bucket.
        let index_id = InfectionStatusVaccinated::index_id();

        let property_store = context.entity_store.get_property_store::<Person>();
        let property_value_store = property_store.get_with_id(index_id);
        let bucket: Ref<HashSet<EntityId<Person>>> = property_value_store
            .get_index_set_with_hash(
                (InfectionStatus::Susceptible, Vaccinated(true)).multi_property_value_hash(),
            )
            .unwrap();

        let address2 = &*bucket as *const _;
        assert_eq!(address2, address);
    }
}
