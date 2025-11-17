use std::any::{Any, TypeId};
use std::cell::Ref;

use log::{trace, warn};
use rand::seq::index::sample as choose_range;
use rand::Rng;
use rustc_hash::FxBuildHasher;

use crate::people::index::{process_indices, BxIndex};
use crate::people::methods::Methods;
use crate::people::query::Query;
use crate::people::{HashValueType, InitializationList, PeoplePlugin, PersonPropertyHolder};
use crate::random::{sample_multiple_from_known_length, sample_single_from_known_length};
use crate::{
    Context, ContextRandomExt, HashSet, HashSetExt, IxaError, PersonCreatedEvent, PersonId,
    PersonProperty, PersonPropertyChangeEvent, RngId, Tabulator,
};

/// A trait extension for [`Context`] that exposes the people
/// functionality.
pub trait ContextPeopleExt {
    /// Returns the current population size
    fn get_current_population(&self) -> usize;

    /// Creates a new person. The caller must supply initial values
    /// for all non-derived properties that don't have a default or an initializer.
    /// Note that although this technically takes any type that implements
    /// [`InitializationList`] it is best to take advantage of the provided
    /// syntax that implements [`InitializationList`] for tuples, such as:
    /// `let person = context.add_person((Age, 42)).unwrap();`
    ///
    /// # Errors
    /// Will return [`IxaError`] if a required initializer is not provided.
    fn add_person<T: InitializationList>(&mut self, props: T) -> Result<PersonId, IxaError>;

    /// Given a [`PersonId`] returns the value of a defined person property,
    /// initializing it if it hasn't been set yet. If no initializer is
    /// provided, and the property is not set this will panic, as long
    /// as the property has been set or subscribed to at least once before.
    /// Otherwise, Ixa doesn't know about the property.
    fn get_person_property<T: PersonProperty>(&self, person_id: PersonId, _property: T)
        -> T::Value;

    #[doc(hidden)]
    fn register_property<T: PersonProperty>(&self);

    /// Given a [`PersonId`], sets the value of a defined person property
    /// Panics if the property is not initialized. Fires a change event.
    fn set_person_property<T: PersonProperty>(
        &mut self,
        person_id: PersonId,
        _property: T,
        value: T::Value,
    );

    /// Create an index for property `T`.
    ///
    /// If an index is available, [`Context::query_people()`] will use it, so this is
    /// intended to allow faster querying of commonly used properties.
    /// Ixa may choose to create an index for its own reasons even if
    /// [`Context::index_property()`] is not called, so this function just ensures
    /// that one is created.
    fn index_property<T: PersonProperty>(&mut self, property: T);

    /// Query for all people matching a given set of criteria, calling the `callback`
    /// with an immutable reference to the fully realized result set.
    ///
    /// If you only need to count the results, use [`Context::query_people_count`]
    ///
    /// [`Context::with_query_results()`] takes any type that implements [`Query`], but
    /// instead of implementing query yourself it is best to use the automatic
    /// syntax that implements [`Query`] for a tuple of pairs of (property,
    /// value), like so: `context.query_people(((Age, 30), (Gender, Female)))`.
    fn with_query_results<Q: Query>(&self, query: Q, callback: &mut dyn FnMut(&HashSet<PersonId>));

    #[deprecated(
        since = "0.3.4",
        note = "Use `with_query_results`, which is much faster for indexed results"
    )]
    /// Query for all people matching a given set of criteria.
    ///
    /// [`Context::query_people()`] takes any type that implements [`Query`],
    /// but instead of implementing query yourself it is best
    /// to use the automatic syntax that implements [`Query`] for
    /// a tuple of pairs of (property, value), like so:
    /// `context.query_people(((Age, 30), (Gender, Female)))`.
    fn query_people<Q: Query>(&self, query: Q) -> Vec<PersonId>;

    /// Get the count of all people matching a given set of criteria.
    ///
    /// [`Context::query_people_count()`] takes any type that implements [`Query`],
    /// but instead of implementing query yourself it is best
    /// to use the automatic syntax that implements [`Query`] for
    /// a tuple of pairs of (property, value), like so:
    /// `context.query_people(((Age, 30), (Gender, Female)))`.
    ///
    /// This is intended to be slightly faster than [`Context::query_people()`]
    /// because it does not need to allocate a list. We haven't actually
    /// measured it, so the difference may be modest if any.
    fn query_people_count<Q: Query>(&self, query: Q) -> usize;

    /// Determine whether a person matches a given expression.
    ///
    /// The syntax here is the same as with [`Context::query_people()`].
    fn match_person<Q: Query>(&self, person_id: PersonId, query: Q) -> bool;

    /// Similar to [`match_person`](Self::match_person), but more efficient, it removes people
    /// from a list who do not match the given query. Note that this
    /// method modifies the vector in-place, so it is up to the caller
    /// to clone the vector if they don't want to modify their original
    /// vector.
    fn filter_people<Q: Query>(&self, people: &mut Vec<PersonId>, query: Q);
    fn tabulate_person_properties<T: Tabulator, F>(&self, tabulator: &T, print_fn: F)
    where
        F: Fn(&Context, &[String], usize);

    /// Randomly sample a person from the population of people who match the query.
    /// Returns None if no people match the query.
    ///
    /// The syntax here is the same as with [`Context::query_people()`].
    ///
    fn sample_person<R: RngId + 'static, Q: Query>(&self, rng_id: R, query: Q) -> Option<PersonId>
    where
        R::RngType: Rng;

    /// Randomly sample a list of people from the population of people who match the query.
    /// Returns an empty list if no people match the query.
    ///
    /// The syntax here is the same as with [`Context::query_people()`].
    fn sample_people<R: RngId + 'static, Q: Query>(
        &self,
        rng_id: R,
        query: Q,
        n: usize,
    ) -> Vec<PersonId>
    where
        R::RngType: Rng;
}

