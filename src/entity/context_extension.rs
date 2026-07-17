use std::hash::Hash;

use smallvec::SmallVec;

use crate::entity::entity_set::{EntitySet, EntitySetIterator, SourceSet};
use crate::entity::events::{EntityCreatedEvent, PartialPropertyChangeEventBox};
use crate::entity::index::{IndexCountResult, IndexSetResult, PropertyIndexType};
use crate::entity::property::{IndexableProperty, Property};
use crate::entity::property_list::{PropertyInitializationList, PropertyList};
use crate::entity::query::Query;
use crate::entity::value_change_counter::StratifiedValueChangeCounter;
use crate::entity::{Entity, EntityId, PopulationIterator};
use crate::rand::Rng;
use crate::random::sample_multiple_from_known_length;
use crate::{warn, Context, ContextRandomExt, ExecutionPhase, IxaError, RngId};

#[cfg(feature = "profiling")]
fn query_profile_label<E: Entity, Q: Query<E>>(query: &Q) -> &'static str {
    <Q as crate::entity::QueryInternal<E>>::query_profile_label(query)
}

fn handle_periodic_value_change_count_event<E, PL, P, F>(
    context: &mut Context,
    period: f64,
    counter_id: usize,
    handler: F,
) where
    E: Entity,
    PL: PropertyList<E> + Eq + Hash,
    P: IndexableProperty<E>,
    F: Fn(&mut Context, &mut StratifiedValueChangeCounter<E, PL, P>) + 'static,
{
    let mut counter = {
        let property_value_store = context.get_property_value_store_mut::<E, P>();
        let slot = property_value_store
            .value_change_counters
            .get_mut(counter_id)
            .unwrap_or_else(|| {
                panic!(
                    "No value change counter found for property {} with counter_id {}",
                    P::name(),
                    counter_id
                )
            });
        std::mem::replace(
            slot.get_mut(),
            Box::new(StratifiedValueChangeCounter::<E, PL, P>::new()),
        )
    };

    {
        let counter = counter
            .as_any_mut()
            .downcast_mut::<StratifiedValueChangeCounter<E, PL, P>>()
            .unwrap_or_else(|| {
                panic!(
                    "Value change counter for property {} and counter_id {} had unexpected type",
                    P::name(),
                    counter_id
                )
            });

        handler(context, counter);
        counter.clear();
    }

    {
        let property_value_store = context.get_property_value_store_mut::<E, P>();
        let slot = property_value_store
            .value_change_counters
            .get_mut(counter_id)
            .unwrap_or_else(|| {
                panic!(
                    "No value change counter found for property {} with counter_id {}",
                    P::name(),
                    counter_id
                )
            });

        // Swap back the cleared counter to retain its allocated capacity.
        let _ = std::mem::replace(slot.get_mut(), counter);
    }

    if context.remaining_plan_count() == 0 {
        return;
    }

    let next_time = context.get_current_time() + period;
    context.add_plan_with_phase(
        next_time,
        move |context| {
            handle_periodic_value_change_count_event::<E, PL, P, F>(
                context, period, counter_id, handler,
            );
        },
        ExecutionPhase::Last,
    );
}

/// A trait extension for [`Context`] that exposes entity-related
/// functionality.
pub trait ContextEntitiesExt {
    fn add_entity<E: Entity, PL: PropertyInitializationList<E>>(
        &mut self,
        property_list: PL,
    ) -> Result<EntityId<E>, IxaError>;

    /// Fetches the property value set for the given `entity_id`.
    ///
    /// The easiest way to call this method is by assigning it to a variable with an explicit type:
    /// ```rust, ignore
    /// let vaccine_status: VaccineStatus = context.get_property(entity_id);
    /// ```
    #[must_use]
    fn get_property<E: Entity, P: Property<E>>(&self, entity_id: EntityId<E>) -> P;

    /// Sets the value of the given property. This method unconditionally emits a `PropertyChangeEvent`.
    fn set_property<E: Entity, P: Property<E>>(
        &mut self,
        entity_id: EntityId<E>,
        property_value: P,
    );

    /// Enables full indexing of property values for the property `P`.
    ///
    /// This method is called with the turbo-fish syntax:
    ///     `context.index_property::<Person, Age>()`
    ///
    /// This method both enables the index and catches it up to the current population.
    fn index_property<E: Entity, P: IndexableProperty<E>>(&mut self);

    /// Enables value-count indexing of property values for the property `P`.
    ///
    /// If the property already has a full index, that index is left unchanged, as it
    /// already supports value-count queries.
    fn index_property_counts<E: Entity, P: IndexableProperty<E>>(&mut self);

    /// Tracks periodic value change counts for a newly created counter.
    ///
    /// Also panics if `period` is not finite and strictly positive.
    ///
    /// Recording starts at `ExecutionPhase::First` at simulation start time. The
    /// first report runs at simulation start time in `ExecutionPhase::Last`, then at
    /// each subsequent `start_time + k * period`. After the handler returns, the
    /// matched counter is cleared.
    ///
    /// ```rust,ignore
    /// context.track_periodic_value_change_counts::<Person, (InfectionStatus,), Age>(
    ///     30.0,
    ///     |_context, counter| {
    ///         let _ = counter;
    ///     },
    /// );
    /// ```
    fn track_periodic_value_change_counts<E, PL, P, F>(&mut self, period: f64, handler: F)
    where
        E: Entity,
        PL: PropertyList<E> + Eq + Hash,
        P: Property<E> + Eq + Hash,
        F: Fn(&mut Context, &mut StratifiedValueChangeCounter<E, PL, P>) + 'static;

    /// Checks if a property `P` is indexed.
    ///
    /// This method is called with the turbo-fish syntax:
    ///     `context.index_property::<Person, Age>()`
    ///
    /// This method only checks the concrete property storage for `P`, not any equivalent
    /// multi-properties.
    #[cfg(test)]
    #[must_use]
    fn is_property_indexed<E: Entity, P: Property<E>>(&self) -> bool;

    /// This method gives client code direct access to the query result as an `EntitySet`.
    /// This is especially efficient for indexed queries, as this method can reduce to wrapping
    /// a single indexed source.
    fn with_query_results<'a, E: Entity, Q: Query<E>>(
        &'a self,
        query: Q,
        callback: &mut dyn FnMut(EntitySet<'a, E>),
    );

    /// Gives the count of distinct entity IDs satisfying the query. This is especially
    /// efficient for indexed queries.
    ///
    /// Supplying a naked entity, e.g. `Person`, is equivalent to calling `get_entity_count::<Person>()`.
    #[must_use]
    fn query_entity_count<E: Entity, Q: Query<E>>(&self, query: Q) -> usize;

    /// Sample a single entity uniformly from the query results. Returns `None` if the
    /// query's result set is empty.
    ///
    /// To sample from the entire population, pass the entity type directly, for example `Person`.
    #[must_use]
    fn sample_entity<E, Q, R>(&self, rng_id: R, query: Q) -> Option<EntityId<E>>
    where
        E: Entity,
        Q: Query<E>,
        R: RngId + 'static,
        R::RngType: Rng;

