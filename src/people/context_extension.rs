use crate::people::query::Query;
use crate::people::{InitializationList, PeoplePlugin};
use crate::{
    type_of, Context, ContextRandomExt, IxaError, PersonCreatedEvent, PersonId, PersonProperty,
    PersonPropertyChangeEvent, RngId, Tabulator,
};
use crate::{HashSet, HashSetExt};
use rand::Rng;

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

    fn property_is_indexed<T: PersonProperty>(&self) -> bool;

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
    fn tabulate_person_properties<T: Tabulator, F>(&mut self, tabulator: &T, print_fn: F)
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
}

impl ContextPeopleExt for Context {
    fn get_current_population(&self) -> usize {
        self.get_data_container(PeoplePlugin)
            .map_or(0, |data_container| data_container.current_population)
    }

    fn add_person<T: InitializationList>(&mut self, props: T) -> Result<PersonId, IxaError> {
        let data_container = self.get_data_container_mut(PeoplePlugin);
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
        let data_container = self.get_data_container_mut(PeoplePlugin);
        data_container.is_initializing = false;

        self.emit_event(PersonCreatedEvent { person_id });
        Ok(person_id)
    }

    fn get_person_property<T: PersonProperty>(
        &self,
        person_id: PersonId,
        _property: T,
    ) -> T::Value {
        T::register(self);

        // ToDo(Robert): This `unwrap` is temporary until the new `option` semantics for missing
        //               values can be propagated to the rest of the code. For now we assume
        //               every property sets an initial value if there is no value yet.
        T::compute(self, person_id).expect("Property not initialized when person created")
    }

    #[allow(clippy::single_match_else)]
    fn set_person_property<T: PersonProperty>(
        &mut self,
        person_id: PersonId,
        property: T,
        value: T::Value,
    ) {
        T::register(self);

        assert!(!T::is_derived(), "Cannot set a derived property");

        // This function can be called in two separate modes:
        //
        // 1. As a regular API function, in which case we want to
        //    emit an event and notify dependencies.
        // 2. Internally as part of initialization during add_person()
        //    in which case no events are emitted.
        //
        // Which mode it is determined by the data_container.is_initializing
        // property, which is set by add_person. This is complicated but
        // necessary because the initialization functions are called by
        // a per-PersonProperty closure generated by a macro and so are
        // outside the crate, but we don't want to expose a public
        // initialize_person_property() function.
        //
        // Temporarily remove dependency properties since we need mutable references
        // to self during callback execution
        let initializing = self
            .get_data_container(PeoplePlugin)
            .unwrap()
            .is_initializing;

        let (previous_value, deps_temp) = if initializing {
            (None, None)
        } else {
            let previous_value = self.get_person_property(person_id, property);
            if previous_value != value {
                self.remove_from_index_maybe(person_id, property);
            }

            (
                Some(previous_value),
                self.get_data_container(PeoplePlugin)
                    .unwrap()
                    .dependency_map
                    .borrow_mut()
                    .get_mut(&type_of::<T>())
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
            let data_container = self.get_data_container(PeoplePlugin).unwrap();
            let mut dependencies = data_container.dependency_map.borrow_mut();
            dependencies.insert(type_of::<T>(), deps);
        }

        // Update the main property and send a change event
        let data_container = self.get_data_container(PeoplePlugin).unwrap();
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
        {
            // Ensure PeopleData is initialized.
            let _ = self.get_data_container_mut(PeoplePlugin);
        }

        T::register(self);

        let data_container = self.get_data_container(PeoplePlugin).unwrap();
        data_container.index_property::<T>();
    }

    fn property_is_indexed<T: PersonProperty>(&self) -> bool {
        if let Some(data_container) = self.get_data_container(PeoplePlugin) {
            data_container.property_is_indexed::<T>()
        } else {
            false
        }
    }

    fn query_people<T: Query>(&self, query: T) -> Vec<PersonId> {
        // Special case the situation where nobody exists.
        if self.get_data_container(PeoplePlugin).is_none() {
            return Vec::new();
        }

        query.setup(self);

        let mut result = Vec::new();
        query.execute_query(self, |person| {
            result.push(person);
        });

        result
    }

    fn query_people_count<T: Query>(&self, query: T) -> usize {
        // Special case the situation where nobody exists.
        if self.get_data_container(PeoplePlugin).is_none() {
            return 0;
        }

        query.setup(self);

        let mut count: usize = 0;
        query.execute_query(self, |_person| {
            count += 1;
        });

        count
    }

    fn match_person<T: Query>(&self, person_id: PersonId, q: T) -> bool {
        q.match_entity(self, person_id)
    }

    fn tabulate_person_properties<T: Tabulator, F>(&mut self, tabulator: &T, print_fn: F)
    where
        F: Fn(&Context, &[String], usize),
    {
        tabulator.tabulate_person_properties(self, print_fn);
    }

    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    fn sample_person<R: RngId + 'static, T: Query>(&self, rng_id: R, query: T) -> Option<PersonId>
    where
        R::RngType: Rng,
    {
        if self.get_current_population() == 0 {
            return None;
        }

        // Special case the empty query because we can do it in O(1).
        if query.get_query().is_empty() {
            let result = self.sample_range(rng_id, 0..self.get_current_population());
            return Some(PersonId(result));
        }

        query.setup(self);

        // This function implements "Algorithm L" from KIM-HUNG LI
        // Reservoir-Sampling Algorithms of Time Complexity O(n(1 + log(N/n)))
        // https://dl.acm.org/doi/pdf/10.1145/198429.198435
        // Temporary variables.
        let mut selected: Option<PersonId> = None;
        let mut w: f64 = self.sample_range(rng_id, 0.0..1.0);
        let mut ctr: usize = 0;
        let mut i: usize = 1;

        query.execute_query(self, |person| {
            ctr += 1;
            if i == ctr {
                selected = Some(person);
                i += (f64::ln(self.sample_range(rng_id, 0.0..1.0)) / f64::ln(1.0 - w)).floor()
                    as usize
                    + 1;
                w *= self.sample_range(rng_id, 0.0..1.0);
            }
        });

        selected
    }
}

pub trait ContextPeopleExtInternal {
    /// Reports whether the property has already been registered for this context. Creates the
    /// `PeopleDataContainer` if it doesn't exist.
    fn is_registered<T: PersonProperty>(&self) -> bool;
    /// Registers the type with all of its dependencies and then registers an index for the type.
    fn register_derived_property<T: PersonProperty>(&self);
    fn register_nonderived_property<T: PersonProperty>(&self);
    fn add_to_index_maybe<T: PersonProperty>(&self, person_id: PersonId, property: T);
    fn remove_from_index_maybe<T: PersonProperty>(&self, person_id: PersonId, property: T);
}

impl ContextPeopleExtInternal for Context {
    fn is_registered<T: PersonProperty>(&self) -> bool {
        // let data_container = self.get_data_container_mut(PeoplePlugin);
        let data_container = self.get_data_container(PeoplePlugin).unwrap();
        data_container
            .registered_derived_properties
            .borrow_mut()
            .contains(&type_of::<T>())
    }

