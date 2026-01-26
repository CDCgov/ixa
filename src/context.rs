//! A manager for the state of a discrete-event simulation
//!
//! Defines a `Context` that is intended to provide the foundational mechanism
//! for storing and manipulating the state of a given simulation.
use std::any::{Any, TypeId};
use std::cell::OnceCell;
use std::collections::VecDeque;
use std::fmt::{Display, Formatter};
use std::rc::Rc;

use crate::data_plugin::DataPlugin;
use crate::entity::entity_store::EntityStore;
use crate::entity::events::{
    EntityCreatedEvent, PartialPropertyChangeEvent, PartialPropertyChangeEventCore,
};
use crate::entity::property::{Property, PropertyInitializationKind};
use crate::entity::property_list::PropertyList;
use crate::entity::property_value_store_core::PropertyValueStoreCore;
use crate::entity::query::QueryResultIterator;
use crate::entity::{Entity, EntityId, EntityIterator, Query};
use crate::execution_stats::{
    log_execution_statistics, print_execution_statistics, ExecutionProfilingCollector,
    ExecutionStatistics,
};
use crate::plan::{PlanId, Queue};
#[cfg(feature = "progress_bar")]
use crate::progress::update_timeline_progress;
use crate::rand::Rng;
#[cfg(feature = "debugger")]
use crate::{debugger::enter_debugger, plan::PlanSchedule};
use crate::{
    get_data_plugin_count, trace, warn, ContextPeopleExt, ContextRandomExt, HashMap, HashMapExt,
    HashSet, RngId,
};

/// The common callback used by multiple [`Context`] methods for future events
type Callback = dyn FnOnce(&mut Context);

/// A handler for an event type `E`
type EventHandler<E> = dyn Fn(&mut Context, E);

pub trait IxaEvent {
    /// Called every time `context.subscribe_to_event` is called with this event
    fn on_subscribe(_context: &mut Context) {}
}

/// An enum to indicate the phase for plans at a given time.
///
/// Most plans will occur as `Normal`. Plans with phase `First` are
/// handled before all `Normal` plans, and those with phase `Last` are
/// handled after all `Normal` plans. In all cases ties between plans at the
/// same time and with the same phase are handled in the order of scheduling.
///
#[derive(PartialEq, Eq, Ord, Clone, Copy, PartialOrd, Hash, Debug)]
pub enum ExecutionPhase {
    First,
    Normal,
    Last,
}

impl Display for ExecutionPhase {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// A manager for the state of a discrete-event simulation
///
/// Provides core simulation services including
/// * Maintaining a notion of time
/// * Scheduling events to occur at some point in the future and executing them
///   at that time
/// * Holding data that can be accessed by simulation modules
///
/// Simulations are constructed out of a series of interacting modules that
/// take turns manipulating the [`Context`] through a mutable reference. Modules
/// store data in the simulation using the [`DataPlugin`] trait that allows them
/// to retrieve data by type.
///
/// The future event list of the simulation is a queue of `Callback` objects -
/// called `plans` - that will assume control of the [`Context`] at a future point
/// in time and execute the logic in the associated `FnOnce(&mut Context)`
/// closure. Modules can add plans to this queue through the [`Context`].
///
/// The simulation also has a separate callback mechanism. Callbacks
/// fire before the next timed event (even if it is scheduled for the
/// current time). This allows modules to schedule actions for immediate
/// execution but outside of the current iteration of the event loop.
///
/// Modules can also emit 'events' that other modules can subscribe to handle by
/// event type. This allows modules to broadcast that specific things have
/// occurred and have other modules take turns reacting to these occurrences.
///
pub struct Context {
    plan_queue: Queue<Box<Callback>, ExecutionPhase>,
    callback_queue: VecDeque<Box<Callback>>,
    event_handlers: HashMap<TypeId, Box<dyn Any>>,
    pub(crate) entity_store: EntityStore,
    data_plugins: Vec<OnceCell<Box<dyn Any>>>,
    #[cfg(feature = "debugger")]
    breakpoints_scheduled: Queue<Box<Callback>, ExecutionPhase>,
    current_time: f64,
    shutdown_requested: bool,
    #[cfg(feature = "debugger")]
    break_requested: bool,
    #[cfg(feature = "debugger")]
    breakpoints_enabled: bool,
    execution_profiler: ExecutionProfilingCollector,
    pub print_execution_statistics: bool,
}

impl Context {
    /// Create a new empty `Context`
    #[must_use]
    pub fn new() -> Context {
        // Create a vector to accommodate all registered data plugins
        let data_plugins = std::iter::repeat_with(OnceCell::new)
            .take(get_data_plugin_count())
            .collect();

        Context {
            plan_queue: Queue::new(),
            callback_queue: VecDeque::new(),
            event_handlers: HashMap::new(),
            entity_store: EntityStore::new(),
            data_plugins,
            #[cfg(feature = "debugger")]
            breakpoints_scheduled: Queue::new(),
            current_time: 0.0,
            shutdown_requested: false,
            #[cfg(feature = "debugger")]
            break_requested: false,
            #[cfg(feature = "debugger")]
            breakpoints_enabled: true,
            execution_profiler: ExecutionProfilingCollector::new(),
            print_execution_statistics: false,
        }
    }