    /// Count query results and sample a single entity uniformly from them.
    ///
    /// Returns `(count, sample)`, where `sample` is `None` iff `count == 0`.
    /// To sample from the entire population, pass the entity type directly, for example `Person`.
    #[must_use]
    fn count_and_sample_entity<E, Q, R>(&self, rng_id: R, query: Q) -> (usize, Option<EntityId<E>>)
    where
        E: Entity,
        Q: Query<E>,
        R: RngId + 'static,
        R::RngType: Rng;

    /// Sample up to `requested` entities uniformly from the query results. If the
    /// query's result set has fewer than `requested` entities, the entire result
    /// set is returned.
    ///
    /// To sample from the entire population, pass the entity type directly, for example `Person`.
    #[must_use]
    fn sample_entities<E, Q, R>(&self, rng_id: R, query: Q, n: usize) -> Vec<EntityId<E>>
    where
        E: Entity,
        Q: Query<E>,
        R: RngId + 'static,
        R::RngType: Rng;

    /// Returns a total count of all created entities of type `E`.
    #[must_use]
    fn get_entity_count<E: Entity>(&self) -> usize;

    /// Returns an iterator over all created entities of type `E`.
    #[must_use]
    fn get_entity_iterator<E: Entity>(&self) -> PopulationIterator<E>;

    /// Generates an `EntitySet` representing the query results.
    #[must_use]
    fn query<E: Entity, Q: Query<E>>(&self, query: Q) -> EntitySet<E>;

    /// Generates an iterator over the results of the query.
    #[must_use]
    fn query_result_iterator<E: Entity, Q: Query<E>>(&self, query: Q) -> EntitySetIterator<E>;

    /// Determines if the given person matches this query.
    #[must_use]
    fn match_entity<E: Entity, Q: Query<E>>(&self, entity_id: EntityId<E>, query: Q) -> bool;

    /// Removes all `EntityId`s from the given vector that do not match the given query.
    fn filter_entities<E: Entity, Q: Query<E>>(&self, entities: &mut Vec<EntityId<E>>, query: Q);
}

impl ContextEntitiesExt for Context {
    fn add_entity<E: Entity, PL: PropertyInitializationList<E>>(
        &mut self,
        property_list: PL,
    ) -> Result<EntityId<E>, IxaError> {
        // Check that the properties in the list are distinct.
        PL::validate()?;

        // Check that all required properties are present.
        if !PL::contains_required_properties() {
            return Err(IxaError::MissingRequiredInitializationProperties);
        }

        // Now that we know we will succeed, we create the entity.
        let new_entity_id = self.entity_store.new_entity_id::<E>();

        // Assign the properties in the list to the new entity.
        // This does not generate a property change event.
        property_list.set_values_for_new_entity(
            new_entity_id,
            self.entity_store.get_property_store_mut::<E>(),
        );

        // Keep all enabled indexes caught up as entities are created.
        let context_ptr: *const Context = self;
        let property_store = self.entity_store.get_property_store_mut::<E>();
        // SAFETY: We create a shared `&Context` for read-only property access while mutably
        // borrowing the property store to update index internals.
        unsafe {
            property_store.index_unindexed_entities_for_all_properties(&*context_ptr);
        }

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

        // The algorithm is as follows:
        // 1. Snapshot previous values for the main property and any dependents that need change
        //    processing by creating `PartialPropertyChangeEvent` instances.
        // 2. Set the new value of the main property in the property store.
        // 3. Emit each partial event; during emission each event computes the current value,
        //    updates its index (remove old/add new), and emits a `PropertyChangeEvent`.

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

        // `SmallVec` inline capacity balances stack footprint against heap allocations: a larger
        // inline size avoids spills for more dependents, while a smaller one keeps every
        // set_property call lighter when most properties have few dependents. A value of 5 is
        // chosen somewhat arbitrarily.
        let mut dependents: SmallVec<[PartialPropertyChangeEventBox; 5]> = SmallVec::new();

        // Immutable: Collect the previous value to create partial property change events
        {
            let property_store = self.entity_store.get_property_store::<E>();

            // Create the partial property change for this value.
            if property_store.should_create_partial_property_change(P::id(), self) {
                dependents.push(property_store.create_partial_property_change(
                    P::id(),
                    entity_id,
                    self,
                ));
            }
            // Now create partial property change events for each dependent.
            for dependent_idx in P::dependents() {
                if property_store.should_create_partial_property_change(*dependent_idx, self) {
                    dependents.push(property_store.create_partial_property_change(
                        *dependent_idx,
                        entity_id,
                        self,
                    ));
                }
            }
        }

        // Update the value
        let property_value_store = self.get_property_value_store_mut::<E, P>();
        property_value_store.set(entity_id, property_value);

        // Mutable: After updating the value, we update its dependents, removing old values and
        // storing the new values in their respective indexes, and emit the property change event.
        for mut dependent in dependents {
            dependent.emit_in_context(self)
        }
    }

    fn index_property<E: Entity, P: IndexableProperty<E>>(&mut self) {
        let property_id = P::id();
        let context_ptr: *const Context = self;
        let property_store = self.entity_store.get_property_store_mut::<E>();
        property_store.set_property_indexed::<P>(PropertyIndexType::FullIndex);
        // SAFETY: This only creates a shared reference to `Context` while mutably borrowing
        // the property store to update index internals.
        unsafe {
            property_store.index_unindexed_entities_for_property_id(&*context_ptr, property_id);
        }
    }

    fn index_property_counts<E: Entity, P: IndexableProperty<E>>(&mut self) {
        let property_store = self.entity_store.get_property_store_mut::<E>();
        let current_index_type = property_store.get::<P>().index_type();
        if current_index_type != PropertyIndexType::FullIndex {
            property_store.set_property_indexed::<P>(PropertyIndexType::ValueCountIndex);
        }
    }

    fn track_periodic_value_change_counts<E, PL, P, F>(&mut self, period: f64, handler: F)
    where
        E: Entity,
        PL: PropertyList<E> + Eq + Hash,
        P: IndexableProperty<E>,
        F: Fn(&mut Context, &mut StratifiedValueChangeCounter<E, PL, P>) + 'static,
    {
        assert!(
            period > 0.0 && !period.is_nan() && !period.is_infinite(),
            "Period must be greater than 0"
        );
        let start_time = self.get_start_time().unwrap_or(0.0);
        self.add_plan_with_phase(
            start_time,
            move |context| {
                // We create the counter at simulation start so initialization-time
                // property writes are never recorded.
                let counter_id = context
                    .entity_store
                    .get_property_store_mut::<E>()
                    .create_value_change_counter::<PL, P>();

                // We defer the first handler plan until now because it needs
                // `counter_id`, and it must run in `ExecutionPhase::Last`.
                context.add_plan_with_phase(
                    context.get_current_time(),
                    move |context| {
                        handle_periodic_value_change_count_event::<E, PL, P, F>(
                            context, period, counter_id, handler,
                        );
                    },
                    ExecutionPhase::Last,
                );
            },
            ExecutionPhase::First,
        );
    }