impl ContextPeopleExt for Context {
    fn get_current_population(&self) -> usize {
        self.get_data(PeoplePlugin).current_population
    }

    fn add_person<T: InitializationList>(&mut self, props: T) -> Result<PersonId, IxaError> {
        let data_container = self.get_data_mut(PeoplePlugin);
        // Verify that every property that was supposed to be provided
        // actually was.
        data_container.check_initialization_list(&props)?;

        // Actually add the person. Nothing can fail after this point because
        // it would leave the person in an inconsistent state.
        let person_id = data_container.add_person();

        // Initialize the properties. We set |is_initializing| to prevent
        // set_person_property() from generating an event.
        data_container.is_initializing = true;
        props.set_properties(self, person_id);
        let data_container = self.get_data_mut(PeoplePlugin);
        data_container.is_initializing = false;

        self.emit_event(PersonCreatedEvent { person_id });
        Ok(person_id)
    }

    fn get_person_property<T: PersonProperty>(&self, person_id: PersonId, property: T) -> T::Value {
        let data_container = self.get_data(PeoplePlugin);
        self.register_property::<T>();

        if T::is_derived() {
            return T::compute(self, person_id);
        }

        // Attempt to retrieve the existing value
        if let Some(value) = *data_container.get_person_property_ref(person_id, property) {
            return value;
        }

        // Initialize the property. This does not fire a change event.
        let initialized_value = T::compute(self, person_id);
        data_container.set_person_property(person_id, property, initialized_value);

        initialized_value
    }

    #[allow(clippy::single_match_else)]
    fn set_person_property<T: PersonProperty>(
        &mut self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        self.register_property::<T>();

        assert!(!T::is_derived(), "Cannot set a derived property");

        // This function can be called in two separate modes:
        //
        // 1. As a regular API function, in which case we want to
        //    emit an event and notify dependencies.
        // 2. Internally as part of initialization during add_person()
        //    in which case no events are emitted.
        //
        // Which mode it is, is determined by the data_container.is_initializing
        // property, which is set by add_person. This is complicated but
        // necessary because the initialization functions are called by
        // a per-PersonProperty closure generated by a macro and so are
        // outside of the crate, but we don't want to expose a public
        // initialize_person_property() function.
        //
        // Temporarily remove dependency properties since we need mutable references
        // to self during callback execution
        let initializing = self.get_data(PeoplePlugin).is_initializing;

        let (previous_value, deps_temp) = if initializing {
            (None, None)
        } else {
            let previous_value = self.get_person_property(person_id, property);
            if previous_value != value {
                self.remove_from_index_maybe(person_id, property);
            }

            (
                Some(previous_value),
                self.get_data(PeoplePlugin)
                    .dependency_map
                    .borrow_mut()
                    .get_mut(&T::type_id())
                    .map(std::mem::take),
            )
        };

        let mut dependency_event_callbacks = Vec::new();
        if let Some(mut deps) = deps_temp {
            // If there are dependencies, set up a bunch of callbacks with the
            // current value
            for dep in &mut deps {
                dep.dependency_changed(self, person_id, &mut dependency_event_callbacks);
            }

            // Put the dependency list back in
            let data_container = self.get_data(PeoplePlugin);
            let mut dependencies = data_container.dependency_map.borrow_mut();
            dependencies.insert(T::type_id(), deps);
        }

        // Update the main property and send a change event
        let data_container = self.get_data(PeoplePlugin);
        data_container.set_person_property(person_id, property, value);

        if !initializing {
            if previous_value.unwrap() != value {
                self.add_to_index_maybe(person_id, property);
            }

            let change_event: PersonPropertyChangeEvent<T> = PersonPropertyChangeEvent {
                person_id,
                current: value,
                previous: previous_value.unwrap(), // This must be Some() if !initializing
            };
            self.emit_event(change_event);
        }

        for callback in dependency_event_callbacks {
            callback(self);
        }
    }

    fn index_property<T: PersonProperty>(&mut self, property: T) {
        trace!("indexing property {}", T::name());
        self.register_property::<T>();

        let data_container = self.get_data(PeoplePlugin);
        data_container.set_property_indexed(true, property);
    }

    fn query_people<T: Query>(&self, q: T) -> Vec<PersonId> {
        T::setup(&q, self);
        let mut result = Vec::new();
        self.query_people_internal(
            |person| {
                result.push(person);
            },
            q,
        );
        result
    }

    fn query_people_count<T: Query>(&self, q: T) -> usize {
        T::setup(&q, self);
        let mut count: usize = 0;
        self.query_people_internal(
            |_person| {
                count += 1;
            },
            q,
        );
        count
    }

    fn match_person<T: Query>(&self, person_id: PersonId, q: T) -> bool {
        T::setup(&q, self);
        // This cannot fail because someone must have been made by now.
        let data_container = self.get_data(PeoplePlugin);

        let query = q.get_query();

        for (t, hash) in &query {
            let methods = data_container.get_methods(*t);
            if *hash != (*methods.indexer)(self, person_id) {
                return false;
            }
        }
        true
    }

    fn filter_people<T: Query>(&self, people: &mut Vec<PersonId>, q: T) {
        T::setup(&q, self);
        let data_container = self.get_data(PeoplePlugin);
        for (t, hash) in q.get_query() {
            let methods = data_container.get_methods(t);
            people.retain(|person_id| hash == (*methods.indexer)(self, *person_id));
            if people.is_empty() {
                break;
            }
        }
    }