    pub fn add_entity<E: Entity, PL: PropertyList<E>>(
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

    pub fn get_property<E: Entity, P: Property<E>>(&self, entity_id: EntityId<E>) -> P {
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

    /// Sets the value of the given property. This method unconditionally emits a `PropertyChangeEvent`.
    pub fn set_property<E: Entity, P: Property<E>>(
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

    /// Enables indexing of property values for the property `P`.
    ///
    /// This method is called with the turbo-fish syntax:
    ///     `context.index_property::<Person, Age>()`
    /// The actual computation of the index is done lazily as needed upon execution of queries,
    /// not when this method is called.
    pub fn index_property<E: Entity, P: Property<E>>(&mut self) {
        let property_store = self.entity_store.get_property_store_mut::<E>();
        property_store.set_property_indexed::<P>(true);
    }

    /// Checks if a property `P` is indexed.
    ///
    /// This method is called with the turbo-fish syntax:
    ///     `context.index_property::<Person, Age>()`
    ///
    /// This method can return `true` even if `context.index_property::<P>()` has never been called. For example,
    /// if a multi-property is indexed, all equivalent multi-properties are automatically also indexed, as they
    /// share a single index.
    #[cfg(test)]
    pub fn is_property_indexed<E: Entity, P: Property<E>>(&self) -> bool {
        let property_store = self.entity_store.get_property_store::<E>();
        property_store.is_property_indexed::<P>()
    }
    pub(crate) fn get_property_value_store<E: Entity, P: Property<E>>(
        &self,
    ) -> &PropertyValueStoreCore<E, P> {
        self.entity_store.get_property_store::<E>().get::<P>()
    }

    pub(crate) fn get_property_value_store_mut<E: Entity, P: Property<E>>(
        &mut self,
    ) -> &mut PropertyValueStoreCore<E, P> {
        self.entity_store
            .get_property_store_mut::<E>()
            .get_mut::<P>()
    }

    /// This method gives client code direct immutable access to the fully realized set of
    /// entity IDs. This is especially efficient for indexed queries, as this method reduces
    /// to a simple lookup of a hash bucket. Otherwise, the set is allocated and computed.
    pub fn with_query_results<E: Entity, Q: Query<E>>(
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

    pub fn query_entity_count<E: Entity, Q: Query<E>>(&self, query: Q) -> usize {
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

    /// Sample a single entity uniformly from the query results. Returns `None` if the
    /// query's result set is empty.
    ///
    /// To sample from the entire population, pass in the empty query `()`.
    pub fn sample_entity<R, E, Q>(&self, rng_id: R, query: Q) -> Option<EntityId<E>>
    where
        R: RngId + 'static,
        R::RngType: Rng,
        E: Entity,
        Q: Query<E>,
    {
        let query_result = self.query_result_iterator(query);
        self.sample(rng_id, move |rng| query_result.sample_entity(rng))
    }

    /// Sample up to `requested` entities uniformly from the query results. If the
    /// query's result set has fewer than `requested` entities, the entire result
    /// set is returned.
    ///
    /// To sample from the entire population, pass in the empty query `()`.
    pub fn sample_entities<R, E, Q>(&self, rng_id: R, query: Q, n: usize) -> Vec<EntityId<E>>
    where
        R: RngId + 'static,
        R::RngType: Rng,
        E: Entity,
        Q: Query<E>,
    {
        let query_result = self.query_result_iterator(query);
        self.sample(rng_id, move |rng| query_result.sample_entities(rng, n))
    }

    /// Returns a total count of all created entities of type `E`.
    pub fn get_entity_count<E: Entity>(&self) -> usize {
        self.entity_store.get_entity_count::<E>()
    }

    /// Returns an iterator over all created entities of type `E`.
    pub fn get_entity_iterator<E: Entity>(&self) -> EntityIterator<E> {
        self.entity_store.get_entity_iterator::<E>()
    }

    /// Generates an iterator over the results of the query.
    pub fn query_result_iterator<E: Entity, Q: Query<E>>(
        &self,
        query: Q,
    ) -> QueryResultIterator<E> {
        query.new_query_result_iterator(self)
    }

    /// Schedule the simulation to pause at time t and start the debugger.
    /// This will give you a REPL which allows you to inspect the state of
    /// the simulation (type help to see a list of commands)
    ///
    /// # Errors
    /// Internal debugger errors e.g., reading or writing to stdin/stdout;
    /// errors in Ixa are printed to stdout
    #[cfg(feature = "debugger")]
    pub fn schedule_debugger(
        &mut self,
        time: f64,
        priority: Option<ExecutionPhase>,
        callback: Box<Callback>,
    ) {
        trace!("scheduling debugger");
        let priority = priority.unwrap_or(ExecutionPhase::First);
        self.breakpoints_scheduled
            .add_plan(time, callback, priority);
    }

    /// Register to handle emission of events of type E
    ///
    /// Handlers will be called upon event emission in order of subscription as
    /// queued `Callback`s with the appropriate event.
    #[allow(clippy::missing_panics_doc)]
    pub fn subscribe_to_event<E: IxaEvent + Copy + 'static>(
        &mut self,
        handler: impl Fn(&mut Context, E) + 'static,
    ) {
        let handler_vec = self
            .event_handlers
            .entry(TypeId::of::<E>())
            .or_insert_with(|| Box::<Vec<Rc<EventHandler<E>>>>::default());
        let handler_vec: &mut Vec<Rc<EventHandler<E>>> = handler_vec.downcast_mut().unwrap();
        handler_vec.push(Rc::new(handler));
        E::on_subscribe(self);
    }

    /// Emit an event of type E to be handled by registered receivers
    ///
    /// Receivers will handle events in the order that they have subscribed and
    /// are queued as callbacks
    #[allow(clippy::missing_panics_doc)]
    pub fn emit_event<E: IxaEvent + Copy + 'static>(&mut self, event: E) {
        // Destructure to obtain event handlers and plan queue
        let Context {
            event_handlers,
            callback_queue,
            ..
        } = self;
        if let Some(handler_vec) = event_handlers.get(&TypeId::of::<E>()) {
            let handler_vec: &Vec<Rc<EventHandler<E>>> = handler_vec.downcast_ref().unwrap();
            for handler in handler_vec {
                let handler_clone = Rc::clone(handler);
                callback_queue.push_back(Box::new(move |context| handler_clone(context, event)));
            }
        }
    }

    /// Add a plan to the future event list at the specified time in the normal
    /// phase
    ///
    /// Returns a [`PlanId`] for the newly-added plan that can be used to cancel it
    /// if needed.
    /// # Panics
    ///
    /// Panics if time is in the past, infinite, or NaN.
    pub fn add_plan(&mut self, time: f64, callback: impl FnOnce(&mut Context) + 'static) -> PlanId {
        self.add_plan_with_phase(time, callback, ExecutionPhase::Normal)
    }

    /// Add a plan to the future event list at the specified time and with the
    /// specified phase (first, normal, or last among plans at the
    /// specified time)
    ///
    /// Returns a [`PlanId`] for the newly-added plan that can be used to cancel it
    /// if needed.
    /// # Panics
    ///
    /// Panics if time is in the past, infinite, or NaN.
    pub fn add_plan_with_phase(
        &mut self,
        time: f64,
        callback: impl FnOnce(&mut Context) + 'static,
        phase: ExecutionPhase,
    ) -> PlanId {
        assert!(
            !time.is_nan() && !time.is_infinite() && time >= self.current_time,
            "Time is invalid"
        );
        self.plan_queue.add_plan(time, Box::new(callback), phase)
    }

    fn evaluate_periodic_and_schedule_next(
        &mut self,
        period: f64,
        callback: impl Fn(&mut Context) + 'static,
        phase: ExecutionPhase,
    ) {
        trace!(
            "evaluate periodic at {} (period={})",
            self.current_time,
            period
        );
        callback(self);
        if !self.plan_queue.is_empty() {
            let next_time = self.current_time + period;
            self.add_plan_with_phase(
                next_time,
                move |context| context.evaluate_periodic_and_schedule_next(period, callback, phase),
                phase,
            );
        }
    }

    /// Add a plan with specified priority to the future event list, and
    /// continuously repeat the plan at the specified period, stopping
    /// only once there are no other plans scheduled.
    ///
    /// # Panics
    ///
    /// Panics if plan period is negative, infinite, or NaN.
    pub fn add_periodic_plan_with_phase(
        &mut self,
        period: f64,
        callback: impl Fn(&mut Context) + 'static,
        phase: ExecutionPhase,
    ) {
        assert!(
            period > 0.0 && !period.is_nan() && !period.is_infinite(),
            "Period must be greater than 0"
        );

        self.add_plan_with_phase(
            0.0,
            move |context| context.evaluate_periodic_and_schedule_next(period, callback, phase),
            phase,
        );
    }

    /// Cancel a plan that has been added to the queue
    ///
    /// # Panics
    ///
    /// This function panics if you cancel a plan which has already been
    /// cancelled or executed.
    pub fn cancel_plan(&mut self, plan_id: &PlanId) {
        trace!("canceling plan {plan_id:?}");
        let result = self.plan_queue.cancel_plan(plan_id);
        if result.is_none() {
            warn!("Tried to cancel nonexistent plan with ID = {plan_id:?}");
        }
    }

    #[doc(hidden)]
    #[allow(dead_code)]
    pub(crate) fn remaining_plan_count(&self) -> usize {
        self.plan_queue.remaining_plan_count()
    }

    /// Add a `Callback` to the queue to be executed before the next plan
    pub fn queue_callback(&mut self, callback: impl FnOnce(&mut Context) + 'static) {
        trace!("queuing callback");
        self.callback_queue.push_back(Box::new(callback));
    }

    /// Retrieve a mutable reference to the data container associated with a
    /// [`DataPlugin`]
    ///
    /// If the data container has not been already added to the [`Context`] then
    /// this function will use the [`DataPlugin::init`] method
    /// to construct a new data container and store it in the [`Context`].
    ///
    /// Returns a mutable reference to the data container
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn get_data_mut<T: DataPlugin>(&mut self, _data_plugin: T) -> &mut T::DataContainer {
        let index = T::index_within_context();

        // If the data plugin is already initialized, return a mutable reference.
        if self.data_plugins[index].get().is_some() {
            return self.data_plugins[index]
                .get_mut()
                .unwrap()
                .downcast_mut::<T::DataContainer>()
                .expect("TypeID does not match data plugin type");
        }

        // Initialize the data plugin if not already initialized.
        let data = T::init(self);
        let cell = self
            .data_plugins
            .get_mut(index)
            .unwrap_or_else(|| panic!("No data plugin found with index = {index:?}. You must use the `define_data_plugin!` macro to create a data plugin."));
        let _ = cell.set(Box::new(data));
        cell.get_mut()
            .unwrap()
            .downcast_mut::<T::DataContainer>()
            .expect("TypeID does not match data plugin type. You must use the `define_data_plugin!` macro to create a data plugin.")
    }

    /// Retrieve a reference to the data container associated with a
    /// [`DataPlugin`]
    ///
    /// Returns a reference to the data container if it exists or else `None`
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn get_data<T: DataPlugin>(&self, _data_plugin: T) -> &T::DataContainer {
        let index = T::index_within_context();
        self.data_plugins
            .get(index)
            .unwrap_or_else(|| panic!("No data plugin found with index = {index:?}. You must use the `define_data_plugin!` macro to create a data plugin."))
            .get_or_init(|| Box::new(T::init(self)))
            .downcast_ref::<T::DataContainer>()
            .expect("TypeID does not match data plugin type. You must use the `define_data_plugin!` macro to create a data plugin.")
    }

    /// Shutdown the simulation cleanly, abandoning all events after whatever
    /// is currently executing.
    pub fn shutdown(&mut self) {
        trace!("shutdown context");
        self.shutdown_requested = true;
    }

    /// Get the current time in the simulation
    ///
    /// Returns the current time
    #[must_use]
    pub fn get_current_time(&self) -> f64 {
        self.current_time
    }

    /// Request to enter a debugger session at next event loop
    #[cfg(feature = "debugger")]
    pub fn request_debugger(&mut self) {
        self.break_requested = true;
    }

    /// Request to enter a debugger session at next event loop
    #[cfg(feature = "debugger")]
    pub fn cancel_debugger_request(&mut self) {
        self.break_requested = false;
    }

    /// Disable breakpoints
    #[cfg(feature = "debugger")]
    pub fn disable_breakpoints(&mut self) {
        self.breakpoints_enabled = false;
    }

    /// Enable breakpoints
    #[cfg(feature = "debugger")]
    pub fn enable_breakpoints(&mut self) {
        self.breakpoints_enabled = true;
    }

    /// Returns `true` if breakpoints are enabled.
    #[must_use]
    #[cfg(feature = "debugger")]
    pub fn breakpoints_are_enabled(&self) -> bool {
        self.breakpoints_enabled
    }

    /// Delete the breakpoint with the given ID
    #[cfg(feature = "debugger")]
    pub fn delete_breakpoint(&mut self, breakpoint_id: u64) -> Option<Box<Callback>> {
        self.breakpoints_scheduled
            .cancel_plan(&PlanId(breakpoint_id))
    }

    /// Returns a list of length `at_most`, or unbounded if `at_most=0`, of active scheduled
    /// `PlanSchedule`s ordered as they are in the queue itself.
    #[must_use]
    #[cfg(feature = "debugger")]
    pub fn list_breakpoints(&self, at_most: usize) -> Vec<&PlanSchedule<ExecutionPhase>> {
        self.breakpoints_scheduled.list_schedules(at_most)
    }

    /// Deletes all breakpoints.
    #[cfg(feature = "debugger")]
    pub fn clear_breakpoints(&mut self) {
        self.breakpoints_scheduled.clear();
    }

    /// Execute the simulation until the plan and callback queues are empty
    pub fn execute(&mut self) {
        trace!("entering event loop");
        // Start plan loop
        loop {
            #[cfg(feature = "progress_bar")]
            if crate::progress::MAX_TIME.get().is_some() {
                update_timeline_progress(self.current_time);
            }

            #[cfg(feature = "debugger")]
            if self.break_requested {
                enter_debugger(self);
            } else if self.shutdown_requested {
                self.shutdown_requested = false;
                break;
            } else {
                self.execute_single_step();
            }

            self.execution_profiler.refresh();

            #[cfg(not(feature = "debugger"))]
            if self.shutdown_requested {
                self.shutdown_requested = false;
                break;
            } else {
                self.execute_single_step();
            }
        }

        let stats = self.get_execution_statistics();
        if self.print_execution_statistics {
            print_execution_statistics(&stats);
        } else {
            log_execution_statistics(&stats);
        }
    }

    /// Executes a single step of the simulation, prioritizing tasks as follows:
    ///   1. Breakpoints
    ///   2. Callbacks
    ///   3. Plans
    ///   4. Shutdown
    pub fn execute_single_step(&mut self) {
        // This always runs the breakpoint before anything scheduled in the task queue regardless
        // of the `ExecutionPhase` of the breakpoint. If breakpoints are disabled, they are still
        // popped from the breakpoint queue at the time they are scheduled even though they are not
        // executed.
        #[cfg(feature = "debugger")]
        if let Some((bp, _)) = self.breakpoints_scheduled.peek() {
            // If the priority of bp is `ExecutionPhase::First`, and if the next scheduled plan
            // is scheduled at or after bp's time (or doesn't exist), run bp.
            // If the priority of bp is `ExecutionPhase::Last`, and if the next scheduled plan
            // is scheduled strictly after bp's time (or doesn't exist), run bp.
            if let Some(plan_time) = self.plan_queue.next_time() {
                if (bp.priority == ExecutionPhase::First && bp.time <= plan_time)
                    || (bp.priority == ExecutionPhase::Last && bp.time < plan_time)
                {
                    self.breakpoints_scheduled.get_next_plan(); // Pop the breakpoint
                    if self.breakpoints_enabled {
                        self.break_requested = true;
                        return;
                    }
                }
            } else {
                self.breakpoints_scheduled.get_next_plan(); // Pop the breakpoint
                if self.breakpoints_enabled {
                    self.break_requested = true;
                    return;
                }
            }
        }

        // If there is a callback, run it.
        if let Some(callback) = self.callback_queue.pop_front() {
            trace!("calling callback");
            callback(self);
        }
        // There aren't any callbacks, so look at the first plan.
        else if let Some(plan) = self.plan_queue.get_next_plan() {
            trace!("calling plan at {:.6}", plan.time);
            self.current_time = plan.time;
            (plan.data)(self);
        } else {
            trace!("No callbacks or plans; exiting event loop");
            // OK, there aren't any plans, so we're done.
            self.shutdown_requested = true;
        }
    }

    pub fn get_execution_statistics(&mut self) -> ExecutionStatistics {
        let population = self.get_current_population();
        self.execution_profiler.compute_final_statistics(population)
    }
}

pub trait ContextBase: Sized {
    fn subscribe_to_event<E: IxaEvent + Copy + 'static>(
        &mut self,
        handler: impl Fn(&mut Context, E) + 'static,
    );
    fn emit_event<E: IxaEvent + Copy + 'static>(&mut self, event: E);
    fn add_plan(&mut self, time: f64, callback: impl FnOnce(&mut Context) + 'static) -> PlanId;
    fn add_plan_with_phase(
        &mut self,
        time: f64,
        callback: impl FnOnce(&mut Context) + 'static,
        phase: ExecutionPhase,
    ) -> PlanId;
    fn add_periodic_plan_with_phase(
        &mut self,
        period: f64,
        callback: impl Fn(&mut Context) + 'static,
        phase: ExecutionPhase,
    );
    fn cancel_plan(&mut self, plan_id: &PlanId);
    fn queue_callback(&mut self, callback: impl FnOnce(&mut Context) + 'static);
    fn get_data_mut<T: DataPlugin>(&mut self, plugin: T) -> &mut T::DataContainer;
    fn get_data<T: DataPlugin>(&self, plugin: T) -> &T::DataContainer;
    fn get_current_time(&self) -> f64;
    fn get_execution_statistics(&mut self) -> ExecutionStatistics;
    fn add_entity<E: Entity, PL: PropertyList<E>>(&mut self, property_list: PL) -> Result<EntityId<E>, String>;
    fn get_property<E: Entity, P: Property<E>>(&self, entity_id: EntityId<E>) -> P;
    fn set_property<E: Entity, P: Property<E>>(&mut self, entity_id: EntityId<E>, property_value: P);
    fn sample_entity<R, E, Q>(&self, rng_id: R, query: Q) -> Option<EntityId<E>>
    where
        R: RngId + 'static,
        R::RngType: Rng,
        E: Entity,
        Q: Query<E>;
    fn sample_entities<R, E, Q>(&self, rng_id: R, query: Q, n: usize) -> Vec<EntityId<E>>
    where
        R: RngId + 'static,
        R::RngType: Rng,
        E: Entity,
        Q: Query<E>;
    fn get_entity_count<E: Entity>(&self) -> usize;
    fn get_entity_iterator<E: Entity>(&self) -> EntityIterator<E>;
}
impl ContextBase for Context {
    delegate::delegate! {
        to self {
            fn subscribe_to_event<E: IxaEvent + Copy + 'static>(&mut self, handler: impl Fn(&mut Context, E) + 'static);
            fn emit_event<E: IxaEvent + Copy + 'static>(&mut self, event: E);
            fn add_plan(&mut self, time: f64, callback: impl FnOnce(&mut Context) + 'static) -> PlanId;
            fn add_plan_with_phase(&mut self, time: f64, callback: impl FnOnce(&mut Context) + 'static, phase: ExecutionPhase) -> PlanId;
            fn add_periodic_plan_with_phase(&mut self, period: f64, callback: impl Fn(&mut Context) + 'static, phase: ExecutionPhase);
            fn cancel_plan(&mut self, plan_id: &PlanId);
            fn queue_callback(&mut self, callback: impl FnOnce(&mut Context) + 'static);
            fn get_data_mut<T: DataPlugin>(&mut self, plugin: T) -> &mut T::DataContainer;
            fn get_data<T: DataPlugin>(&self, plugin: T) -> &T::DataContainer;
            fn get_current_time(&self) -> f64;
            fn get_execution_statistics(&mut self) -> ExecutionStatistics;
            fn add_entity<E: Entity, PL: PropertyList<E>>(&mut self, property_list: PL) -> Result<EntityId<E>, String>;
            fn get_property<E: Entity, P: Property<E>>(&self, entity_id: EntityId<E>) -> P;
            fn set_property<E: Entity, P: Property<E>>(&mut self, entity_id: EntityId<E>, property_value: P);
            fn sample_entity<R, E, Q>(&self, rng_id: R, query: Q) -> Option<EntityId<E>>
            where
                R: RngId + 'static,
                R::RngType: Rng,
                E: Entity,
                Q: Query<E>;
            fn sample_entities<R, E, Q>(&self, rng_id: R, query: Q, n: usize) -> Vec<EntityId<E>>
            where
                R: RngId + 'static,
                R::RngType: Rng,
                E: Entity,
                Q: Query<E>;
            fn get_entity_count<E: Entity>(&self) -> usize;
            fn get_entity_iterator<E: Entity>(&self) -> EntityIterator<E>;
    }
}
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    #![allow(dead_code)]
    use std::cell::{Ref, RefCell};

    use ixa_derive::IxaEvent;

    use super::*;
    use crate::{
        define_data_plugin, define_entity, define_multi_property, define_property, HashSet,
    };

    define_data_plugin!(ComponentA, Vec<u32>, vec![]);

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

    #[test]
    fn empty_context() {
        let mut context = Context::new();
        context.execute();
        assert_eq!(context.get_current_time(), 0.0);
    }

    #[test]
    fn get_data() {
        let mut context = Context::new();
        context.get_data_mut(ComponentA).push(1);
        assert_eq!(*context.get_data(ComponentA), vec![1],);
    }

    fn add_plan(context: &mut Context, time: f64, value: u32) -> PlanId {
        context.add_plan(time, move |context| {
            context.get_data_mut(ComponentA).push(value);
        })
    }

    fn add_plan_with_phase(
        context: &mut Context,
        time: f64,
        value: u32,
        phase: ExecutionPhase,
    ) -> PlanId {
        context.add_plan_with_phase(
            time,
            move |context| {
                context.get_data_mut(ComponentA).push(value);
            },
            phase,
        )
    }

    #[test]
    #[should_panic(expected = "Time is invalid")]
    fn negative_plan_time() {
        let mut context = Context::new();
        add_plan(&mut context, -1.0, 0);
    }

    #[test]
    #[should_panic(expected = "Time is invalid")]
    fn infinite_plan_time() {
        let mut context = Context::new();
        add_plan(&mut context, f64::INFINITY, 0);
    }

    #[test]
    #[should_panic(expected = "Time is invalid")]
    fn nan_plan_time() {
        let mut context = Context::new();
        add_plan(&mut context, f64::NAN, 0);
    }

    #[test]
    fn timed_plan_only() {
        let mut context = Context::new();
        add_plan(&mut context, 1.0, 1);
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);
    }

    #[test]
    fn callback_only() {
        let mut context = Context::new();
        context.queue_callback(|context| {
            context.get_data_mut(ComponentA).push(1);
        });
        context.execute();
        assert_eq!(context.get_current_time(), 0.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);
    }

    #[test]
    fn callback_before_timed_plan() {
        let mut context = Context::new();
        context.queue_callback(|context| {
            context.get_data_mut(ComponentA).push(1);
        });
        add_plan(&mut context, 1.0, 2);
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2]);
    }

    #[test]
    fn callback_adds_timed_plan() {
        let mut context = Context::new();
        context.queue_callback(|context| {
            context.get_data_mut(ComponentA).push(1);
            add_plan(context, 1.0, 2);
            context.get_data_mut(ComponentA).push(3);
        });
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 3, 2]);
    }

    #[test]
    fn callback_adds_callback_and_timed_plan() {
        let mut context = Context::new();
        context.queue_callback(|context| {
            context.get_data_mut(ComponentA).push(1);
            add_plan(context, 1.0, 2);
            context.queue_callback(|context| {
                context.get_data_mut(ComponentA).push(4);
            });
            context.get_data_mut(ComponentA).push(3);
        });
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 3, 4, 2]);
    }