    #[cfg(test)]
    fn is_property_indexed<E: Entity, P: Property<E>>(&self) -> bool {
        let property_store = self.entity_store.get_property_store::<E>();
        property_store.is_property_indexed::<P>()
    }

    fn with_query_results<'a, E: Entity, Q: Query<E>>(
        &'a self,
        query: Q,
        callback: &mut dyn FnMut(EntitySet<'a, E>),
    ) {
        #[cfg(feature = "profiling")]
        let profile = self.query_profile_handle(query_profile_label::<E, Q>(&query));

        // The fast path for indexed queries.
        //
        // This mirrors the indexed case in `SourceSet<'a, E>::new()` and
        // `QueryInternal::new_query_result`. The difference is, we access the index set if we find it.
        if let Some(multi_property_id) = query.multi_property_id() {
            let property_store = self.entity_store.get_property_store::<E>();
            let query_parts = query.query_parts();
            let lookup_result = property_store
                .get_index_set_for_query_parts(multi_property_id, query_parts.as_ref());
            match lookup_result {
                IndexSetResult::Set(people_set) => {
                    let result = EntitySet::from_source(SourceSet::IndexSet(people_set));
                    #[cfg(feature = "profiling")]
                    let result = result.with_query_profile(profile);
                    callback(result);
                    return;
                }
                IndexSetResult::Empty => {
                    let result = EntitySet::empty();
                    #[cfg(feature = "profiling")]
                    let result = result.with_query_profile(profile);
                    callback(result);
                    return;
                }
                IndexSetResult::Unsupported => {}
            }
            // If the property is not indexed, we fall through.
        }

        // Special case a whole-population query.
        if query.is_empty_query() {
            warn!("Called Context::with_query_results() with an empty query. Prefer Context::get_entity_iterator::<E>() for working with the entire population.");
            let result =
                EntitySet::from_source(SourceSet::PopulationRange(0..self.get_entity_count::<E>()));
            #[cfg(feature = "profiling")]
            let result = result.with_query_profile(profile);
            callback(result);
            return;
        }

        // The slow path of computing the full query set.
        warn!("Called Context::with_query_results() with an unindexed query. It's almost always better to use Context::query_result_iterator() for unindexed queries.");

        let result = query.new_query_result(self);
        #[cfg(feature = "profiling")]
        let result = result.with_query_profile(profile);
        callback(result);
    }

    fn query_entity_count<E: Entity, Q: Query<E>>(&self, query: Q) -> usize {
        #[cfg(feature = "profiling")]
        let _query_profile_scope = self
            .query_profile_handle(query_profile_label::<E, Q>(&query))
            .scope();

        // The fast path for indexed queries.
        //
        // This mirrors the indexed case in `SourceSet<'a, E>::new()` and `QueryInternal::new_query_result`.
        if let Some(multi_property_id) = query.multi_property_id() {
            let property_store = self.entity_store.get_property_store::<E>();
            let query_parts = query.query_parts();
            let lookup_result = property_store
                .get_index_count_for_query_parts(multi_property_id, query_parts.as_ref());
            match lookup_result {
                IndexCountResult::Count(count) => return count,
                IndexCountResult::Unsupported => {}
            }
            // If the property is not indexed, we fall through.
        }

        query.new_query_result_iterator(self).count()
    }

    fn sample_entity<E, Q, R>(&self, rng_id: R, query: Q) -> Option<EntityId<E>>
    where
        E: Entity,
        Q: Query<E>,
        R: RngId + 'static,
        R::RngType: Rng,
    {
        #[cfg(feature = "profiling")]
        let _query_profile_scope = self
            .query_profile_handle(query_profile_label::<E, Q>(&query))
            .scope();

        if query.is_empty_query() {
            let population = self.get_entity_count::<E>();
            return self.sample(rng_id, move |rng| {
                if population == 0 {
                    warn!("Requested a sample entity from an empty population");
                    return None;
                }
                let index = if population <= u32::MAX as usize {
                    rng.random_range(0..population as u32) as usize
                } else {
                    rng.random_range(0..population)
                };
                Some(EntityId::new(index))
            });
        }

        let query_result = query.new_query_result(self);
        self.sample(rng_id, move |rng| query_result.sample_entity(rng))
    }

    fn count_and_sample_entity<E, Q, R>(&self, rng_id: R, query: Q) -> (usize, Option<EntityId<E>>)
    where
        E: Entity,
        Q: Query<E>,
        R: RngId + 'static,
        R::RngType: Rng,
    {
        #[cfg(feature = "profiling")]
        let _query_profile_scope = self
            .query_profile_handle(query_profile_label::<E, Q>(&query))
            .scope();

        if query.is_empty_query() {
            let population = self.get_entity_count::<E>();
            return self.sample(rng_id, move |rng| {
                if population == 0 {
                    return (0, None);
                }
                let index = if population <= u32::MAX as usize {
                    rng.random_range(0..population as u32) as usize
                } else {
                    rng.random_range(0..population)
                };
                (population, Some(EntityId::new(index)))
            });
        }

        let query_result = query.new_query_result(self);
        self.sample(rng_id, move |rng| query_result.count_and_sample_entity(rng))
    }

    fn sample_entities<E, Q, R>(&self, rng_id: R, query: Q, n: usize) -> Vec<EntityId<E>>
    where
        E: Entity,
        Q: Query<E>,
        R: RngId + 'static,
        R::RngType: Rng,
    {
        #[cfg(feature = "profiling")]
        let _query_profile_scope = self
            .query_profile_handle(query_profile_label::<E, Q>(&query))
            .scope();

        if query.is_empty_query() {
            let population = self.get_entity_count::<E>();
            return self.sample(rng_id, move |rng| {
                if population == 0 {
                    warn!("Requested a sample of entities from an empty population");
                    return vec![];
                }
                if n >= population {
                    return PopulationIterator::<E>::new(population).collect();
                }
                sample_multiple_from_known_length(rng, PopulationIterator::<E>::new(population), n)
            });
        }

        let query_result = query.new_query_result(self);
        self.sample(rng_id, move |rng| query_result.sample_entities(rng, n))
    }

    fn get_entity_count<E: Entity>(&self) -> usize {
        self.entity_store.get_entity_count::<E>()
    }

    fn get_entity_iterator<E: Entity>(&self) -> PopulationIterator<E> {
        self.entity_store.get_entity_iterator::<E>()
    }

    fn query<E: Entity, Q: Query<E>>(&self, query: Q) -> EntitySet<E> {
        #[cfg(feature = "profiling")]
        let profile = self.query_profile_handle(query_profile_label::<E, Q>(&query));
        let result = query.new_query_result(self);
        #[cfg(feature = "profiling")]
        let result = result.with_query_profile(profile);
        result
    }