    fn register_property<T: PersonProperty>(&self) {
        let data_container = self.get_data(PeoplePlugin);
        if data_container
            .registered_properties
            .borrow()
            .contains(&T::type_id())
        {
            return;
        }

        let instance = T::get_instance();

        // In order to avoid borrowing recursively, we must register dependencies first.
        if instance.is_derived() {
            T::register_dependencies(self);

            let dependencies = instance.non_derived_dependencies();
            for dependency in dependencies {
                let mut dependency_map = data_container.dependency_map.borrow_mut();
                let derived_prop_list = dependency_map.entry(dependency).or_default();
                derived_prop_list.push(Box::new(instance));
            }
        }

        // TODO<ryl8@cdc.gov>: We create an index for every property in order to
        // support the get_person_property_by_name() function used in external_api.rs,
        // but ideally this shouldn't be necessary.
        data_container
            .methods
            .borrow_mut()
            .insert(T::type_id(), Methods::new::<T>());
        data_container
            .people_types
            .borrow_mut()
            .insert(T::name().to_string(), T::type_id());
        data_container
            .registered_properties
            .borrow_mut()
            .insert(T::type_id());

        self.register_indexer::<T>();
    }

    fn tabulate_person_properties<T: Tabulator, F>(&self, tabulator: &T, print_fn: F)
    where
        F: Fn(&Context, &[String], usize),
    {
        trace!("tabulating properties for {:?}", tabulator.get_columns());
        let type_ids = tabulator.get_typelist();
        tabulator.setup(self).unwrap();

        let data_container = self.get_data(PeoplePlugin);
        for type_id in &type_ids {
            data_container.set_property_indexed_by_type_id(true, *type_id);
            data_container.index_unindexed_people_for_type_id(self, *type_id);
        }

        let index_container = data_container.property_indexes.borrow();
        let indices = type_ids
            .iter()
            .filter_map(|t| index_container.get(t))
            .collect::<Vec<&BxIndex>>();

        process_indices(
            self,
            indices.as_slice(),
            &mut Vec::new(),
            &HashSet::default(),
            &print_fn,
        );
    }

    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    fn sample_people<R: RngId + 'static, Q: Query>(
        &self,
        rng_id: R,
        query: Q,
        n: usize,
    ) -> Vec<PersonId>
    where
        R::RngType: Rng,
    {
        if n == 1 {
            return match self.sample_person(rng_id, query) {
                None => {
                    vec![]
                }
                Some(person) => {
                    vec![person]
                }
            };
        }

        let current_population = self.get_current_population();

        let requested = std::cmp::min(n, current_population);
        if requested == 0 {
            warn!(
                "Requested a sample of {} people from a population of {}",
                n, current_population
            );
            return Vec::new();
        }

        // Special case the empty query because we can do it in O(1).
        if query.type_id() == TypeId::of::<()>() {
            let selected = self
                .sample(rng_id, |rng| {
                    choose_range(rng, current_population, requested)
                        .into_iter()
                        .map(PersonId)
                })
                .collect();
            return selected;
        }

        Q::setup(&query, self);

        // Check if this query is indexed. This is a "known length" case.
        if let Some(multi_property_id) = query.multi_property_type_id() {
            let container = self.get_data(PeoplePlugin);
            // Get the mutable index, because we need to refresh the index.
            if let Some(index) = container
                .property_indexes
                .borrow_mut()
                .get_mut(&multi_property_id)
            {
                // Make sure the index isn't stale.
                index.index_unindexed_people(self);

                if let Some(people_set) = index.get_with_hash(query.multi_property_value_hash()) {
                    // If there are not enough items in the set to satisfy the request, return as
                    // many as we can.
                    if people_set.len() <= requested {
                        return people_set.to_owned_vec();
                    }

                    // This is slightly faster than "Algorithm L" reservoir sampling when requested << ~5
                    // and always much faster than the reservoir sampling algorithm in `rand`.
                    return self.sample(rng_id, |rng| {
                        sample_multiple_from_known_length(rng, people_set, requested)
                    });
                }
            }
        }

        // This is the "unknown length" case. This algorithm is *much*
        // faster than the reservoir algorithm implemented in `rand`.
        // This implements "Algorithm L" from KIM-HUNG LI, Reservoir-
        // Sampling Algorithms of Time Complexity O(n(1 + log(N/n)))
        // https://dl.acm.org/doi/pdf/10.1145/198429.198435
        let mut weight: f64 = self.sample_range(rng_id, 0.0..1.0); // controls skip distance distribution
        weight = weight.powf(1.0 / requested as f64);
        let mut position: usize = 0; // current index in data
        let mut next_pick_position: usize = 1; // index of the next item to pick
        let mut reservoir = Vec::with_capacity(requested); // the sample reservoir

        // ToDo(RobertJacobsonCDC): This will use `iter_query_results` API when it is ready.
        self.query_people_internal(
            |person| {
                position += 1;
                if position == next_pick_position {
                    if reservoir.len() == requested {
                        let to_remove = self.sample_range(rng_id, 0..reservoir.len());
                        reservoir.swap_remove(to_remove);
                    }
                    reservoir.push(person);

                    if reservoir.len() == requested {
                        let uniform_random: f64 = self.sample_range(rng_id, 0.0..1.0);
                        next_pick_position +=
                            (f64::ln(uniform_random) / f64::ln(1.0 - weight)).floor() as usize + 1;
                        let uniform_random: f64 = self.sample_range(rng_id, 0.0..1.0);
                        weight *= uniform_random.powf(1.0 / requested as f64);
                    } else {
                        next_pick_position += 1;
                    }
                }
            },
            query,
        );

        reservoir
    }

    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    fn sample_person<R: RngId + 'static, Q: Query>(&self, rng_id: R, query: Q) -> Option<PersonId>
    where
        R::RngType: Rng,
    {
        let current_population = self.get_current_population();

        if current_population == 0 {
            warn!("Requested a sample person from an empty population");
            return None;
        }

        // Special case the empty query because we can do it in O(1).
        if query.type_id() == TypeId::of::<()>() {
            let result = self.sample_range(rng_id, 0..current_population);
            return Some(PersonId(result));
        }

        Q::setup(&query, self);

        // Check if this query is indexed. This is a "known length" case.
        if let Some(multi_property_id) = query.multi_property_type_id() {
            let container = self.get_data(PeoplePlugin);
            // Get the mutable index, because we need to refresh the index.
            if let Some(index) = container
                .property_indexes
                .borrow_mut()
                .get_mut(&multi_property_id)
            {
                // Make sure the index isn't stale.
                index.index_unindexed_people(self);

                if let Some(people_set) = index.get_with_hash(query.multi_property_value_hash()) {
                    return self.sample(rng_id, |rng| {
                        sample_single_from_known_length(rng, people_set)
                    });
                }
            }
        }

        // This is the "unknown length" case. This algorithm is *much*
        // faster than the reservoir algorithm implemented in `rand`.
        // This implements "Algorithm L" from KIM-HUNG LI, Reservoir-
        // Sampling Algorithms of Time Complexity O(n(1 + log(N/n)))
        // https://dl.acm.org/doi/pdf/10.1145/198429.198435
        let mut selected: Option<PersonId> = None;
        let mut weight: f64 = self.sample_range(rng_id, 0.0..1.0);
        let mut position: usize = 0;
        let mut next_pick_position: usize = 1;

        // ToDo(RobertJacobsonCDC): This will use `random::sample_single_l_reservoir`
        //     when the `iter_query_results` API is ready.
        self.query_people_internal(
            |person| {
                position += 1;
                if next_pick_position == position {
                    selected = Some(person);
                    // `f32` arithmetic is no faster than `f64` on modern hardware.
                    next_pick_position += (f64::ln(self.sample_range(rng_id, 0.0..1.0))
                        / f64::ln(1.0 - weight))
                    .floor() as usize
                        + 1;
                    weight *= self.sample_range(rng_id, 0.0..1.0);
                }
            },
            query,
        );

        selected
    }

    fn with_query_results<Q: Query>(&self, query: Q, callback: &mut dyn FnMut(&HashSet<PersonId>)) {
        // Special case the empty query, which creates a set containing the entire population.
        if query.type_id() == TypeId::of::<()>() {
            let mut people_set =
                HashSet::with_capacity_and_hasher(self.get_current_population(), FxBuildHasher);
            (0..self.get_current_population()).for_each(|i| {
                people_set.insert(PersonId(i));
            });
            callback(&people_set);
            return;
        }

        Q::setup(&query, self);
        let data_container = self.get_data(PeoplePlugin);

        // The fast path for queries that are indexed.
        if let Some(multi_property_id) = query.multi_property_type_id() {
            // Make sure the index isn't stale.
            data_container.index_unindexed_people_for_type_id(self, multi_property_id);

            if let Some(index) = data_container
                .property_indexes
                .borrow()
                .get(&multi_property_id)
            {
                if index.is_indexed() {
                    let value = query.multi_property_value_hash();
                    if let Some(people_set) = index.get_with_hash(value) {
                        callback(people_set);
                        return;
                    } else {
                        let empty = HashSet::default();
                        callback(&empty);
                        return;
                    }
                }
            }
        }

        // ToDo(Robert): This will use `iter_query_results` API when it is ready.
        // The slow path: compute the result set.
        let mut result = HashSet::default();
        self.query_people_internal(
            |person| {
                result.insert(person);
            },
            query,
        );
        callback(&result);
    }
}

