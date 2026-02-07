use std::any::{Any, TypeId};

use crate::entity::entity_set::EntitySetIterator;
use crate::entity::events::{EntityCreatedEvent, PartialPropertyChangeEvent};
use crate::entity::index::{IndexCountResult, IndexSetResult, PropertyIndexType};
use crate::entity::property::Property;
use crate::entity::property_list::PropertyList;
use crate::entity::query::Query;
use crate::entity::{Entity, EntityId, PopulationIterator};
use crate::hashing::IndexSet;
use crate::rand::Rng;
use crate::{warn, Context, ContextRandomExt, IxaError, RngId};

/// A trait extension for [`Context`] that exposes entity-related
/// functionality.
pub trait ContextEntitiesExt {
    fn add_entity<E: Entity, PL: PropertyList<E>>(
        &mut self,
        property_list: PL,
    ) -> Result<EntityId<E>, IxaError>;

    /// Fetches the property value set for the given `entity_id`.
    ///
    /// The easiest way to call this method is by assigning it to a variable with an explicit type:
    /// ```rust, ignore
    /// let vaccine_status: VaccineStatus = context.get_property(entity_id);
    /// ```
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

    /// Enables value-count indexing of property values for the property `P`.
    ///
    /// If the property already has a full index, that index is left unchanged, as it
    /// already supports value-count queries.
    fn index_property_counts<E: Entity, P: Property<E>>(&mut self);

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
        callback: &mut dyn FnMut(&IndexSet<EntityId<E>>),
    );

    /// Gives the count of distinct entity IDs satisfying the query. This is especially
    /// efficient for indexed queries.
    ///
    /// Supplying an empty query `()` is equivalent to calling `get_entity_count::<E>()`.
    fn query_entity_count<E: Entity, Q: Query<E>>(&self, query: Q) -> usize;

    /// Sample a single entity uniformly from the query results. Returns `None` if the
    /// query's result set is empty.
    ///
    /// To sample from the entire population, pass in the empty query `()`.
    fn sample_entity<E, Q, R>(&self, rng_id: R, query: Q) -> Option<EntityId<E>>
    where
        E: Entity,
        Q: Query<E>,
        R: RngId + 'static,
        R::RngType: Rng;

    /// Sample up to `requested` entities uniformly from the query results. If the
    /// query's result set has fewer than `requested` entities, the entire result
    /// set is returned.
    ///
    /// To sample from the entire population, pass in the empty query `()`.
    fn sample_entities<E, Q, R>(&self, rng_id: R, query: Q, n: usize) -> Vec<EntityId<E>>
    where
        E: Entity,
        Q: Query<E>,
        R: RngId + 'static,
        R::RngType: Rng;

    /// Returns a total count of all created entities of type `E`.
    fn get_entity_count<E: Entity>(&self) -> usize;

    /// Returns an iterator over all created entities of type `E`.
    fn get_entity_iterator<E: Entity>(&self) -> PopulationIterator<E>;

    /// Generates an iterator over the results of the query.
    fn query_result_iterator<E: Entity, Q: Query<E>>(&self, query: Q) -> EntitySetIterator<E>;

    /// Determines if the given person matches this query.
    fn match_entity<E: Entity, Q: Query<E>>(&self, entity_id: EntityId<E>, query: Q) -> bool;

    /// Removes all `EntityId`s from the given vector that do not match the given query.
    fn filter_entities<E: Entity, Q: Query<E>>(&self, entities: &mut Vec<EntityId<E>>, query: Q);
}