    #[test]
    fn timed_plan_adds_callback_and_timed_plan() {
        let mut context = Context::new();
        context.add_plan(1.0, |context| {
            context.get_data_mut(ComponentA).push(1);
            // We add the plan first, but the callback will fire first.
            add_plan(context, 2.0, 3);
            context.queue_callback(|context| {
                context.get_data_mut(ComponentA).push(2);
            });
        });
        context.execute();
        assert_eq!(context.get_current_time(), 2.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2, 3]);
    }

    #[test]
    fn cancel_plan() {
        let mut context = Context::new();
        let to_cancel = add_plan(&mut context, 2.0, 1);
        context.add_plan(1.0, move |context| {
            context.cancel_plan(&to_cancel);
        });
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        let test_vec: Vec<u32> = vec![];
        assert_eq!(*context.get_data_mut(ComponentA), test_vec);
    }

    #[test]
    fn add_plan_with_current_time() {
        let mut context = Context::new();
        context.add_plan(1.0, move |context| {
            context.get_data_mut(ComponentA).push(1);
            add_plan(context, 1.0, 2);
            context.queue_callback(|context| {
                context.get_data_mut(ComponentA).push(3);
            });
        });
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 3, 2]);
    }

    #[test]
    fn plans_at_same_time_fire_in_order() {
        let mut context = Context::new();
        add_plan(&mut context, 1.0, 1);
        add_plan(&mut context, 1.0, 2);
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2]);
    }

    #[test]
    fn check_plan_phase_ordering() {
        assert!(ExecutionPhase::First < ExecutionPhase::Normal);
        assert!(ExecutionPhase::Normal < ExecutionPhase::Last);
    }

    #[test]
    fn plans_at_same_time_follow_phase() {
        let mut context = Context::new();
        add_plan(&mut context, 1.0, 1);
        add_plan_with_phase(&mut context, 1.0, 5, ExecutionPhase::Last);
        add_plan_with_phase(&mut context, 1.0, 3, ExecutionPhase::First);
        add_plan(&mut context, 1.0, 2);
        add_plan_with_phase(&mut context, 1.0, 6, ExecutionPhase::Last);
        add_plan_with_phase(&mut context, 1.0, 4, ExecutionPhase::First);
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![3, 4, 1, 2, 5, 6]);
    }

    #[derive(Copy, Clone, IxaEvent)]
    struct Event1 {
        pub data: usize,
    }

    #[derive(Copy, Clone, IxaEvent)]
    struct Event2 {
        pub data: usize,
    }

    #[test]
    fn simple_event() {
        let mut context = Context::new();
        let obs_data = Rc::new(RefCell::new(0));
        let obs_data_clone = Rc::clone(&obs_data);

        context.subscribe_to_event::<Event1>(move |_, event| {
            *obs_data_clone.borrow_mut() = event.data;
        });

        context.emit_event(Event1 { data: 1 });
        context.execute();
        assert_eq!(*obs_data.borrow(), 1);
    }

    #[test]
    fn multiple_events() {
        let mut context = Context::new();
        let obs_data = Rc::new(RefCell::new(0));
        let obs_data_clone = Rc::clone(&obs_data);

        context.subscribe_to_event::<Event1>(move |_, event| {
            *obs_data_clone.borrow_mut() += event.data;
        });

        context.emit_event(Event1 { data: 1 });
        context.emit_event(Event1 { data: 2 });
        context.execute();

        // Both of these should have been received.
        assert_eq!(*obs_data.borrow(), 3);
    }

    #[test]
    fn multiple_event_handlers() {
        let mut context = Context::new();
        let obs_data1 = Rc::new(RefCell::new(0));
        let obs_data1_clone = Rc::clone(&obs_data1);
        let obs_data2 = Rc::new(RefCell::new(0));
        let obs_data2_clone = Rc::clone(&obs_data2);

        context.subscribe_to_event::<Event1>(move |_, event| {
            *obs_data1_clone.borrow_mut() = event.data;
        });
        context.subscribe_to_event::<Event1>(move |_, event| {
            *obs_data2_clone.borrow_mut() = event.data;
        });
        context.emit_event(Event1 { data: 1 });
        context.execute();
        assert_eq!(*obs_data1.borrow(), 1);
        assert_eq!(*obs_data2.borrow(), 1);
    }

    #[test]
    fn multiple_event_types() {
        let mut context = Context::new();
        let obs_data1 = Rc::new(RefCell::new(0));
        let obs_data1_clone = Rc::clone(&obs_data1);
        let obs_data2 = Rc::new(RefCell::new(0));
        let obs_data2_clone = Rc::clone(&obs_data2);

        context.subscribe_to_event::<Event1>(move |_, event| {
            *obs_data1_clone.borrow_mut() = event.data;
        });
        context.subscribe_to_event::<Event2>(move |_, event| {
            *obs_data2_clone.borrow_mut() = event.data;
        });
        context.emit_event(Event1 { data: 1 });
        context.emit_event(Event2 { data: 2 });
        context.execute();
        assert_eq!(*obs_data1.borrow(), 1);
        assert_eq!(*obs_data2.borrow(), 2);
    }

    #[test]
    fn subscribe_after_event() {
        let mut context = Context::new();
        let obs_data = Rc::new(RefCell::new(0));
        let obs_data_clone = Rc::clone(&obs_data);

        context.emit_event(Event1 { data: 1 });
        context.subscribe_to_event::<Event1>(move |_, event| {
            *obs_data_clone.borrow_mut() = event.data;
        });

        context.execute();
        assert_eq!(*obs_data.borrow(), 0);
    }

    #[test]
    fn shutdown_cancels_plans() {
        let mut context = Context::new();
        add_plan(&mut context, 1.0, 1);
        context.add_plan(1.5, Context::shutdown);
        add_plan(&mut context, 2.0, 2);
        context.execute();
        assert_eq!(context.get_current_time(), 1.5);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);
    }

    #[test]
    fn shutdown_cancels_callbacks() {
        let mut context = Context::new();
        add_plan(&mut context, 1.0, 1);
        context.add_plan(1.5, |context| {
            // Note that we add the callback *before* we call shutdown
            // but shutdown cancels everything.
            context.queue_callback(|context| {
                context.get_data_mut(ComponentA).push(3);
            });
            context.shutdown();
        });
        context.execute();
        assert_eq!(context.get_current_time(), 1.5);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);
    }

    #[test]
    fn shutdown_cancels_events() {
        let mut context = Context::new();
        let obs_data = Rc::new(RefCell::new(0));
        let obs_data_clone = Rc::clone(&obs_data);
        context.subscribe_to_event::<Event1>(move |_, event| {
            *obs_data_clone.borrow_mut() = event.data;
        });
        context.emit_event(Event1 { data: 1 });
        context.shutdown();
        context.execute();
        assert_eq!(*obs_data.borrow(), 0);
    }

    #[test]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    fn periodic_plan_self_schedules() {
        // checks whether the person properties report schedules itself
        // based on whether there are plans in the queue
        let mut context = Context::new();
        context.add_periodic_plan_with_phase(
            1.0,
            |context| {
                let time = context.get_current_time();
                context.get_data_mut(ComponentA).push(time as u32);
            },
            ExecutionPhase::Last,
        );
        context.add_plan(1.0, move |_context| {});
        context.add_plan(1.5, move |_context| {});
        context.execute();
        assert_eq!(context.get_current_time(), 2.0);

        assert_eq!(*context.get_data(ComponentA), vec![0, 1, 2]); // time 0.0, 1.0, and 2.0
    }

    #[test]
    fn shutdown_requested_reset() {
      // This test verifies that shutdown_requested is properly reset after
      // being acted upon. This allows the context to be reused after shutdown.
      let mut context = Context::new();
      context.add_person(()).unwrap();

      // Schedule a plan at time 0.0 that calls shutdown
      context.add_plan(0.0, |ctx| {
        ctx.shutdown();
      });

      // First execute - should run until shutdown
      context.execute();
      assert_eq!(context.get_current_time(), 0.0);
      assert_eq!(context.get_current_population(), 1);

      // Add a new plan at time 2.0
      context.add_plan(2.0, |ctx| {
        ctx.add_person(()).unwrap();
      });

      // Second execute - should execute the new plan
      // If shutdown_requested wasn't reset, this would immediately break
      // without executing the plan, leaving population at 1.
      context.execute();
      assert_eq!(context.get_current_time(), 2.0);
      assert_eq!(
        context.get_current_population(),
        2,
        "If this fails, shutdown_requested was not properly reset"
      );
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