pub trait ContextPeopleExtInternal {
    fn register_indexer<T: PersonProperty>(&self);
    fn add_to_index_maybe<T: PersonProperty>(&mut self, person_id: PersonId, property: T);
    fn remove_from_index_maybe<T: PersonProperty>(&mut self, person_id: PersonId, property: T);
    fn query_people_internal<Q: Query>(&self, accumulator: impl FnMut(PersonId), query: Q);
}

impl ContextPeopleExtInternal for Context {
    fn register_indexer<T: PersonProperty>(&self) {
        let data_container = self.get_data(PeoplePlugin);
        // Create an index object if it doesn't exist.
        data_container.register_index::<T>();
    }

    /// If the property is being indexed, add the person to the property's index.
    fn add_to_index_maybe<T: PersonProperty>(&mut self, person_id: PersonId, property: T) {
        let data_container = self.get_data(PeoplePlugin);
        let value = self.get_person_property(person_id, property);
        data_container.add_person_if_indexed::<T>(T::make_canonical(value), person_id);
    }

    /// If the property is being indexed, add the person to the property's index.
    fn remove_from_index_maybe<T: PersonProperty>(&mut self, person_id: PersonId, property: T) {
        let data_container = self.get_data(PeoplePlugin);
        let value = self.get_person_property(person_id, property);
        data_container.remove_person_if_indexed::<T>(T::make_canonical(value), person_id);
    }