    fn query_result_iterator<E: Entity, Q: Query<E>>(&self, query: Q) -> EntitySetIterator<E> {
        #[cfg(feature = "profiling")]
        let profile = self.query_profile_handle(query_profile_label::<E, Q>(&query));
        let result = query.new_query_result_iterator(self);
        #[cfg(feature = "profiling")]
        let result = result.with_query_profile(profile);
        result
    }

    fn match_entity<E: Entity, Q: Query<E>>(&self, entity_id: EntityId<E>, query: Q) -> bool {
        #[cfg(feature = "profiling")]
        let _query_profile_scope = self
            .query_profile_handle(query_profile_label::<E, Q>(&query))
            .scope();
        query.match_entity(entity_id, self)
    }

    fn filter_entities<E: Entity, Q: Query<E>>(&self, entities: &mut Vec<EntityId<E>>, query: Q) {
        #[cfg(feature = "profiling")]
        let _query_profile_scope = self
            .query_profile_handle(query_profile_label::<E, Q>(&query))
            .scope();
        query.filter_entities(entities, self);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    #[cfg(feature = "profiling")]
    use std::time::Duration;

    use super::*;
    use crate::entity::query::QueryInternal;
    use crate::hashing::IndexSet;
    use crate::prelude::PropertyChangeEvent;
    use crate::{
        define_derived_property, define_entity, define_multi_property, define_property, define_rng,
        impl_property, with,
    };

    define_entity!(Animal);
    define_property!(struct Legs(u8), Animal, default_const = Legs(4));
    define_rng!(EntityContextTestRng);

    define_entity!(Person);

    define_property!(struct Age(u8), Person);

    #[cfg(feature = "profiling")]
    define_entity!(ProfilingPerson);
    #[cfg(feature = "profiling")]
    define_property!(struct ProfilingAge(u8), ProfilingPerson);
    #[cfg(feature = "profiling")]
    define_property!(struct ProfilingCounty(u8), ProfilingPerson);
    #[cfg(feature = "profiling")]
    define_entity!(ProfilingBoundaryPerson);
    #[cfg(feature = "profiling")]
    define_property!(struct ProfilingBoundaryAge(u8), ProfilingBoundaryPerson);
    #[cfg(feature = "profiling")]
    define_entity!(ProfilingCallbackPerson);
    #[cfg(feature = "profiling")]
    define_property!(struct ProfilingCallbackAge(u8), ProfilingCallbackPerson);
    #[cfg(feature = "profiling")]
    define_entity!(ProfilingIdlePerson);
    #[cfg(feature = "profiling")]
    define_property!(struct ProfilingIdleAge(u8), ProfilingIdlePerson);
    #[cfg(feature = "profiling")]
    define_entity!(ProfilingUnusedIteratorPerson);
    #[cfg(feature = "profiling")]
    define_property!(struct ProfilingUnusedIteratorAge(u8), ProfilingUnusedIteratorPerson);
    #[cfg(feature = "profiling")]
    define_entity!(ProfilingContainsPerson);
    #[cfg(feature = "profiling")]
    define_property!(struct ProfilingContainsAge(u8), ProfilingContainsPerson);
    #[cfg(feature = "profiling")]
    define_entity!(ProfilingIndexedCountPerson);
    #[cfg(feature = "profiling")]
    define_property!(struct ProfilingIndexedCountAge(u8), ProfilingIndexedCountPerson);
    #[cfg(feature = "profiling")]
    define_entity!(ProfilingUnindexedCountPerson);
    #[cfg(feature = "profiling")]
    define_property!(
        struct ProfilingUnindexedCountAge(u8),
        ProfilingUnindexedCountPerson
    );
    #[cfg(feature = "profiling")]
    define_entity!(ProfilingIndexedIteratorPerson);
    #[cfg(feature = "profiling")]
    define_property!(
        struct ProfilingIndexedIteratorAge(u8),
        ProfilingIndexedIteratorPerson
    );
    #[cfg(feature = "profiling")]
    define_entity!(ProfilingUnindexedIteratorPerson);
    #[cfg(feature = "profiling")]
    define_property!(
        struct ProfilingUnindexedIteratorAge(u8),
        ProfilingUnindexedIteratorPerson
    );
    #[cfg(feature = "profiling")]
    define_entity!(ProfilingIndexedWithResultsPerson);
    #[cfg(feature = "profiling")]
    define_property!(
        struct ProfilingIndexedWithResultsAge(u8),
        ProfilingIndexedWithResultsPerson
    );
    #[cfg(feature = "profiling")]
    define_entity!(ProfilingIndexedMatchPerson);
    #[cfg(feature = "profiling")]
    define_property!(
        struct ProfilingIndexedMatchAge(u8),
        ProfilingIndexedMatchPerson
    );
    #[cfg(feature = "profiling")]
    define_entity!(ProfilingIndexedFilterPerson);
    #[cfg(feature = "profiling")]
    define_property!(
        struct ProfilingIndexedFilterAge(u8),
        ProfilingIndexedFilterPerson
    );
    #[cfg(feature = "profiling")]
    define_property!(
        struct ProfilingIndexedFilterCounty(u8),
        ProfilingIndexedFilterPerson
    );
    #[cfg(feature = "profiling")]
    define_multi_property!(
        ProfilingIndexedFilterPerson,
        (ProfilingIndexedFilterAge, ProfilingIndexedFilterCounty)
    );
    #[cfg(feature = "profiling")]
    define_entity!(ProfilingSingleFilterPerson);
    #[cfg(feature = "profiling")]
    define_property!(
        struct ProfilingSingleFilterAge(u8),
        ProfilingSingleFilterPerson
    );
    #[cfg(feature = "profiling")]
    define_entity!(ProfilingComposedPerson);
    #[cfg(feature = "profiling")]
    define_property!(
        struct ProfilingComposedAge(u8),
        ProfilingComposedPerson
    );
    #[cfg(feature = "profiling")]
    define_property!(
        struct ProfilingComposedCounty(u8),
        ProfilingComposedPerson
    );

    #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
    struct CounterValue(u8);
    impl_property!(CounterValue, Person, default_const = CounterValue(0));

    #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
    struct CounterStratum(bool);
    impl_property!(
        CounterStratum,
        Person,
        default_const = CounterStratum(false)
    );

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
            .add_entity(with!(
                Person,
                Age(12),
                InfectionStatus::Susceptible,
                Vaccinated(true)
            ))
            .unwrap();
        assert_eq!(context.get_entity_count::<Person>(), 1);

        let _person2 = context
            .add_entity(with!(Person, Age(34), Vaccinated(true)))
            .unwrap();
        assert_eq!(context.get_entity_count::<Person>(), 2);

        // Age is the only required property
        let _person3 = context.add_entity(with!(Person, Age(120))).unwrap();
        assert_eq!(context.get_entity_count::<Person>(), 3);
    }