impl ContextEntitiesExt for Context {
    fn add_entity<E: Entity, PL: PropertyList<E>>(
        &mut self,
        property_list: PL,
    ) -> Result<EntityId<E>, IxaError> {
        // Check that the properties in the list are distinct.
        PL::validate()?;

        // Check that all required properties are present.
        if !PL::contains_required_properties() {
            return Err("initialization list is missing required properties".into());
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
        if P::is_derived() {
            P::compute_derived(self, entity_id)
        } else {
            let property_store = self.get_property_value_store::<E, P>();
            property_store.get(entity_id)
        }
    }

    fn set_property<E: Entity, P: Property<E>>(
        &mut self,
        entity_id: EntityId<E>,
        property_value: P,
    ) {
        debug_assert!(!P::is_derived(), "cannot set a derived property");

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

        // We decided not to do the following check:
        // ```rust
        // let previous_value = { self.get_property_value_store::<E, P>().get(entity_id) };
        // if property_value == previous_value {
        //     return;
        // }
        // ```
        // The reasoning is:
        // - It should be rare that we ever set a property to its present value.
        // - It's not a significant burden on client code to check `property_value == previous_value` on
        //   their own if they need to.
        // - There may be use cases for listening to "writes" that don't actually change values.

        let mut dependents: Vec<Box<dyn PartialPropertyChangeEvent>> = vec![];
        let property_store = self.entity_store.get_property_store::<E>();

        // Create the partial property change for this value.
        dependents.push(property_store.create_partial_property_change(P::id(), entity_id, self));
        // Now create partial property change events for each dependent.
        for dependent_idx in P::dependents() {
            dependents.push(property_store.create_partial_property_change(
                *dependent_idx,
                entity_id,
                self,
            ));
        }

        // Update the value
        let property_value_store = self.get_property_value_store::<E, P>();
        property_value_store.set(entity_id, property_value);

        // After updating the value
        for dependent in dependents.into_iter() {
            dependent.emit_in_context(self)
        }
    }

    fn index_property<E: Entity, P: Property<E>>(&mut self) {
        let property_store = self.entity_store.get_property_store_mut::<E>();
        property_store.set_property_indexed::<P>(PropertyIndexType::FullIndex);
    }

    fn index_property_counts<E: Entity, P: Property<E>>(&mut self) {
        let property_store = self.entity_store.get_property_store_mut::<E>();
        let current_index_type = property_store.get::<P>().index_type();
        if current_index_type != PropertyIndexType::FullIndex {
            property_store.set_property_indexed::<P>(PropertyIndexType::ValueCountIndex);
        }
    }

    #[cfg(test)]
    fn is_property_indexed<E: Entity, P: Property<E>>(&self) -> bool {
        let property_store = self.entity_store.get_property_store::<E>();
        property_store.is_property_indexed::<P>()
    }

    fn with_query_results<E: Entity, Q: Query<E>>(
        &self,
        query: Q,
        callback: &mut dyn FnMut(&IndexSet<EntityId<E>>),
    ) {
        // The fast path for indexed queries.

        // This mirrors the indexed case in `SourceSet<'a, E>::new()` and `Query:: new_query_result_iterator`.
        // The difference is, we access the index set if we find it.
        if let Some(multi_property_id) = query.multi_property_id() {
            let property_store = self.entity_store.get_property_store::<E>();
            match property_store.get_index_set_with_hash_for_property_id(
                self,
                multi_property_id,
                query.multi_property_value_hash(),
            ) {
                IndexSetResult::Set(people_set) => {
                    callback(&people_set);
                    return;
                }
                IndexSetResult::Empty => {
                    let people_set = IndexSet::default();
                    callback(&people_set);
                    return;
                }
                IndexSetResult::Unsupported => {}
            }
            // If the property is not indexed, we fall through.
        }

        // Special case the empty query, which creates a set containing the entire population.
        if query.type_id() == TypeId::of::<()>() {
            warn!("Called Context::with_query_results() with an empty query. Prefer Context::get_entity_iterator::<E>() for working with the entire population.");
            let entity_set = self.get_entity_iterator::<E>().collect::<IndexSet<_>>();
            callback(&entity_set);
            return;
        }

        // The slow path of computing the full query set.
        warn!("Called Context::with_query_results() with an unindexed query. It's almost always better to use Context::query_result_iterator() for unindexed queries.");

        // Fall back to `EntitySetIterator`.
        let people_set = query
            .new_query_result_iterator(self)
            .collect::<IndexSet<_>>();
        callback(&people_set);
    }

    fn query_entity_count<E: Entity, Q: Query<E>>(&self, query: Q) -> usize {
        // The fast path for indexed queries.
        //
        // This mirrors the indexed case in `SourceSet<'a, E>::new()` and `Query:: new_query_result_iterator`.
        if let Some(multi_property_id) = query.multi_property_id() {
            let property_store = self.entity_store.get_property_store::<E>();
            match property_store.get_index_count_with_hash_for_property_id(
                self,
                multi_property_id,
                query.multi_property_value_hash(),
            ) {
                IndexCountResult::Count(count) => return count,
                IndexCountResult::Unsupported => {}
            }
            // If the property is not indexed, we fall through.
        }

        self.query_result_iterator(query).count()
    }
    fn sample_entity<E, Q, R>(&self, rng_id: R, query: Q) -> Option<EntityId<E>>
    where
        E: Entity,
        Q: Query<E>,
        R: RngId + 'static,
        R::RngType: Rng,
    {
        let query_result = self.query_result_iterator(query);
        self.sample(rng_id, move |rng| query_result.sample_entity(rng))
    }

    fn sample_entities<E, Q, R>(&self, rng_id: R, query: Q, n: usize) -> Vec<EntityId<E>>
    where
        E: Entity,
        Q: Query<E>,
        R: RngId + 'static,
        R::RngType: Rng,
    {
        let query_result = self.query_result_iterator(query);
        self.sample(rng_id, move |rng| query_result.sample_entities(rng, n))
    }

    fn get_entity_count<E: Entity>(&self) -> usize {
        self.entity_store.get_entity_count::<E>()
    }

    fn get_entity_iterator<E: Entity>(&self) -> PopulationIterator<E> {
        self.entity_store.get_entity_iterator::<E>()
    }

    fn query_result_iterator<E: Entity, Q: Query<E>>(&self, query: Q) -> EntitySetIterator<E> {
        query.new_query_result_iterator(self)
    }

    fn match_entity<E: Entity, Q: Query<E>>(&self, entity_id: EntityId<E>, query: Q) -> bool {
        query.match_entity(entity_id, self)
    }

    fn filter_entities<E: Entity, Q: Query<E>>(&self, entities: &mut Vec<EntityId<E>>, query: Q) {
        query.filter_entities(entities, self);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Ref, RefCell};
    use std::rc::Rc;

    use super::*;
    use crate::hashing::IndexSet;
    use crate::prelude::PropertyChangeEvent;
    use crate::{define_derived_property, define_entity, define_multi_property, define_property};

    define_entity!(Animal);
    define_property!(struct Legs(u8), Animal, default_const = Legs(4));

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

    define_derived_property!(
        enum AgeGroup {
            Child,
            Adult,
            Senior,
        },
        Person,
        [Age],
        |age| {
            if age.0 <= 18 {
                AgeGroup::Child
            } else if age.0 <= 65 {
                AgeGroup::Adult
            } else {
                AgeGroup::Senior
            }
        }
    );

    define_derived_property!(
        enum RiskLevel {
            Low,
            Medium,
            High,
        },
        Person,
        [AgeGroup, Vaccinated, InfectionStatus],
        |age_group, vaccinated, infection_status| {
            match (age_group, vaccinated, infection_status) {
                (AgeGroup::Senior, Vaccinated(false), InfectionStatus::Susceptible) => {
                    RiskLevel::High
                }
                (_, Vaccinated(false), InfectionStatus::Susceptible) => RiskLevel::Medium,
                _ => RiskLevel::Low,
            }
        }
    );

    // ToDo(RobertJacobsonCDC): Enable this once #691 is resolved, https://github.com/CDCgov/ixa/issues/691.
    // define_global_property!(GlobalDummy, u8);
    // define_derived_property!(
    //     struct MyDerivedProperty(u8),
    //     Person,
    //     [Age],
    //     [GlobalDummy],
    //     |age, global_dummy| {
    //         MyDerivedProperty(age.0 + global_dummy)
    //     }
    // );

    // Derived properties in a diamond dependency relationship
    define_property!(struct IsRunner(bool), Person, default_const = IsRunner(false));
    define_property!(struct IsSwimmer(bool), Person, default_const = IsSwimmer(false));
    define_derived_property!(
        struct AdultRunner(bool),
        Person,
        [AgeGroup, IsRunner],
        | age_group, is_runner | {
            AdultRunner(
                age_group == AgeGroup::Adult
                && is_runner.0
            )
        }
    );
    define_derived_property!(
        struct AdultSwimmer(bool),
        Person,
        [AgeGroup, IsSwimmer],
        | age_group, is_swimmer | {
            AdultSwimmer(
                age_group == AgeGroup::Adult
                && is_swimmer.0
            )
        }
    );
    define_derived_property!(
        struct AdultAthlete(bool),
        Person,
        [AdultSwimmer, AdultRunner],
        | adult_swimmer, adult_runner | {
            AdultAthlete(
                adult_swimmer.0 || adult_runner.0
            )
        }
    );

    #[test]
    fn add_and_count_entities() {
        let mut context = Context::new();

        let _person1 = context
            .add_entity((Age(12), InfectionStatus::Susceptible, Vaccinated(true)))
            .unwrap();
        assert_eq!(context.get_entity_count::<Person>(), 1);

        let _person2 = context.add_entity((Age(34), Vaccinated(true))).unwrap();
        assert_eq!(context.get_entity_count::<Person>(), 2);

        // Age is the only required property
        let _person3 = context.add_entity((Age(120),)).unwrap();
        assert_eq!(context.get_entity_count::<Person>(), 3);
    }

    // Helper for index tests
    #[derive(Copy, Clone)]
    enum IndexMode {
        Unindexed,
        FullIndex,
        ValueCountIndex,
    }

    // Returns `(context, existing_value, missing_value)`
    fn setup_context_for_index_tests(index_mode: IndexMode) -> (Context, Age, Age) {
        let mut context = Context::new();
        match index_mode {
            IndexMode::Unindexed => {}
            IndexMode::FullIndex => context.index_property::<Person, Age>(),
            IndexMode::ValueCountIndex => context.index_property_counts::<Person, Age>(),
        }

        let existing_value = Age(12);
        let missing_value = Age(99);

        let _ = context.add_entity((existing_value,)).unwrap();
        let _ = context.add_entity((existing_value,)).unwrap();

        (context, existing_value, missing_value)
    }

    #[test]
    fn query_results_respect_index_modes() {
        let modes = [
            IndexMode::Unindexed,
            IndexMode::FullIndex,
            IndexMode::ValueCountIndex,
        ];

        for mode in modes {
            let (context, existing_value, missing_value) = setup_context_for_index_tests(mode);

            let mut existing_len = 0;
            context.with_query_results((existing_value,), &mut |people_set| {
                existing_len = people_set.len();
            });
            assert_eq!(existing_len, 2);

            let mut missing_len = 0;
            context.with_query_results((missing_value,), &mut |people_set| {
                missing_len = people_set.len();
            });
            assert_eq!(missing_len, 0);

            let existing_count = context.query_result_iterator((existing_value,)).count();
            assert_eq!(existing_count, 2);

            let missing_count = context.query_result_iterator((missing_value,)).count();
            assert_eq!(missing_count, 0);

            assert_eq!(context.query_entity_count((existing_value,)), 2);
            assert_eq!(context.query_entity_count((missing_value,)), 0);
        }
    }

    #[test]
    fn add_an_entity_without_required_properties() {
        let mut context = Context::new();
        let result = context.add_entity((InfectionStatus::Susceptible, Vaccinated(true)));

        assert!(matches!(
            result,
            Err(crate::IxaError::IxaError(ref msg)) if msg == "initialization list is missing required properties"
        ));
    }

    #[test]
    fn new_entities_have_default_values() {
        let mut context = Context::new();

        // Create a person with required Age property
        let person = context.add_entity((Age(25),)).unwrap();

        // Retrieve and check their values
        let age: Age = context.get_property(person);
        assert_eq!(age, Age(25));
        let infection_status: InfectionStatus = context.get_property(person);
        assert_eq!(infection_status, InfectionStatus::Susceptible);
        let vaccinated: Vaccinated = context.get_property(person);
        assert_eq!(vaccinated, Vaccinated(false));

        // Change them
        context.set_property(person, Age(26));
        context.set_property(person, InfectionStatus::Infected);
        context.set_property(person, Vaccinated(true));

        // Retrieve and check their values
        let age: Age = context.get_property(person);
        assert_eq!(age, Age(26));
        let infection_status: InfectionStatus = context.get_property(person);
        assert_eq!(infection_status, InfectionStatus::Infected);
        let vaccinated: Vaccinated = context.get_property(person);
        assert_eq!(vaccinated, Vaccinated(true));
    }

    #[test]
    fn get_and_set_property_explicit() {
        let mut context = Context::new();

        // Create a person with explicit property values
        let person = context
            .add_entity((Age(25), InfectionStatus::Recovered, Vaccinated(true)))
            .unwrap();

        // Retrieve and check their values
        let age: Age = context.get_property(person);
        assert_eq!(age, Age(25));
        let infection_status: InfectionStatus = context.get_property(person);
        assert_eq!(infection_status, InfectionStatus::Recovered);
        let vaccinated: Vaccinated = context.get_property(person);
        assert_eq!(vaccinated, Vaccinated(true));

        // Change them
        context.set_property(person, Age(26));
        context.set_property(person, InfectionStatus::Infected);
        context.set_property(person, Vaccinated(false));

        // Retrieve and check their values
        let age: Age = context.get_property(person);
        assert_eq!(age, Age(26));
        let infection_status: InfectionStatus = context.get_property(person);
        assert_eq!(infection_status, InfectionStatus::Infected);
        let vaccinated: Vaccinated = context.get_property(person);
        assert_eq!(vaccinated, Vaccinated(false));
    }

    #[test]
    fn count_entities() {
        let mut context = Context::new();

        assert_eq!(context.get_entity_count::<Animal>(), 0);
        assert_eq!(context.get_entity_count::<Person>(), 0);

        // Create entities of different kinds
        for _ in 0..7 {
            let _: PersonId = context.add_entity((Age(25),)).unwrap();
        }
        for _ in 0..5 {
            let _: AnimalId = context.add_entity((Legs(2),)).unwrap();
        }

        assert_eq!(context.get_entity_count::<Animal>(), 5);
        assert_eq!(context.get_entity_count::<Person>(), 7);

        let _: PersonId = context.add_entity((Age(30),)).unwrap();
        let _: AnimalId = context.add_entity((Legs(8),)).unwrap();

        assert_eq!(context.get_entity_count::<Animal>(), 6);
        assert_eq!(context.get_entity_count::<Person>(), 8);
    }

    #[test]
    fn get_derived_property_multiple_deps() {
        let mut context = Context::new();

        let expected_high_id: PersonId = context
            .add_entity((Age(77), Vaccinated(false), InfectionStatus::Susceptible))
            .unwrap();
        let expected_med_id: PersonId = context
            .add_entity((Age(30), Vaccinated(false), InfectionStatus::Susceptible))
            .unwrap();
        let expected_low_id: PersonId = context
            .add_entity((Age(3), Vaccinated(true), InfectionStatus::Recovered))
            .unwrap();

        let actual_high: RiskLevel = context.get_property(expected_high_id);
        assert_eq!(actual_high, RiskLevel::High);
        let actual_med: RiskLevel = context.get_property(expected_med_id);
        assert_eq!(actual_med, RiskLevel::Medium);
        let actual_low: RiskLevel = context.get_property(expected_low_id);
        assert_eq!(actual_low, RiskLevel::Low);
    }

    #[test]
    fn listen_to_derived_property_change_event() {
        let mut context = Context::new();

        let expected_high_id = PersonId::new(0);

        // Listen for derived property change events and record how many times it fires
        // For `RiskLevel`
        let risk_flag = Rc::new(RefCell::new(0));
        let risk_flag_clone = risk_flag.clone();
        context.subscribe_to_event(
            move |_context, event: PropertyChangeEvent<Person, RiskLevel>| {
                assert_eq!(event.entity_id, expected_high_id);
                assert_eq!(event.previous, RiskLevel::High);
                assert_eq!(event.current, RiskLevel::Medium);
                *risk_flag_clone.borrow_mut() += 1;
            },
        );
        // For `AgeGroup`
        let age_group_flag = Rc::new(RefCell::new(0));
        let age_group_flag_clone = age_group_flag.clone();
        context.subscribe_to_event(
            move |_context, event: PropertyChangeEvent<Person, AgeGroup>| {
                assert_eq!(event.entity_id, expected_high_id);
                assert_eq!(event.previous, AgeGroup::Senior);
                assert_eq!(event.current, AgeGroup::Adult);
                *age_group_flag_clone.borrow_mut() += 1;
            },
        );

        // Should not emit change events
        let expected_high_id: PersonId = context
            .add_entity((Age(77), Vaccinated(false), InfectionStatus::Susceptible))
            .unwrap();

        // Should emit change events
        context.set_property(expected_high_id, Age(20));

        // Execute queued event handlers
        context.execute();
        // Should have exactly one event recorded
        assert_eq!(*risk_flag.borrow(), 1);
        assert_eq!(*age_group_flag.borrow(), 1);
    }

    /*
    ToDo(RobertJacobsonCDC): Enable this once #691 is resolved, https://github.com/CDCgov/ixa/issues/691.

    #[test]
    fn get_derived_property_with_globals() {
        let mut context = Context::new();

        context.set_global_property_value(GlobalDummy, 18).unwrap();
        let child = context.add_entity((Age(17),)).unwrap();
        let adult = context.add_entity((Age(19),)).unwrap();

        let child_computed: MyDerivedProperty = context.get_property(child);
        assert_eq!(child_computed, MyDerivedProperty(17+18));

        let adult_computed: MyDerivedProperty = context.get_property(adult);
        assert_eq!(adult_computed, MyDerivedProperty(19+18));
    }
    */

    #[test]
    fn observe_diamond_property_change() {
        let mut context = Context::new();
        let person = context.add_entity((Age(17), IsSwimmer(true))).unwrap();

        let is_adult_athlete: AdultAthlete = context.get_property(person);
        assert!(!is_adult_athlete.0);

        let flag = Rc::new(RefCell::new(0));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, event: PropertyChangeEvent<Person, AdultAthlete>| {
                assert_eq!(event.entity_id, person);
                assert_eq!(event.previous, AdultAthlete(false));
                assert_eq!(event.current, AdultAthlete(true));
                *flag_clone.borrow_mut() += 1;
            },
        );

        context.set_property(person, Age(20));
        // Make sure the derived property is what we expect.
        let is_adult_athlete: AdultAthlete = context.get_property(person);
        assert!(is_adult_athlete.0);

        // Execute queued event handlers
        context.execute();
        // Should have exactly one event recorded
        assert_eq!(*flag.borrow(), 1);
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
        let mut address: *const IndexSet<EntityId<Person>> = std::ptr::null();
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
        let bucket: Ref<IndexSet<EntityId<Person>>> = property_value_store
            .get_index_set_with_hash(
                (InfectionStatus::Susceptible, Vaccinated(true)).multi_property_value_hash(),
            )
            .unwrap();

        let address2 = &*bucket as *const _;
        assert_eq!(address2, address);
    }