    fn query_people_internal<Q: Query>(&self, mut accumulator: impl FnMut(PersonId), query: Q) {
        let mut indexes = Vec::<Ref<HashSet<PersonId>>>::new();
        let mut unindexed = Vec::<(TypeId, HashValueType)>::new();
        let data_container = self.get_data(PeoplePlugin);

        let property_hashes_working_set: Vec<(TypeId, HashValueType)> =
            if let Some(multi_property_id) = query.multi_property_type_id() {
                let combined_hash = query.multi_property_value_hash();
                vec![(multi_property_id, combined_hash)]
            } else {
                query.get_query()
            };

        // 1. Walk through each property and update the indexes.
        for (t, _) in &property_hashes_working_set {
            data_container.index_unindexed_people_for_type_id(self, *t);
        }

        // 2. Collect the index entry corresponding to the value.
        for (t, hash) in property_hashes_working_set {
            let (is_indexed, people_set) = data_container.get_people_for_id_hash(t, hash);
            if is_indexed {
                if let Some(matching_people) = people_set {
                    indexes.push(matching_people);
                } else {
                    // This is empty and so the intersection will
                    // also be empty.
                    return;
                }
            } else {
                // No index, so we'll get to this after.
                unindexed.push((t, hash));
            }
        }

        // 3. Create an iterator over people, based on either:
        //    (1) the smallest index if there is one.
        //    (2) the overall population if there are no indices.

        let holder: Ref<HashSet<PersonId>>;
        let to_check: Box<dyn Iterator<Item = PersonId>> = if indexes.is_empty() {
            Box::new(data_container.people_iterator())
        } else {
            indexes.sort_by_key(|x| x.len());

            holder = indexes.remove(0);
            Box::new(holder.iter().copied())
        };

        // 4. Walk over the iterator and add people to the result
        // iff:
        //    (1) they exist in all the indexes
        //    (2) they match the unindexed properties
        'outer: for person in to_check {
            // (1) check all the indexes
            for index in &indexes {
                if !index.contains(&person) {
                    continue 'outer;
                }
            }

            // (2) check the unindexed properties
            for (t, hash) in &unindexed {
                let methods = data_container.get_methods(*t);
                if *hash != (*methods.indexer)(self, person) {
                    continue 'outer;
                }
            }

            // This matches.
            accumulator(person);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::rc::Rc;

    use serde_derive::Serialize;

    use crate::people::{PeoplePlugin, PersonProperty, PersonPropertyHolder};
    use crate::random::{define_rng, ContextRandomExt};
    use crate::{
        define_derived_property, define_global_property, define_person_property,
        define_person_property_with_default, Context, ContextGlobalPropertiesExt, ContextPeopleExt,
        HashSetExt, IxaError, PersonId, PersonPropertyChangeEvent,
    };

    define_person_property!(Age, u8);
    #[derive(Serialize, Copy, Clone, Debug, PartialEq, Eq)]
    pub enum AgeGroupValue {
        Child,
        Adult,
    }
    define_global_property!(ThresholdP, u8);
    define_derived_property!(IsEligible, bool, [Age], [ThresholdP], |age, threshold| {
        &age >= threshold
    });

    #[allow(dead_code)]
    mod unused {
        use super::*;
        // This isn't used, it's just testing for a compile error.
        define_derived_property!(
            NotUsed,
            bool,
            [Age],
            [ThresholdP, ThresholdP],
            |age, threshold, threshold2| { &age >= threshold && &age <= threshold2 }
        );
    }

    define_derived_property!(AgeGroup, AgeGroupValue, [Age], |age| {
        if age < 18 {
            AgeGroupValue::Child
        } else {
            AgeGroupValue::Adult
        }
    });

    #[derive(Serialize, Copy, Clone, PartialEq, Eq, Debug)]
    pub enum RiskCategoryValue {
        High,
        Low,
    }

    define_person_property!(RiskCategory, RiskCategoryValue);
    define_person_property_with_default!(IsRunner, bool, false);
    define_person_property!(RunningShoes, u8, |context: &Context, person: PersonId| {
        let is_runner = context.get_person_property(person, IsRunner);
        if is_runner {
            4
        } else {
            0
        }
    });
    define_derived_property!(AdultRunner, bool, [IsRunner, Age], |is_runner, age| {
        is_runner && age >= 18
    });
    define_derived_property!(
        SeniorRunner,
        bool,
        [AdultRunner, Age],
        |adult_runner, age| { adult_runner && age >= 65 }
    );
    define_person_property_with_default!(IsSwimmer, bool, false);
    define_derived_property!(AdultSwimmer, bool, [IsSwimmer, Age], |is_swimmer, age| {
        is_swimmer && age >= 18
    });
    define_derived_property!(
        AdultAthlete,
        bool,
        [AdultRunner, AdultSwimmer],
        |adult_runner, adult_swimmer| { adult_runner || adult_swimmer }
    );

    #[derive(Serialize, Copy, Clone, PartialEq, Eq, Debug)]
    pub enum InfectionStatusValue {
        #[allow(unused)]
        Susceptible,
        #[allow(unused)]
        Infectious,
        #[allow(unused)]
        Recovered,
    }
    define_person_property_with_default!(
        InfectionStatus,
        InfectionStatusValue,
        InfectionStatusValue::Susceptible
    );

    #[test]
    fn set_get_properties() {
        let mut context = Context::new();

        let person = context.add_person((Age, 42)).unwrap();
        assert_eq!(context.get_person_property(person, Age), 42);
    }

    #[allow(clippy::should_panic_without_expect)]
    #[test]
    #[should_panic]
    fn get_uninitialized_property_panics() {
        // The `PersonProperty::compute()` implementation panics if there is no default value.
        let mut context = Context::new();
        let person = context.add_person(()).unwrap();
        context.get_person_property(person, Age);
    }

    #[test]
    fn get_current_population() {
        let mut context = Context::new();
        assert_eq!(context.get_current_population(), 0);
        for _ in 0..3 {
            context.add_person(()).unwrap();
        }
        assert_eq!(context.get_current_population(), 3);
    }

    #[test]
    fn add_person() {
        let mut context = Context::new();

        let person_id = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();
        assert_eq!(context.get_person_property(person_id, Age), 42);
        assert_eq!(
            context.get_person_property(person_id, RiskCategory),
            RiskCategoryValue::Low
        );
    }

    #[test]
    fn add_person_with_initialize() {
        let mut context = Context::new();

        let person_id = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();
        assert_eq!(context.get_person_property(person_id, Age), 42);
        assert_eq!(
            context.get_person_property(person_id, RiskCategory),
            RiskCategoryValue::Low
        );
    }

    #[test]
    fn add_person_with_initialize_missing() {
        let mut context = Context::new();

        context.add_person((Age, 10)).unwrap();
        // Fails because we don't provide a value for Age
        assert!(matches!(context.add_person(()), Err(IxaError::IxaError(_))));
    }

    #[test]
    fn add_person_with_initialize_missing_first() {
        let mut context = Context::new();

        // Succeeds because context doesn't know about any properties
        // yet.
        context.add_person(()).unwrap();
    }

    #[test]
    fn add_person_with_initialize_missing_with_default() {
        let mut context = Context::new();

        context.add_person((IsRunner, true)).unwrap();
        // Succeeds because |IsRunner| has a default.
        context.add_person(()).unwrap();
    }

    #[test]
    fn person_debug_display() {
        let mut context = Context::new();

        let person_id = context.add_person(()).unwrap();
        assert_eq!(format!("{person_id}"), "0");
        assert_eq!(format!("{person_id:?}"), "Person 0");
    }

    #[test]
    fn add_person_initializers() {
        let mut context = Context::new();
        let person_id = context.add_person(()).unwrap();

        assert_eq!(context.get_person_property(person_id, RunningShoes), 0);
        assert!(!context.get_person_property(person_id, IsRunner));
    }

    #[test]
    fn property_initialization_is_lazy() {
        let mut context = Context::new();
        let person = context.add_person((IsRunner, true)).unwrap();
        let people_data = context.get_data_mut(PeoplePlugin);

        // Verify we haven't initialized the property yet
        let has_value = *people_data.get_person_property_ref(person, RunningShoes);
        assert!(has_value.is_none());

        // This should initialize it
        let value = context.get_person_property(person, RunningShoes);
        assert_eq!(value, 4);
    }

    #[test]
    fn initialize_without_initializer_succeeds() {
        let mut context = Context::new();
        context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();
    }

    #[test]
    #[should_panic(expected = "Property not initialized when person created")]
    fn set_without_initializer_panics() {
        let mut context = Context::new();
        let person_id = context.add_person(()).unwrap();
        context.set_person_property(person_id, RiskCategory, RiskCategoryValue::High);
    }

    #[test]
    #[should_panic(expected = "Property not initialized when person created")]
    fn get_without_initializer_panics() {
        let mut context = Context::new();
        let person_id = context.add_person(()).unwrap();
        context.get_person_property(person_id, RiskCategory);
    }

    #[test]
    fn get_person_property_returns_correct_value() {
        let mut context = Context::new();
        let person = context.add_person((Age, 10)).unwrap();
        assert_eq!(
            context.get_person_property(person, AgeGroup),
            AgeGroupValue::Child
        );
    }

    #[test]
    fn get_person_property_changes_correctly() {
        let mut context = Context::new();
        let person = context.add_person((Age, 17)).unwrap();
        assert_eq!(
            context.get_person_property(person, AgeGroup),
            AgeGroupValue::Child
        );
        context.set_person_property(person, Age, 18);
        assert_eq!(
            context.get_person_property(person, AgeGroup),
            AgeGroupValue::Adult
        );
    }

    #[test]
    fn get_derived_property_multiple_deps() {
        let mut context = Context::new();
        let person = context.add_person(((Age, 17), (IsRunner, true))).unwrap();
        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, event: PersonPropertyChangeEvent<AdultRunner>| {
                assert_eq!(event.person_id.0, 0);
                assert!(!event.previous);
                assert!(event.current);
                *flag_clone.borrow_mut() = true;
            },
        );
        context.set_person_property(person, Age, 18);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn register_derived_only_once() {
        let mut context = Context::new();
        let person = context.add_person(((Age, 17), (IsRunner, true))).unwrap();

        let flag = Rc::new(RefCell::new(0));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, _event: PersonPropertyChangeEvent<AdultRunner>| {
                *flag_clone.borrow_mut() += 1;
            },
        );
        context.subscribe_to_event(
            move |_context, _event: PersonPropertyChangeEvent<AdultRunner>| {
                // Make sure that we don't register multiple times
            },
        );
        context.set_person_property(person, Age, 18);
        context.execute();
        assert_eq!(*flag.borrow(), 1);
    }

