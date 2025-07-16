use crate::people::index::{
    get_and_register_multi_property_index, get_multi_property_hash, Index, IndexValue,
};
use crate::people::query::Query;
use crate::people::{index, InitializationList, PeoplePlugin, PersonPropertyHolder};
use crate::{
    Context, ContextRandomExt, IxaError, PersonCreatedEvent, PersonId, PersonProperty,
    PersonPropertyChangeEvent, RngId, Tabulator,
};
use crate::{HashMap, HashMapExt, HashSet, HashSetExt};
use rand::Rng;
use std::any::TypeId;
use std::cell::Ref;

use crate::people::methods::Methods;

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

    /// Given a `PersonId` returns the value of a defined person property,
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
    /// If an index is available [`Context::query_people()`] will use it, so this is
    /// intended to allow faster querying of commonly used properties.
    /// Ixa may choose to create an index for its own reasons even if
    /// [`Context::index_property()`] is not called, so this function just ensures
    /// that one is created.
    fn index_property<T: PersonProperty>(&mut self, property: T);

    /// Query for all people matching a given set of criteria.
    ///
    /// [`Context::query_people()`] takes any type that implements [Query],
    /// but instead of implementing query yourself it is best
    /// to use the automatic syntax that implements [Query] for
    /// a tuple of pairs of (property, value), like so:
    /// `context.query_people(((Age, 30), (Gender, Female)))`.
    fn query_people<T: Query>(&self, q: T) -> Vec<PersonId>;

    /// Get the count of all people matching a given set of criteria.
    ///
    /// [`Context::query_people_count()`] takes any type that implements [Query],
    /// but instead of implementing query yourself it is best
    /// to use the automatic syntax that implements [Query] for
    /// a tuple of pairs of (property, value), like so:
    /// `context.query_people(((Age, 30), (Gender, Female)))`.
    ///
    /// This is intended to be slightly faster than [`Context::query_people()`]
    /// because it does not need to allocate a list. We haven't actually
    /// measured it, so the difference may be modest if any.
    fn query_people_count<T: Query>(&self, q: T) -> usize;

    /// Determine whether a person matches a given expression.
    ///
    /// The syntax here is the same as with [`Context::query_people()`].
    fn match_person<T: Query>(&self, person_id: PersonId, q: T) -> bool;

    /// Similar to `match_person`, but more efficient, it removes people
    /// from a list who do not match the given query. Note that this
    /// method modifies the vector in-place, so it is up to the caller
    /// to clone the vector if they don't want to modify their original
    /// vector.
    fn filter_people<T: Query>(&self, people: &mut Vec<PersonId>, q: T);
    fn tabulate_person_properties<T: Tabulator, F>(&self, tabulator: &T, print_fn: F)
    where
        F: Fn(&Context, &[String], usize);

    /// Randomly sample a person from the population of people who match the query.
    /// Returns None if no people match the query.
    ///
    /// The syntax here is the same as with [`Context::query_people()`].
    ///
    fn sample_person<R: RngId + 'static, T: Query>(&self, rng_id: R, query: T) -> Option<PersonId>
    where
        R::RngType: Rng;

    /// Randomly sample a list of people from the population of people who match the query.
    /// Returns an empty list if no people match the query.
    ///
    /// The syntax here is the same as with [`Context::query_people()`].
    fn sample_people<R: RngId + 'static, T: Query>(
        &self,
        rng_id: R,
        query: T,
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

        // Initialize the property. This does not fire a change event
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
                    .get_mut(&TypeId::of::<T>())
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
            dependencies.insert(TypeId::of::<T>(), deps);
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
                previous: previous_value.unwrap(), // This muse be Some() of !initializing
            };
            self.emit_event(change_event);
        }

        for callback in dependency_event_callbacks {
            callback(self);
        }
    }

    fn index_property<T: PersonProperty>(&mut self, _property: T) {
        // Ensure that the data container exists
        {
            let _ = self.get_data_mut(PeoplePlugin);
        }

        self.register_property::<T>();

        let data_container = self.get_data(PeoplePlugin);
        let mut index = data_container
            .get_index_ref_mut_by_prop(T::get_instance())
            .unwrap();
        if index.lookup.is_none() {
            index.lookup = Some(HashMap::new());
        }
    }

    fn query_people<T: Query>(&self, q: T) -> Vec<PersonId> {
        T::setup(&q, self);
        let mut result = Vec::new();
        self.query_people_internal(
            |person| {
                result.push(person);
            },
            q.get_query(),
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
            q.get_query(),
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
            .contains(&TypeId::of::<T>())
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
            .insert(TypeId::of::<T>(), Methods::new::<T>());
        data_container
            .people_types
            .borrow_mut()
            .insert(T::name().to_string(), TypeId::of::<T>());
        data_container
            .registered_properties
            .borrow_mut()
            .insert(TypeId::of::<T>());

        self.register_indexer::<T>();
    }

    fn tabulate_person_properties<T: Tabulator, F>(&self, tabulator: &T, print_fn: F)
    where
        F: Fn(&Context, &[String], usize),
    {
        let type_ids = tabulator.get_typelist();
        tabulator.setup(self).unwrap();

        let data_container = self.get_data(PeoplePlugin);
        for type_id in &type_ids {
            let mut index = data_container.get_index_ref_mut(*type_id).unwrap();
            if index.lookup.is_none() {
                // Start indexing this property if it's not already
                // indexed.
                index.lookup = Some(HashMap::new());
            }

            let methods = data_container.get_methods(*type_id);
            index.index_unindexed_people(self, &methods);
        }

        let index_container = data_container.property_indexes.borrow();
        let indices = type_ids
            .iter()
            .filter_map(|t| index_container.get(t))
            .collect::<Vec<&Index>>();

        index::process_indices(
            self,
            indices.as_slice(),
            &mut Vec::new(),
            &HashSet::new(),
            &print_fn,
        );
    }

    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    fn sample_people<R: RngId + 'static, T: Query>(
        &self,
        rng_id: R,
        query: T,
        n: usize,
    ) -> Vec<PersonId>
    where
        R::RngType: Rng,
    {
        if self.get_current_population() == 0 {
            return Vec::new();
        }
        let requested = std::cmp::min(n, self.get_current_population());
        // Special case the empty query because we can do it in O(1).
        if query.get_query().is_empty() {
            let mut selected = HashSet::new();
            while selected.len() < requested {
                let result = self.sample_range(rng_id, 0..self.get_current_population());
                selected.insert(PersonId(result));
            }
            return selected.into_iter().collect();
        }

        T::setup(&query, self);

        // This function implements "Algorithm L" from KIM-HUNG LI
        // Reservoir-Sampling Algorithms of Time Complexity O(n(1 + log(N/n)))
        // https://dl.acm.org/doi/pdf/10.1145/198429.198435
        // Temporary variables.
        let mut w: f64 = self.sample_range(rng_id, 0.0..1.0);
        let mut ctr: usize = 0;
        let mut i: usize = 1;
        let mut selected = Vec::new();

        self.query_people_internal(
            |person| {
                ctr += 1;
                if i == ctr {
                    if selected.len() == requested {
                        let to_remove = self.sample_range(rng_id, 0..selected.len());
                        selected.swap_remove(to_remove);
                    }
                    selected.push(person);
                    if selected.len() == requested {
                        i += (f64::ln(self.sample_range(rng_id, 0.0..1.0)) / f64::ln(1.0 - w))
                            .floor() as usize
                            + 1;
                        w *= self.sample_range(rng_id, 0.0..1.0);
                    } else {
                        i += 1;
                    }
                }
            },
            query.get_query(),
        );

        selected
    }

    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    fn sample_person<R: RngId + 'static, T: Query>(&self, rng_id: R, query: T) -> Option<PersonId>
    where
        R::RngType: Rng,
    {
        let people = self.sample_people(rng_id, query, 1);
        if people.is_empty() {
            None
        } else {
            Some(people[0])
        }
    }
}