    #[test]
    fn add_entity_with_zst() {
        let mut context = Context::new();
        let animal = context.add_entity(Animal).unwrap();
        assert_eq!(context.get_entity_count::<Animal>(), 1);
        assert_eq!(context.get_property::<Animal, Legs>(animal), Legs(4));
    }

    // Helper for index tests
    #[derive(Copy, Clone, Debug)]
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

        let _ = context.add_entity(with!(Person, existing_value)).unwrap();
        let _ = context.add_entity(with!(Person, existing_value)).unwrap();

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
            context.with_query_results(with!(Person, existing_value), &mut |people_set| {
                existing_len = people_set.into_iter().count();
            });
            assert_eq!(existing_len, 2, "Wrong length for {mode:?}");

            let mut missing_len = 0;
            context.with_query_results(with!(Person, missing_value), &mut |people_set| {
                missing_len = people_set.into_iter().count();
            });
            assert_eq!(missing_len, 0);

            let existing_count = context
                .query_result_iterator(with!(Person, existing_value))
                .count();
            assert_eq!(existing_count, 2);

            let missing_count = context
                .query_result_iterator(with!(Person, missing_value))
                .count();
            assert_eq!(missing_count, 0);

            assert_eq!(context.query_entity_count(with!(Person, existing_value)), 2);
            assert_eq!(context.query_entity_count(with!(Person, missing_value)), 0);
        }
    }

    #[test]
    fn add_an_entity_without_required_properties() {
        let mut context = Context::new();
        let result = context.add_entity(with!(
            Person,
            InfectionStatus::Susceptible,
            Vaccinated(true)
        ));

        assert!(matches!(
            result,
            Err(crate::IxaError::MissingRequiredInitializationProperties)
        ));
    }

    #[test]
    fn new_entities_have_default_values() {
        let mut context = Context::new();

        // Create a person with required Age property
        let person = context.add_entity(with!(Person, Age(25))).unwrap();

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
            .add_entity(with!(
                Person,
                Age(25),
                InfectionStatus::Recovered,
                Vaccinated(true)
            ))
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
            let _: PersonId = context.add_entity(with!(Person, Age(25))).unwrap();
        }
        for _ in 0..5 {
            let _: AnimalId = context.add_entity(with!(Animal, Legs(2))).unwrap();
        }

        assert_eq!(context.get_entity_count::<Animal>(), 5);
        assert_eq!(context.get_entity_count::<Person>(), 7);

        let _: PersonId = context.add_entity(with!(Person, Age(30))).unwrap();
        let _: AnimalId = context.add_entity(with!(Animal, Legs(8))).unwrap();

        assert_eq!(context.get_entity_count::<Animal>(), 6);
        assert_eq!(context.get_entity_count::<Person>(), 8);
    }

    #[test]
    fn count_and_sample_entity_empty_query_fast_path() {
        let mut context = Context::new();
        context.init_random(42);
        for age in [10u8, 20, 30] {
            let _: PersonId = context.add_entity(with!(Person, Age(age))).unwrap();
        }

        let (count, sampled) =
            context.count_and_sample_entity::<Person, _, _>(EntityContextTestRng, Person);
        assert_eq!(count, 3);
        assert!(sampled.is_some());
    }

    #[test]
    fn count_and_sample_entity_unindexed_derived_query() {
        let mut context = Context::new();
        context.init_random(43);
        for age in [10u8, 20, 30, 80] {
            let _: PersonId = context.add_entity(with!(Person, Age(age))).unwrap();
        }

        let query = with!(Person, AgeGroup::Adult);
        let expected_count = context.query_entity_count(query);
        let (count, sampled) = context.count_and_sample_entity(EntityContextTestRng, query);
        assert_eq!(count, expected_count);
        assert_eq!(sampled.is_some(), count > 0);
        if let Some(entity_id) = sampled {
            assert!(context.match_entity(entity_id, query));
        }
    }

    #[test]
    fn get_derived_property_multiple_deps() {
        let mut context = Context::new();

        let expected_high_id: PersonId = context
            .add_entity(with!(
                Person,
                Age(77),
                Vaccinated(false),
                InfectionStatus::Susceptible
            ))
            .unwrap();
        let expected_med_id: PersonId = context
            .add_entity(with!(
                Person,
                Age(30),
                Vaccinated(false),
                InfectionStatus::Susceptible
            ))
            .unwrap();
        let expected_low_id: PersonId = context
            .add_entity(with!(
                Person,
                Age(3),
                Vaccinated(true),
                InfectionStatus::Recovered
            ))
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
            .add_entity(with!(
                Person,
                Age(77),
                Vaccinated(false),
                InfectionStatus::Susceptible
            ))
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
        let child = context.add_entity(with!(Person, Age(17))).unwrap();
        let adult = context.add_entity(with!(Person, Age(19))).unwrap();

        let child_computed: MyDerivedProperty = context.get_property(child);
        assert_eq!(child_computed, MyDerivedProperty(17+18));

        let adult_computed: MyDerivedProperty = context.get_property(adult);
        assert_eq!(adult_computed, MyDerivedProperty(19+18));
    }
    */

    #[test]
    fn observe_diamond_property_change() {
        let mut context = Context::new();
        let person = context
            .add_entity(with!(Person, Age(17), IsSwimmer(true)))
            .unwrap();

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

    define_multi_property!(Person, (InfectionStatus, Vaccinated));
    define_multi_property!(Person, (Vaccinated, InfectionStatus));

    #[test]
    fn with_query_results_finds_multi_index() {
        use crate::rand::rngs::SmallRng;
        use crate::rand::seq::IndexedRandom;
        use crate::rand::SeedableRng;

        let mut rng = SmallRng::seed_from_u64(42);
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
                .add_entity(with!(
                    Person,
                    Age(age),
                    infection_status,
                    Vaccinated(vaccination_status)
                ))
                .unwrap();
        }
        context.index_property::<Person, InfectionStatusVaccinated>();
        // Force an index build by running a query.
        let _ = context.query_result_iterator(with!(
            Person,
            InfectionStatus::Susceptible,
            Vaccinated(true)
        ));

        // Capture the set given by `with_query_results`.
        let mut result_entities: IndexSet<EntityId<Person>> = IndexSet::default();
        context.with_query_results(
            with!(Person, InfectionStatus::Susceptible, Vaccinated(true)),
            &mut |result_set| {
                result_entities = result_set.into_iter().collect::<IndexSet<_>>();
            },
        );

        // Check that equivalent multi-properties keep distinct storage and type IDs while
        // sharing query routing identity through the registry.
        assert_ne!(
            InfectionStatusVaccinated::id(),
            VaccinatedInfectionStatus::id()
        );
        assert_ne!(
            InfectionStatusVaccinated::type_id(),
            VaccinatedInfectionStatus::type_id()
        );
        assert_eq!(
            InfectionStatusVaccinated::id(),
            (InfectionStatus::Susceptible, Vaccinated(true))
                .multi_property_id()
                .unwrap()
        );

        // Check if it matches the expected bucket.
        let property_id = InfectionStatusVaccinated::id();

        let property_store = context.entity_store.get_property_store::<Person>();
        let query = (InfectionStatus::Susceptible, Vaccinated(true));
        let query_parts = query.query_parts();
        let bucket =
            match property_store.get_index_set_for_query_parts(property_id, query_parts.as_ref()) {
                IndexSetResult::Set(bucket) => bucket,
                other => panic!("expected indexed query bucket, found {other:?}"),
            };

        let expected_entities = bucket.iter().copied().collect::<IndexSet<_>>();
        assert_eq!(expected_entities, result_entities);
    }

    #[test]
    fn query_returns_entity_set_and_query_result_iterator_remains_compatible() {
        let mut context = Context::new();
        let p1 = context
            .add_entity(with!(
                Person,
                Age(21),
                InfectionStatus::Susceptible,
                Vaccinated(true)
            ))
            .unwrap();
        let _p2 = context
            .add_entity(with!(
                Person,
                Age(22),
                InfectionStatus::Susceptible,
                Vaccinated(false)
            ))
            .unwrap();
        let p3 = context
            .add_entity(with!(
                Person,
                Age(23),
                InfectionStatus::Infected,
                Vaccinated(true)
            ))
            .unwrap();

        let query = with!(Person, Vaccinated(true));

        let from_set = context
            .query::<Person, _>(query)
            .into_iter()
            .collect::<IndexSet<_>>();
        let from_iterator = context
            .query_result_iterator(query)
            .collect::<IndexSet<_>>();

        assert_eq!(from_set, from_iterator);
        assert!(from_set.contains(&p1));
        assert!(from_set.contains(&p3));
        assert_eq!(from_set.len(), 2);
    }

    #[test]
    fn set_property_correctly_maintains_index() {
        let mut context = Context::new();
        context.index_property::<Person, InfectionStatus>();
        context.index_property::<Person, AgeGroup>();

        let person1 = context.add_entity(with!(Person, Age(22))).unwrap();
        let person2 = context.add_entity(with!(Person, Age(22))).unwrap();
        for _ in 0..4 {
            let _: PersonId = context.add_entity(with!(Person, Age(22))).unwrap();
        }

        // Check non-derived property index is correctly maintained
        assert_eq!(
            context.query_entity_count(with!(Person, InfectionStatus::Susceptible)),
            6
        );
        assert_eq!(
            context.query_entity_count(with!(Person, InfectionStatus::Infected)),
            0
        );
        assert_eq!(
            context.query_entity_count(with!(Person, InfectionStatus::Recovered)),
            0
        );

        context.set_property(person1, InfectionStatus::Infected);

        assert_eq!(
            context.query_entity_count(with!(Person, InfectionStatus::Susceptible)),
            5
        );
        assert_eq!(
            context.query_entity_count(with!(Person, InfectionStatus::Infected)),
            1
        );
        assert_eq!(
            context.query_entity_count(with!(Person, InfectionStatus::Recovered)),
            0
        );

        context.set_property(person1, InfectionStatus::Recovered);

        assert_eq!(
            context.query_entity_count(with!(Person, InfectionStatus::Susceptible)),
            5
        );
        assert_eq!(
            context.query_entity_count(with!(Person, InfectionStatus::Infected)),
            0
        );
        assert_eq!(
            context.query_entity_count(with!(Person, InfectionStatus::Recovered)),
            1
        );

        // Check derived property index is correctly maintained.
        assert_eq!(
            context.query_entity_count(with!(Person, AgeGroup::Child)),
            0
        );
        assert_eq!(
            context.query_entity_count(with!(Person, AgeGroup::Adult)),
            6
        );
        assert_eq!(
            context.query_entity_count(with!(Person, AgeGroup::Senior)),
            0
        );

        context.set_property(person2, Age(12));

        assert_eq!(
            context.query_entity_count(with!(Person, AgeGroup::Child)),
            1
        );
        assert_eq!(
            context.query_entity_count(with!(Person, AgeGroup::Adult)),
            5
        );
        assert_eq!(
            context.query_entity_count(with!(Person, AgeGroup::Senior)),
            0
        );

        context.set_property(person1, Age(75));

        assert_eq!(
            context.query_entity_count(with!(Person, AgeGroup::Child)),
            1
        );
        assert_eq!(
            context.query_entity_count(with!(Person, AgeGroup::Adult)),
            4
        );
        assert_eq!(
            context.query_entity_count(with!(Person, AgeGroup::Senior)),
            1
        );

        context.set_property(person2, Age(77));

        assert_eq!(
            context.query_entity_count(with!(Person, AgeGroup::Child)),
            0
        );
        assert_eq!(
            context.query_entity_count(with!(Person, AgeGroup::Adult)),
            4
        );
        assert_eq!(
            context.query_entity_count(with!(Person, AgeGroup::Senior)),
            2
        );
    }

    #[test]
    fn query_unindexed_default_properties() {
        let mut context = Context::new();

        // Half will have the default value.
        for idx in 0..10 {
            if idx % 2 == 0 {
                context.add_entity(with!(Person, Age(22))).unwrap();
            } else {
                context
                    .add_entity(with!(Person, Age(22), InfectionStatus::Recovered))
                    .unwrap();
            }
        }
        // The tail also has the default value
        for _ in 0..10 {
            let _: PersonId = context.add_entity(with!(Person, Age(22))).unwrap();
        }

        assert_eq!(
            context.query_entity_count(with!(Person, InfectionStatus::Recovered)),
            5
        );
        assert_eq!(
            context.query_entity_count(with!(Person, InfectionStatus::Susceptible)),
            15
        );
    }

    #[test]
    fn query_unindexed_derived_properties() {
        let mut context = Context::new();

        for _ in 0..10 {
            let _: PersonId = context.add_entity(with!(Person, Age(22))).unwrap();
        }

        assert_eq!(
            context.query_entity_count(with!(Person, AdultAthlete(false))),
            10
        );
    }

    #[test]
    fn track_periodic_value_change_counts_uses_distinct_counters() {
        let mut context = Context::new();

        context.track_periodic_value_change_counts::<Person, (CounterStratum,), CounterValue, _>(
            1.0,
            move |_context, _counter| {},
        );

        context.track_periodic_value_change_counts::<Person, (CounterStratum,), CounterValue, _>(
            1.0,
            move |_context, _counter| {},
        );

        let property_value_store = context.get_property_value_store::<Person, CounterValue>();
        assert_eq!(property_value_store.value_change_counters.len(), 0);

        context.add_plan(0.5, Context::shutdown);
        context.execute();

        let property_value_store = context.get_property_value_store::<Person, CounterValue>();
        assert_eq!(property_value_store.value_change_counters.len(), 2);
    }

    #[test]
    fn value_change_counter_updates_on_true_transitions() {
        let mut context = Context::new();
        let observed = Rc::new(RefCell::new(Vec::<(usize, usize)>::new()));
        let observed_clone = observed.clone();

        context.track_periodic_value_change_counts(1.0, move |_context, counter| {
            observed_clone.borrow_mut().push((
                counter.get_count((CounterStratum(true),), CounterValue(1)),
                counter.get_count((CounterStratum(true),), CounterValue(2)),
            ));
        });

        let person = context
            .add_entity(with!(
                Person,
                Age(10),
                CounterValue(0),
                CounterStratum(true)
            ))
            .unwrap();
        context.add_plan(0.1, move |context| {
            context.set_property(person, CounterValue(1));
            context.set_property(person, CounterValue(1));
            context.set_property(person, CounterValue(2));
        });

        context.execute();
        assert_eq!(*observed.borrow(), vec![(0, 0), (1, 1)]);
    }

    #[test]
    fn periodic_value_change_counts_report_and_clear() {
        let mut context = Context::new();
        let person = context
            .add_entity(with!(
                Person,
                Age(10),
                CounterValue(0),
                CounterStratum(true)
            ))
            .unwrap();

        let observed = Rc::new(RefCell::new(Vec::<usize>::new()));
        let observed_clone = observed.clone();

        context.track_periodic_value_change_counts(1.0, move |_context, counter| {
            observed_clone
                .borrow_mut()
                .push(counter.get_count((CounterStratum(true),), CounterValue(1)));
        });

        context.add_plan(0.5, move |context| {
            context.set_property(person, CounterValue(1));
        });
        context.add_plan(1.5, move |context| {
            context.set_property(person, CounterValue(1));
        });

        context.execute();
        assert_eq!(*observed.borrow(), vec![0, 1, 0]);
    }

    #[test]
    fn periodic_value_change_counts_start_time_and_phase_behavior() {
        let mut context = Context::new();
        context.set_start_time(-2.0);

        let person = context
            .add_entity(with!(
                Person,
                Age(10),
                CounterValue(0),
                CounterStratum(true)
            ))
            .unwrap();

        let observed_times = Rc::new(RefCell::new(Vec::<f64>::new()));
        let observed_counts = Rc::new(RefCell::new(Vec::<usize>::new()));
        let observed_times_clone = observed_times.clone();
        let observed_counts_clone = observed_counts.clone();

        context.track_periodic_value_change_counts(1.0, move |context, counter| {
            observed_times_clone
                .borrow_mut()
                .push(context.get_current_time());
            observed_counts_clone
                .borrow_mut()
                .push(counter.get_count((CounterStratum(true),), CounterValue(1)));
        });

        context.add_plan_with_phase(
            -2.0,
            move |context| {
                context.set_property(person, CounterValue(1));
            },
            ExecutionPhase::Normal,
        );
        context.add_plan(0.0, |_| {});

        context.execute();

        assert_eq!(*observed_times.borrow(), vec![-2.0, -1.0, 0.0]);
        assert_eq!(*observed_counts.borrow(), vec![1, 0, 0]);
    }

    #[cfg(feature = "profiling")]
    fn query_count(context: &Context, query: &str) -> Option<usize> {
        context.query_timing(query).map(|timing| timing.count)
    }

    #[cfg(feature = "profiling")]
    fn query_total(context: &Context, query: &str) -> Option<Duration> {
        context.query_timing(query).map(|timing| timing.total)
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn query_identity_aggregates_unordered_properties_and_ignores_values() {
        let mut context = Context::new();
        context
            .add_entity(with!(ProfilingPerson, ProfilingAge(42), ProfilingCounty(1)))
            .unwrap();
        context
            .add_entity(with!(ProfilingPerson, ProfilingAge(7), ProfilingCounty(2)))
            .unwrap();

        let label = "ProfilingPerson: (ProfilingAge, ProfilingCounty)";
        assert_eq!(
            context
                .query_result_iterator(with!(ProfilingPerson, ProfilingAge(42), ProfilingCounty(1)))
                .count(),
            1
        );
        assert_eq!(
            context
                .query_result_iterator(with!(ProfilingPerson, ProfilingCounty(1), ProfilingAge(42)))
                .count(),
            1
        );
        assert_eq!(
            context
                .query_result_iterator(with!(ProfilingPerson, ProfilingAge(7), ProfilingCounty(2)))
                .count(),
            1
        );

        assert_eq!(query_count(&context, label), Some(3));
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn iterator_consumption_styles_each_record_one_execution() {
        let mut context = Context::new();
        let person = context
            .add_entity(with!(ProfilingBoundaryPerson, ProfilingBoundaryAge(42)))
            .unwrap();

        let label = "ProfilingBoundaryPerson: (ProfilingBoundaryAge)";
        assert_eq!(
            context
                .query_result_iterator(with!(ProfilingBoundaryPerson, ProfilingBoundaryAge(42)))
                .count(),
            1
        );
        assert_eq!(query_count(&context, label), Some(1));
        let timing = context.query_timing(label).unwrap();
        assert_eq!(timing.min, timing.total);
        assert_eq!(timing.max, timing.total);

        let mut iter =
            context.query_result_iterator(with!(ProfilingBoundaryPerson, ProfilingBoundaryAge(42)));
        assert_eq!(iter.next(), Some(person));
        assert_eq!(query_count(&context, label), Some(1));
        assert_eq!(iter.next(), None);
        assert_eq!(query_count(&context, label), Some(2));
        assert_eq!(iter.next(), None);
        assert_eq!(query_count(&context, label), Some(2));
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn partially_consumed_iterator_records_one_execution_when_dropped() {
        let mut context = Context::new();
        let person = context
            .add_entity(with!(ProfilingBoundaryPerson, ProfilingBoundaryAge(42)))
            .unwrap();

        let label = "ProfilingBoundaryPerson: (ProfilingBoundaryAge)";
        let mut iter =
            context.query_result_iterator(with!(ProfilingBoundaryPerson, ProfilingBoundaryAge(42)));
        assert_eq!(iter.next(), Some(person));
        assert_eq!(query_count(&context, label), None);
        drop(iter);
        assert_eq!(query_count(&context, label), Some(1));
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn unused_query_result_iterator_does_not_record_query_timing() {
        let mut context = Context::new();
        context
            .add_entity(with!(
                ProfilingUnusedIteratorPerson,
                ProfilingUnusedIteratorAge(42)
            ))
            .unwrap();

        let label = "ProfilingUnusedIteratorPerson: (ProfilingUnusedIteratorAge)";
        let iter = context.query_result_iterator(with!(
            ProfilingUnusedIteratorPerson,
            ProfilingUnusedIteratorAge(42)
        ));
        drop(iter);

        assert_eq!(query_count(&context, label), None);
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn iterator_adaptor_methods_record_once_per_direct_call() {
        let mut context = Context::new();
        for age in [1, 2, 2, 3] {
            context
                .add_entity(with!(ProfilingIdlePerson, ProfilingIdleAge(age)))
                .unwrap();
        }

        let label = "ProfilingIdlePerson: (ProfilingIdleAge)";
        assert_eq!(
            context
                .query_result_iterator(with!(ProfilingIdlePerson, ProfilingIdleAge(1)))
                .count(),
            1
        );
        assert!(context
            .query_result_iterator(with!(ProfilingIdlePerson, ProfilingIdleAge(2)))
            .nth(1)
            .is_some());
        context
            .query_result_iterator(with!(ProfilingIdlePerson, ProfilingIdleAge(3)))
            .for_each(|_| {});
        let folded = context
            .query_result_iterator(with!(ProfilingIdlePerson, ProfilingIdleAge(1)))
            .fold(0usize, |count, _| count + 1);
        assert_eq!(folded, 1);

        assert_eq!(query_count(&context, label), Some(4));
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn iterator_adaptor_methods_do_not_record_callback_work() {
        let mut context = Context::new();
        for age in [10, 11] {
            context
                .add_entity(with!(ProfilingIdlePerson, ProfilingIdleAge(age)))
                .unwrap();
        }

        let label = "ProfilingIdlePerson: (ProfilingIdleAge)";
        context
            .query_result_iterator(with!(ProfilingIdlePerson, ProfilingIdleAge(10)))
            .for_each(|_| std::thread::sleep(Duration::from_millis(50)));

        let folded = context
            .query_result_iterator(with!(ProfilingIdlePerson, ProfilingIdleAge(11)))
            .fold(0usize, |count, _| {
                std::thread::sleep(Duration::from_millis(50));
                count + 1
            });

        assert_eq!(folded, 1);
        assert_eq!(query_count(&context, label), Some(2));
        assert!(
            query_total(&context, label).unwrap() < Duration::from_millis(50),
            "query timing should exclude callback work"
        );
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn entity_set_operations_record_once_without_counting_callback_work() {
        use rand::rngs::StdRng;
        use rand::SeedableRng;

        let mut context = Context::new();
        let person = context
            .add_entity(with!(ProfilingCallbackPerson, ProfilingCallbackAge(42)))
            .unwrap();
        context
            .add_entity(with!(ProfilingCallbackPerson, ProfilingCallbackAge(7)))
            .unwrap();

        let label = "ProfilingCallbackPerson: (ProfilingCallbackAge)";
        context.with_query_results(
            with!(ProfilingCallbackPerson, ProfilingCallbackAge(42)),
            &mut |_people| {
                std::thread::sleep(Duration::from_millis(10));
            },
        );
        assert_eq!(query_count(&context, label), None);

        context.with_query_results(
            with!(ProfilingCallbackPerson, ProfilingCallbackAge(42)),
            &mut |people| {
                let mut rng = StdRng::seed_from_u64(1);
                assert!(people.contains(person));
                assert!(people.sample_entity(&mut rng).is_some());
                assert_eq!(people.count_and_sample_entity(&mut rng).0, 1);
                assert_eq!(people.sample_entities(&mut rng, 1).len(), 1);
            },
        );

        assert_eq!(query_count(&context, label), Some(4));
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn eager_public_query_methods_record_once_per_call() {
        let mut context = Context::new();
        let person = context
            .add_entity(with!(ProfilingContainsPerson, ProfilingContainsAge(42)))
            .unwrap();
        let other = context
            .add_entity(with!(ProfilingContainsPerson, ProfilingContainsAge(7)))
            .unwrap();

        let label = "ProfilingContainsPerson: (ProfilingContainsAge)";
        assert_eq!(
            context.query_entity_count(with!(ProfilingContainsPerson, ProfilingContainsAge(42))),
            1
        );
        assert!(context.match_entity(
            person,
            with!(ProfilingContainsPerson, ProfilingContainsAge(42))
        ));
        let mut people = vec![person, other];
        context.filter_entities(
            &mut people,
            with!(ProfilingContainsPerson, ProfilingContainsAge(42)),
        );
        assert_eq!(people, vec![person]);

        assert_eq!(query_count(&context, label), Some(3));
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn composed_entity_set_operation_preserves_single_query_profile() {
        let mut context = Context::new();
        let included = context
            .add_entity(with!(
                ProfilingComposedPerson,
                ProfilingComposedAge(42),
                ProfilingComposedCounty(1)
            ))
            .unwrap();
        let excluded = context
            .add_entity(with!(
                ProfilingComposedPerson,
                ProfilingComposedAge(42),
                ProfilingComposedCounty(2)
            ))
            .unwrap();
        context
            .add_entity(with!(
                ProfilingComposedPerson,
                ProfilingComposedAge(7),
                ProfilingComposedCounty(3)
            ))
            .unwrap();

        let exclusions = EntitySet::from_source(SourceSet::singleton(excluded));
        let count = context
            .query(with!(ProfilingComposedPerson, ProfilingComposedAge(42)))
            .difference(exclusions)
            .into_iter()
            .count();

        assert_eq!(count, 1);
        assert_eq!(
            query_count(&context, "ProfilingComposedPerson: (ProfilingComposedAge)"),
            Some(1)
        );
        assert!(context
            .query(with!(ProfilingComposedPerson, ProfilingComposedAge(42)))
            .difference(EntitySet::from_source(SourceSet::singleton(excluded)))
            .contains(included));
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn set_algebra_preserves_a_shared_query_profile() {
        let mut context = Context::new();
        context
            .add_entity(with!(
                ProfilingComposedPerson,
                ProfilingComposedAge(42),
                ProfilingComposedCounty(1)
            ))
            .unwrap();
        context
            .add_entity(with!(
                ProfilingComposedPerson,
                ProfilingComposedAge(7),
                ProfilingComposedCounty(2)
            ))
            .unwrap();

        let count = context
            .query(with!(ProfilingComposedPerson, ProfilingComposedAge(42)))
            .union(context.query(with!(ProfilingComposedPerson, ProfilingComposedAge(7))))
            .into_iter()
            .count();

        assert_eq!(count, 2);
        assert_eq!(
            query_count(&context, "ProfilingComposedPerson: (ProfilingComposedAge)"),
            Some(1)
        );
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn set_algebra_clears_distinct_query_profiles() {
        let mut context = Context::new();
        context
            .add_entity(with!(
                ProfilingComposedPerson,
                ProfilingComposedAge(42),
                ProfilingComposedCounty(1)
            ))
            .unwrap();

        let count = context
            .query(with!(ProfilingComposedPerson, ProfilingComposedAge(42)))
            .intersection(context.query(with!(ProfilingComposedPerson, ProfilingComposedCounty(1))))
            .into_iter()
            .count();

        assert_eq!(count, 1);
        assert_eq!(
            query_count(&context, "ProfilingComposedPerson: (ProfilingComposedAge)"),
            None
        );
        assert_eq!(
            query_count(
                &context,
                "ProfilingComposedPerson: (ProfilingComposedCounty)"
            ),
            None
        );
    }
}