    #[test]
    fn test_resolve_dependencies() {
        let mut actual = SeniorRunner.non_derived_dependencies();
        let mut expected = vec![Age::type_id(), IsRunner::type_id()];
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
    }

    #[test]
    fn get_derived_property_dependent_on_another_derived() {
        let mut context = Context::new();
        let person = context.add_person(((Age, 88), (IsRunner, false))).unwrap();
        let flag = Rc::new(RefCell::new(0));
        let flag_clone = flag.clone();
        assert!(!context.get_person_property(person, SeniorRunner));
        context.subscribe_to_event(
            move |_context, event: PersonPropertyChangeEvent<SeniorRunner>| {
                assert_eq!(event.person_id.0, 0);
                assert!(!event.previous);
                assert!(event.current);
                *flag_clone.borrow_mut() += 1;
            },
        );
        context.set_person_property(person, IsRunner, true);
        context.execute();
        assert_eq!(*flag.borrow(), 1);
    }

    #[test]
    fn get_derived_property_diamond_dependencies() {
        let mut context = Context::new();
        let person = context.add_person(((Age, 17), (IsSwimmer, true))).unwrap();

        let flag = Rc::new(RefCell::new(0));
        let flag_clone = flag.clone();
        assert!(!context.get_person_property(person, AdultAthlete));
        context.subscribe_to_event(
            move |_context, event: PersonPropertyChangeEvent<AdultAthlete>| {
                assert_eq!(event.person_id.0, 0);
                assert!(!event.previous);
                assert!(event.current);
                *flag_clone.borrow_mut() += 1;
            },
        );
        context.set_person_property(person, Age, 18);
        context.execute();
        assert_eq!(*flag.borrow(), 1);
    }