pub trait ContextPeopleExtInternal {
    fn register_indexer<T: PersonProperty>(&self);
    fn add_to_index_maybe<T: PersonProperty>(&mut self, person_id: PersonId, property: T);
    fn remove_from_index_maybe<T: PersonProperty>(&mut self, person_id: PersonId, property: T);
    fn query_people_internal(
        &self,
        accumulator: impl FnMut(PersonId),
        property_hashes: Vec<(TypeId, IndexValue)>,
    );
}

impl ContextPeopleExtInternal for Context {
    fn register_indexer<T: PersonProperty>(&self) {
        {
            let data_container = self.get_data(PeoplePlugin);

            let property_indexes = data_container.property_indexes.borrow_mut();
            if property_indexes.contains_key(&TypeId::of::<T>()) {
                return; // Index already exists, do nothing
            }
        }

        // If it doesn't exist, insert the new index
        let index = Index::new(self, T::get_instance());
        let data_container = self.get_data(PeoplePlugin);
        let mut property_indexes = data_container.property_indexes.borrow_mut();
        property_indexes.insert(TypeId::of::<T>(), index);
    }

    fn add_to_index_maybe<T: PersonProperty>(&mut self, person_id: PersonId, property: T) {
        let data_container = self.get_data(PeoplePlugin);

        if let Some(mut index) = data_container.get_index_ref_mut_by_prop(property) {
            let methods = data_container.get_methods(TypeId::of::<T>());
            if index.lookup.is_some() {
                index.add_person(self, &methods, person_id);
            }
        }
    }