    #[test]
    fn set_property_correctly_maintains_index() {
        let mut context = Context::new();
        context.index_property::<Person, InfectionStatus>();
        context.index_property::<Person, AgeGroup>();

        let person1 = context.add_entity((Age(22),)).unwrap();
        let person2 = context.add_entity((Age(22),)).unwrap();
        for _ in 0..4 {
            let _: PersonId = context.add_entity((Age(22),)).unwrap();
        }

        // Check non-derived property index is correctly maintained
        assert_eq!(
            context.query_entity_count((InfectionStatus::Susceptible,)),
            6
        );
        assert_eq!(context.query_entity_count((InfectionStatus::Infected,)), 0);
        assert_eq!(context.query_entity_count((InfectionStatus::Recovered,)), 0);

        context.set_property(person1, InfectionStatus::Infected);

        assert_eq!(
            context.query_entity_count((InfectionStatus::Susceptible,)),
            5
        );
        assert_eq!(context.query_entity_count((InfectionStatus::Infected,)), 1);
        assert_eq!(context.query_entity_count((InfectionStatus::Recovered,)), 0);

        context.set_property(person1, InfectionStatus::Recovered);

        assert_eq!(
            context.query_entity_count((InfectionStatus::Susceptible,)),
            5
        );
        assert_eq!(context.query_entity_count((InfectionStatus::Infected,)), 0);
        assert_eq!(context.query_entity_count((InfectionStatus::Recovered,)), 1);

        // Check derived property index is correctly maintained.
        assert_eq!(context.query_entity_count((AgeGroup::Child,)), 0);
        assert_eq!(context.query_entity_count((AgeGroup::Adult,)), 6);
        assert_eq!(context.query_entity_count((AgeGroup::Senior,)), 0);

        context.set_property(person2, Age(12));

        assert_eq!(context.query_entity_count((AgeGroup::Child,)), 1);
        assert_eq!(context.query_entity_count((AgeGroup::Adult,)), 5);
        assert_eq!(context.query_entity_count((AgeGroup::Senior,)), 0);

        context.set_property(person1, Age(75));

        assert_eq!(context.query_entity_count((AgeGroup::Child,)), 1);
        assert_eq!(context.query_entity_count((AgeGroup::Adult,)), 4);
        assert_eq!(context.query_entity_count((AgeGroup::Senior,)), 1);

        context.set_property(person2, Age(77));

        assert_eq!(context.query_entity_count((AgeGroup::Child,)), 0);
        assert_eq!(context.query_entity_count((AgeGroup::Adult,)), 4);
        assert_eq!(context.query_entity_count((AgeGroup::Senior,)), 2);
    }

    #[test]
    fn query_unindexed_default_properties() {
        let mut context = Context::new();

        // Half will have the default value.
        for idx in 0..10 {
            if idx % 2 == 0 {
                context.add_entity((Age(22),)).unwrap();
            } else {
                context
                    .add_entity((Age(22), InfectionStatus::Recovered))
                    .unwrap();
            }
        }
        // The tail also has the default value
        for _ in 0..10 {
            let _: PersonId = context.add_entity((Age(22),)).unwrap();
        }

        assert_eq!(context.query_entity_count((InfectionStatus::Recovered,)), 5);
        assert_eq!(
            context.query_entity_count((InfectionStatus::Susceptible,)),
            15
        );
    }

    #[test]
    fn query_unindexed_derived_properties() {
        let mut context = Context::new();

        for _ in 0..10 {
            let _: PersonId = context.add_entity((Age(22),)).unwrap();
        }

        assert_eq!(context.query_entity_count((AdultAthlete(false),)), 10);
    }
}