    #[test]
    fn get_derived_property_with_globals() {
        let mut context = Context::new();
        context.set_global_property_value(ThresholdP, 18).unwrap();
        let child = context.add_person((Age, 17)).unwrap();
        let adult = context.add_person((Age, 19)).unwrap();
        assert!(!context.get_person_property(child, IsEligible));
        assert!(context.get_person_property(adult, IsEligible));
    }

    #[test]
    fn text_match_person() {
        let mut context = Context::new();
        let person = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::High)))
            .unwrap();
        assert!(context.match_person(person, ((Age, 42), (RiskCategory, RiskCategoryValue::High))));
        assert!(!context.match_person(person, ((Age, 43), (RiskCategory, RiskCategoryValue::High))));
        assert!(!context.match_person(person, ((Age, 42), (RiskCategory, RiskCategoryValue::Low))));
    }

    #[test]
    fn test_filter_people() {
        let mut context = Context::new();
        let _ = context.add_person((Age, 40)).unwrap();
        let _ = context.add_person((Age, 42)).unwrap();
        let _ = context.add_person((Age, 42)).unwrap();
        let mut all_people = Vec::new();

        context.with_query_results((), &mut |results| {
            all_people = results.to_owned_vec();
        });

        let mut result = all_people.clone();
        context.filter_people(&mut result, (Age, 42));
        assert_eq!(result.len(), 2);

        context.filter_people(&mut all_people, (Age, 43));
        assert!(all_people.is_empty());
    }

    #[test]
    fn test_sample_person_simple() {
        define_rng!(SampleRng1);
        let mut context = Context::new();
        context.init_random(42);
        assert!(context.sample_person(SampleRng1, ()).is_none());
        let person = context.add_person(()).unwrap();
        assert_eq!(context.sample_person(SampleRng1, ()).unwrap(), person);
    }

    #[test]
    fn test_sample_person_distribution() {
        define_rng!(SampleRng2);

        let mut context = Context::new();
        context.init_random(42);

        // Test an empty query.
        assert!(context.sample_person(SampleRng2, ()).is_none());
        let person1 = context.add_person((Age, 10)).unwrap();
        let person2 = context.add_person((Age, 10)).unwrap();
        let person3 = context.add_person((Age, 10)).unwrap();
        let person4 = context.add_person((Age, 30)).unwrap();

        // Test a non-matching query.
        assert!(context.sample_person(SampleRng2, (Age, 50)).is_none());

        // See that the simple query always returns person4
        for _ in 0..10 {
            assert_eq!(
                context.sample_person(SampleRng2, (Age, 30)).unwrap(),
                person4
            );
        }

        let mut count_p1: usize = 0;
        let mut count_p2: usize = 0;
        let mut count_p3: usize = 0;
        for _ in 0..30000 {
            let p = context.sample_person(SampleRng2, (Age, 10)).unwrap();
            if p == person1 {
                count_p1 += 1;
            } else if p == person2 {
                count_p2 += 1;
            } else if p == person3 {
                count_p3 += 1;
            } else {
                panic!("Unexpected person");
            }
        }

        // The chance of any of these being more unbalanced than this is ~10^{-4}
        assert!(count_p1 >= 8700);
        assert!(count_p2 >= 8700);
        assert!(count_p3 >= 8700);
    }

    #[test]
    fn test_sample_people_distribution() {
        define_rng!(SampleRng5);

        let mut context = Context::new();
        context.init_random(66);

        // Test an empty query.
        assert!(context.sample_person(SampleRng5, ()).is_none());
        let person1 = context.add_person((Age, 10)).unwrap();
        let person2 = context.add_person((Age, 10)).unwrap();
        let person3 = context.add_person((Age, 10)).unwrap();
        let person4 = context.add_person((Age, 44)).unwrap();
        let person5 = context.add_person((Age, 10)).unwrap();
        let person6 = context.add_person((Age, 10)).unwrap();
        let person7 = context.add_person((Age, 22)).unwrap();
        let person8 = context.add_person((Age, 10)).unwrap();

        let mut count_p1: usize = 0;
        let mut count_p2: usize = 0;
        let mut count_p3: usize = 0;
        let mut count_p5: usize = 0;
        let mut count_p6: usize = 0;
        let mut count_p8: usize = 0;
        for _ in 0..60000 {
            let p = context.sample_people(SampleRng5, (Age, 10), 2);
            if p.contains(&person1) {
                count_p1 += 1;
            }
            if p.contains(&person2) {
                count_p2 += 1;
            }
            if p.contains(&person3) {
                count_p3 += 1;
            }
            if p.contains(&person5) {
                count_p5 += 1;
            }
            if p.contains(&person6) {
                count_p6 += 1;
            }
            if p.contains(&person8) {
                count_p8 += 1;
            }
            if p.contains(&person4) || p.contains(&person7) {
                println!("Unexpected person in sample: {:?}", p);
                panic!("Unexpected person");
            }
        }

        // The chance of any of these being more unbalanced than this is ~10^{-4}
        assert!(count_p1 >= 8700);
        assert!(count_p2 >= 8700);
        assert!(count_p3 >= 8700);
        assert!(count_p5 >= 8700);
        assert!(count_p6 >= 8700);
        assert!(count_p8 >= 8700);
    }

    #[test]
    fn test_sample_people_simple() {
        define_rng!(SampleRng3);
        let mut context = Context::new();
        context.init_random(42);
        let people0 = context.sample_people(SampleRng3, (), 1);
        assert_eq!(people0.len(), 0);
        let person1 = context.add_person(()).unwrap();
        let person2 = context.add_person(()).unwrap();
        let person3 = context.add_person(()).unwrap();

        let people1 = context.sample_people(SampleRng3, (), 1);
        assert_eq!(people1.len(), 1);
        assert!(
            people1.contains(&person1) || people1.contains(&person2) || people1.contains(&person3)
        );

        let people2 = context.sample_people(SampleRng3, (), 2);
        assert_eq!(people2.len(), 2);

        let people3 = context.sample_people(SampleRng3, (), 3);
        assert_eq!(people3.len(), 3);

        let people4 = context.sample_people(SampleRng3, (), 4);
        assert_eq!(people4.len(), 3);
    }

    #[test]
    fn test_sample_people() {
        define_rng!(SampleRng4);
        let mut context = Context::new();
        context.init_random(42);
        let person1 = context
            .add_person(((Age, 40), (RiskCategory, RiskCategoryValue::High)))
            .unwrap();
        let _ = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::High)))
            .unwrap();
        let person3 = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();
        let _ = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::High)))
            .unwrap();
        let _ = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::High)))
            .unwrap();
        let person6 = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();

        // Test a non-matching query.
        assert!(context.sample_people(SampleRng4, (Age, 50), 1).is_empty());

        // See that the simple query always returns person4
        for _ in 0..10 {
            assert!(context
                .sample_people(SampleRng4, (Age, 40), 1)
                .contains(&person1));
        }

        let people1 = context.sample_people(SampleRng4, (Age, 40), 2);
        assert_eq!(people1.len(), 1);
        assert!(people1.contains(&person1));

        let people2 = context.sample_people(SampleRng4, (Age, 42), 2);
        assert_eq!(people2.len(), 2);
        assert!(!people2.contains(&person1));

        let people3 = context.sample_people(
            SampleRng4,
            ((Age, 42), (RiskCategory, RiskCategoryValue::High)),
            2,
        );
        assert_eq!(people3.len(), 2);
        assert!(
            !people3.contains(&person1)
                && !people3.contains(&person3)
                && !people3.contains(&person6)
        );
    }

    #[test]
    fn test_sample_initial_population_seed() {
        // Test that we get a uniformly distributed sample of 100 people from a population of 1000.
        define_rng!(InfectionRng);

        let mut context = Context::new();

        let seed: u64 = 42;
        let requested = 100;

        context.init_random(seed);

        for _ in 0..1000 {
            let _ = context.add_person(());
        }

        let susceptibles = context.sample_people(
            InfectionRng,
            (InfectionStatus, InfectionStatusValue::Susceptible),
            requested,
        );
        // Unwrap the IDs to get numbers
        let sample: Vec<u64> = susceptibles.into_iter().map(|p| p.0 as u64).collect();

        // Correct sample size
        assert_eq!(sample.len(), requested);

        // All sampled values are within the valid range
        assert!(sample.iter().all(|v| *v < 1000));

        // The sample should not have duplicates
        let unique: HashSet<_> = sample.iter().collect();
        assert_eq!(unique.len(), sample.len());

        // ---- Chi-square test of uniformity ----

        // Partition range 0..1000 into 10 equal-width bins
        let mut counts = [0usize; 10];
        for &value in &sample {
            let bin = (value as usize) / 100; // 0..99 → bin 0, ..., 900..999 → bin 9
            counts[bin] += 1;
        }

        // Expected count per bin for uniform sampling of 100 numbers from 0..1000
        let expected = requested as f64 / 10.0; // = 10.0

        // Compute chi-square statistic
        let chi_square: f64 = counts
            .iter()
            .map(|&obs| {
                let diff = (obs as f64) - expected;
                diff * diff / expected
            })
            .sum();

        // The critical value is just looked up in the chi-square distribution table
        // or extracted from your favorite CAS. Since we are hard-coding a random
        // seed, this test is actually deterministic. If you don't touch any of the
        // code it uses, it should always pass.

        // Degrees of freedom = (#bins - 1) = 9
        // Critical χ²₀.₉₉₉ (p = 0.001) for df=9 is 27.877
        // If chi_square > 27.877, reject uniformity at 0.1% level.
        // (Using strict 0.1% significance keeps false failures very unlikely.)
        let critical = 27.877;

        assert!(
            chi_square < critical,
            "Reservoir sampling fails chi-square test: seed = {},  χ² = {}, counts = {:?}",
            seed,
            chi_square,
            counts
        );
    }

    mod property_initialization_queries {
        use super::*;

        define_rng!(PropertyInitRng);
        define_person_property_with_default!(SimplePropWithDefault, u8, 1);
        define_derived_property!(DerivedOnce, u8, [SimplePropWithDefault], |n| n * 2);
        define_derived_property!(DerivedTwice, bool, [DerivedOnce], |n| n == 2);

        #[test]
        fn test_query_derived_property_not_initialized() {
            let mut context = Context::new();
            context.init_random(42);
            let person = context.add_person(()).unwrap();
            assert_eq!(
                context
                    .sample_person(PropertyInitRng, (DerivedOnce, 2))
                    .unwrap(),
                person
            );
        }

        #[test]
        fn test_query_derived_property_not_initialized_two_levels() {
            let mut context = Context::new();
            context.init_random(42);
            let person = context.add_person(()).unwrap();
            assert_eq!(
                context
                    .sample_person(PropertyInitRng, (DerivedTwice, true))
                    .unwrap(),
                person
            );
        }
    }
}