    fn remove_from_index_maybe<T: PersonProperty>(&mut self, person_id: PersonId, property: T) {
        let data_container = self.get_data(PeoplePlugin);

        if let Some(mut index) = data_container.get_index_ref_mut_by_prop(property) {
            let methods = data_container.get_methods(TypeId::of::<T>());
            if index.lookup.is_some() {
                index.remove_person(self, &methods, person_id);
            }
        }
    }

    fn query_people_internal(
        &self,
        mut accumulator: impl FnMut(PersonId),
        property_hashes: Vec<(TypeId, IndexValue)>,
    ) {
        let mut indexes = Vec::<Ref<HashSet<PersonId>>>::new();
        let mut unindexed = Vec::<(TypeId, IndexValue)>::new();
        let data_container = self.get_data(PeoplePlugin);

        let mut property_hashs_working_set = property_hashes.clone();
        // intercept multi-property queries
        if property_hashs_working_set.len() > 1 {
            if let Some(combined_index) =
                get_and_register_multi_property_index(&property_hashes, self)
            {
                let combined_hash = get_multi_property_hash(&property_hashes);
                property_hashs_working_set = vec![(combined_index, combined_hash)];
            }
        }

        // 1. Walk through each property and update the indexes.
        for (t, _) in &property_hashs_working_set {
            let mut index = data_container.get_index_ref_mut(*t).unwrap();
            let methods = data_container.get_methods(*t);
            index.index_unindexed_people(self, &methods);
        }

        // 2. Collect the index entry corresponding to the value.
        for (t, hash) in property_hashs_working_set {
            let index = data_container.get_index_ref(t).unwrap();
            if let Ok(lookup) = Ref::filter_map(index, |x| x.lookup.as_ref()) {
                if let Ok(matching_people) =
                    Ref::filter_map(lookup, |x| x.get(&hash).map(|entry| &entry.1))
                {
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

        // 3. Create an iterator over people, based one either:
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
    use crate::people::{PeoplePlugin, PersonPropertyHolder};
    use crate::random::{define_rng, ContextRandomExt};
    use crate::{
        define_derived_property, define_global_property, define_person_property,
        define_person_property_with_default, Context, ContextGlobalPropertiesExt, ContextPeopleExt,
        IxaError, PersonId, PersonPropertyChangeEvent,
    };
    use serde_derive::Serialize;
    use std::any::TypeId;
    use std::cell::RefCell;
    use std::rc::Rc;

    define_person_property!(Age, u8);
    #[derive(Serialize, Copy, Clone, Debug, PartialEq, Eq)]
    pub enum AgeGroupValue {
        Child,
        Adult,
    }
    define_global_property!(ThresholdP, u8);
    define_derived_property!(IsEligible, bool, [Age], [ThresholdP], |age, threshold| {
        age >= threshold
    });

    // This isn't used, it's just testing for a compile error.
    define_derived_property!(
        NotUsed,
        bool,
        [Age],
        [ThresholdP, ThresholdP],
        |age, threshold, threshold2| { age >= threshold && age <= threshold2 }
    );

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
        let mut expected = vec![TypeId::of::<Age>(), TypeId::of::<IsRunner>()];
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
        let mut all_people = context.query_people(());

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