    /// Registers the type with all of its dependencies and then registers an index for the type.
    fn register_derived_property<T: PersonProperty>(&self) {
        let entity_data = self.get_data_container(PeoplePlugin).unwrap();
        let type_id = type_of::<T>();

        // This method should only be called during initial Property registration.
        assert!(!entity_data.property_indexes.borrow().contains_key(&type_id));

        {
            // Scope of `derived_prop_map`, a mutable borrow
            let mut dependencies = HashSet::new();
            T::collect_dependencies(&mut dependencies);
            let mut derived_prop_map = entity_data.dependency_map.borrow_mut();
            for dependency in dependencies {
                derived_prop_map
                    .entry(dependency)
                    .or_default()
                    .push(Box::new(T::get_instance()));
            }
        }
        // Also do everything that needs to be done for nonderived properties
        self.register_nonderived_property::<T>();
    }

    fn register_nonderived_property<T: PersonProperty>(&self) {
        let data_container = self.get_data_container(PeoplePlugin).unwrap();
        data_container.register_nonderived_property::<T>();
    }

    fn add_to_index_maybe<T: PersonProperty>(&self, person_id: PersonId, property: T) {
        self.get_data_container(PeoplePlugin)
            .inspect(|data_container| {
                data_container.add_to_index_maybe(self, person_id, property);
            });
    }

    fn remove_from_index_maybe<T: PersonProperty>(&self, person_id: PersonId, property: T) {
        self.get_data_container(PeoplePlugin)
            .inspect(|data_container| {
                data_container.remove_from_index_maybe(self, person_id, property);
            });
    }
}

#[cfg(test)]
mod tests {
    use crate::people::PeoplePlugin;
    use crate::random::{define_rng, ContextRandomExt};
    use crate::{
        define_derived_property, define_global_property, define_person_property,
        define_person_property_with_default, Context, ContextGlobalPropertiesExt, ContextPeopleExt,
        HashSet, IxaError, PersonId, PersonProperty, PersonPropertyChangeEvent,
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
        let people_data = context.get_data_container_mut(PeoplePlugin);

        // Verify we haven't initialized the property yet
        let indexes = people_data.property_indexes.get_mut();
        assert!(indexes.get_container_ref::<RunningShoes>().is_none());

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
        let mut actual = HashSet::default();
        SeniorRunner::collect_dependencies(&mut actual);
        let mut actual: Vec<TypeId> = actual.into_iter().collect();
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
    fn test_sample_person_simple() {
        define_rng!(SampleRng1);
        let mut context = Context::new();
        context.init_random(42);
        assert!(context.sample_person(SampleRng1, ()).is_none());
        let person = context.add_person(()).unwrap();
        assert_eq!(context.sample_person(SampleRng1, ()).unwrap(), person);
    }

    #[test]
    fn test_sample_matching_person() {
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

        // See that the simple query always returns person3
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
