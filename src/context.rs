//! A manager for the state of a discrete-event simulation
//!
//! Defines a [`Context`] that is intended to provide the foundational mechanism
//! for storing and manipulating the state of a given simulation.
use std::any::{Any, TypeId};
use std::cell::OnceCell;
use std::collections::VecDeque;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;
use std::rc::Rc;

use crate::data_plugin::DataPlugin;
use crate::entity::entity_store::EntityStore;
use crate::entity::multi_property::emit_pre_main_diagnostics;
use crate::entity::property::Property;
use crate::entity::property_value_store_core::PropertyValueStoreCore;
use crate::entity::Entity;
use crate::execution_stats::{
    log_execution_statistics, print_execution_statistics, ExecutionProfilingCollector,
    ExecutionStatistics,
};
use crate::global_properties::get_global_property_count;
use crate::plan_queue::{PlanId, PlanQueue};
use crate::{get_data_plugin_count, trace, warn, HashMap, HashMapExt};

/// The common callback used by multiple [`Context`] methods for future events
type Callback = dyn FnOnce(&mut Context);

/// A handler for an event type `E`
type EventHandler<E> = dyn Fn(&mut Context, E);

/// An opaque token for a registered event listener.
///
/// Pass this token to [`Context::unsubscribe_from_event`] to stop the listener
/// from receiving future emissions of the same event type.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EventListenerId<E: IxaEvent> {
    id: usize,
    event_type: PhantomData<fn() -> E>,
}

impl<E: IxaEvent> EventListenerId<E> {
    fn new(id: usize) -> Self {
        Self {
            id,
            event_type: PhantomData,
        }
    }
}

struct EventHandlerRegistration<E: IxaEvent> {
    listener_id: EventListenerId<E>,
    handler: Rc<EventHandler<E>>,
}

pub trait IxaEvent: Copy + 'static {
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

/// Tracks event-loop shutdown state and the current shutdown lifecycle phase.
///
/// This is private implementation state, not public API. `Context::shutdown`
/// requests normal shutdown and `Context::abort` requests an immediate stop of
/// the current `execute` loop. The stopped status is deliberately cleared when a
/// later `execute` call begins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShutdownStatus {
    /// Normal execution; no shutdown has been requested.
    None,
    /// Normal shutdown requested or in progress.
    ///
    /// In this state, callbacks still run first, but regular plans are executed
    /// only if they are scheduled at `Context::current_time`. Simulation time is
    /// not advanced.
    Normal,
    /// Drain the distinguished shutdown-time plan queue.
    ///
    /// Once this state is reached, regular plans are not inspected again during
    /// the same execution pass, even if shutdown-time work schedules a regular
    /// plan at the current simulation time. Callbacks are still executed.
    ShutdownTimePlans,
    /// Stop the current `execute` event loop.
    ///
    /// This is set by `Context::abort` and when the shutdown-time queue is
    /// exhausted. Manual `execute_single_step` calls clear this state when there
    /// is no callback to run.
    Stopped,
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
    plan_queue: PlanQueue,
    callback_queue: VecDeque<Box<Callback>>,
    event_handlers: HashMap<TypeId, Box<dyn Any>>,
    next_event_listener_id: usize,
    pub(crate) entity_store: EntityStore,
    data_plugins: Vec<OnceCell<Box<dyn Any>>>,
    pub(crate) global_properties: Vec<OnceCell<Box<dyn Any>>>,
    current_time: Option<f64>,
    start_time: Option<f64>,
    shutdown_status: ShutdownStatus,
    execution_profiler: ExecutionProfilingCollector,
    pub(crate) print_execution_statistics: bool,
}

impl Context {
    /// Create a new empty `Context`
    #[must_use]
    pub fn new() -> Context {
        emit_pre_main_diagnostics();

        // Create a vector to accommodate all registered data plugins
        let data_plugins = std::iter::repeat_with(OnceCell::new)
            .take(get_data_plugin_count())
            .collect();
        let global_properties = std::iter::repeat_with(OnceCell::new)
            .take(get_global_property_count())
            .collect();

        Context {
            plan_queue: PlanQueue::new(),
            callback_queue: VecDeque::new(),
            event_handlers: HashMap::new(),
            next_event_listener_id: 0,
            entity_store: EntityStore::new(),
            data_plugins,
            global_properties,
            current_time: None,
            start_time: None,
            shutdown_status: ShutdownStatus::None,
            execution_profiler: ExecutionProfilingCollector::new(),
            print_execution_statistics: false,
        }
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

    /// Register to handle emission of events of type E
    ///
    /// Handlers will be called upon event emission in order of subscription as
    /// queued `Callback`s with the appropriate event.
    pub fn subscribe_to_event<E: IxaEvent>(
        &mut self,
        handler: impl Fn(&mut Context, E) + 'static,
    ) -> EventListenerId<E> {
        let listener_id = EventListenerId::new(self.next_event_listener_id);
        self.next_event_listener_id = self
            .next_event_listener_id
            .checked_add(1)
            .unwrap_or_else(|| panic!("event listener id overflow"));

        let handler_vec = self
            .event_handlers
            .entry(TypeId::of::<E>())
            .or_insert_with(|| Box::<Vec<EventHandlerRegistration<E>>>::default());
        let handler_vec: &mut Vec<EventHandlerRegistration<E>> =
            handler_vec.downcast_mut().unwrap();
        handler_vec.push(EventHandlerRegistration {
            listener_id,
            handler: Rc::new(handler),
        });
        E::on_subscribe(self);
        listener_id
    }

    /// Unsubscribe a previously registered event listener.
    ///
    /// Returns `true` if a listener was unsubscribed and `false` if the token is
    /// unknown, already unsubscribed, or otherwise absent.
    #[allow(clippy::missing_panics_doc)]
    pub fn unsubscribe_from_event<E: IxaEvent>(
        &mut self,
        listener_id: &EventListenerId<E>,
    ) -> bool {
        let Some(handler_vec) = self.event_handlers.get_mut(&TypeId::of::<E>()) else {
            return false;
        };
        let handler_vec: &mut Vec<EventHandlerRegistration<E>> =
            handler_vec.downcast_mut().unwrap();
        let Some(index) = handler_vec
            .iter()
            .position(|entry| entry.listener_id.id == listener_id.id)
        else {
            return false;
        };

        handler_vec.swap_remove(index);
        true
    }

    pub(crate) fn has_event_handlers<E: IxaEvent>(&self) -> bool {
        self.event_handlers
            .get(&TypeId::of::<E>())
            .is_some_and(|handler_vec| {
                let handler_vec: &Vec<EventHandlerRegistration<E>> =
                    handler_vec.downcast_ref().unwrap();
                !handler_vec.is_empty()
            })
    }

    /// Emit an event of type E to be handled by registered receivers
    ///
    /// Receivers will handle events in the order that they have subscribed and
    /// are queued as callbacks
    pub fn emit_event<E: IxaEvent>(&mut self, event: E) {
        // Destructure to obtain event handlers and plan queue
        let Context {
            event_handlers,
            callback_queue,
            ..
        } = self;
        if let Some(handler_vec) = event_handlers.get(&TypeId::of::<E>()) {
            let handler_vec: &Vec<EventHandlerRegistration<E>> =
                handler_vec.downcast_ref().unwrap();
            for registration in handler_vec {
                let handler_clone = Rc::clone(&registration.handler);
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
        self.add_plan_with_phase_and_passivity(time, callback, phase, false)
    }

    /// Add a passive plan to the future event list at the specified time in the
    /// normal phase.
    ///
    /// Passive plans execute like regular plans but do not keep the simulation
    /// timeline alive.
    ///
    /// Returns a [`PlanId`] for the newly-added plan that can be used to cancel it
    /// if needed.
    /// # Panics
    ///
    /// Panics if time is in the past, infinite, or NaN.
    pub fn add_passive_plan(
        &mut self,
        time: f64,
        callback: impl FnOnce(&mut Context) + 'static,
    ) -> PlanId {
        self.add_passive_plan_with_phase(time, callback, ExecutionPhase::Normal)
    }

    /// Add a passive plan to the future event list at the specified time and
    /// with the specified phase.
    ///
    /// Passive plans execute like regular plans but do not keep the simulation
    /// timeline alive.
    ///
    /// Returns a [`PlanId`] for the newly-added plan that can be used to cancel it
    /// if needed.
    /// # Panics
    ///
    /// Panics if time is in the past, infinite, or NaN.
    pub fn add_passive_plan_with_phase(
        &mut self,
        time: f64,
        callback: impl FnOnce(&mut Context) + 'static,
        phase: ExecutionPhase,
    ) -> PlanId {
        self.add_plan_with_phase_and_passivity(time, callback, phase, true)
    }

    fn add_plan_with_phase_and_passivity(
        &mut self,
        time: f64,
        callback: impl FnOnce(&mut Context) + 'static,
        phase: ExecutionPhase,
        is_passive: bool,
    ) -> PlanId {
        let current = self.get_current_time();
        assert!(!time.is_nan(), "Time {time} is invalid: cannot be NaN");
        assert!(
            !time.is_infinite(),
            "Time {time} is invalid: cannot be infinite"
        );
        assert!(
            time >= current,
            "Time {time} is invalid: cannot be less than the current time ({}). Consider calling set_start_time() before scheduling plans.",
            current
        );
        self.plan_queue
            .add_plan(time, Box::new(callback), phase, is_passive)
    }

    /// Add a plan to execute during shutdown-time in the normal phase.
    ///
    /// Shutdown-time plans execute after regular plans at the current simulation
    /// time are exhausted during normal shutdown, and after natural exhaustion of
    /// the regular plan queue.
    ///
    /// Returns a [`PlanId`] for the newly-added plan that can be used to cancel it
    /// if needed.
    pub fn add_shutdown_plan(&mut self, callback: impl FnOnce(&mut Context) + 'static) -> PlanId {
        self.add_shutdown_plan_with_phase(callback, ExecutionPhase::Normal)
    }

    /// Add a plan to execute during shutdown-time with the specified phase.
    ///
    /// Shutdown-time plans have no simulation time. They are ordered by phase and
    /// insertion order.
    ///
    /// Returns a [`PlanId`] for the newly-added plan that can be used to cancel it
    /// if needed.
    pub fn add_shutdown_plan_with_phase(
        &mut self,
        callback: impl FnOnce(&mut Context) + 'static,
        phase: ExecutionPhase,
    ) -> PlanId {
        self.plan_queue.add_shutdown_plan(Box::new(callback), phase)
    }

    pub(crate) fn evaluate_periodic_and_schedule_next(
        &mut self,
        period: f64,
        callback: impl Fn(&mut Context) + 'static,
        phase: ExecutionPhase,
    ) {
        trace!(
            "evaluate periodic at {} (period={})",
            self.get_current_time(),
            period
        );
        callback(self);
        let next_time = self.get_current_time() + period;
        self.add_passive_plan_with_phase(
            next_time,
            move |context| context.evaluate_periodic_and_schedule_next(period, callback, phase),
            phase,
        );
    }

    /// Add a passive periodic plan with specified priority to the future event
    /// list.
    ///
    /// Periodic plans reschedule themselves after every run. They do not keep
    /// the simulation timeline alive: when no non-passive plans remain, normal
    /// shutdown begins, and only passive plans at the final current time can
    /// still run during that execution pass. Future passive periodic plans
    /// remain queued and may run if later non-passive work is scheduled.
    ///
    /// Notes:
    /// * The first periodic plan is scheduled at time `0.0`. If `set_start_time` was
    ///   set to a positive value, this will currently panic because the first plan
    ///   occurs before the start time (see issue #634 for future behavior).
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

        self.add_passive_plan_with_phase(
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
    pub fn get_data<T: DataPlugin>(&self, _data_plugin: T) -> &T::DataContainer {
        let index = T::index_within_context();
        self.data_plugins
            .get(index)
            .unwrap_or_else(|| panic!("No data plugin found with index = {index:?}. You must use the `define_data_plugin!` macro to create a data plugin."))
            .get_or_init(|| Box::new(T::init(self)))
            .downcast_ref::<T::DataContainer>()
            .expect("TypeID does not match data plugin type. You must use the `define_data_plugin!` macro to create a data plugin.")
    }

    /// Request normal shutdown.
    ///
    /// Normal shutdown stops simulation time from advancing. Execution continues
    /// through queued callbacks, regular plans at the current time, and then
    /// shutdown-time plans. Calling `shutdown` during shutdown-time execution
    /// does not return execution to regular current-time plans.
    pub fn shutdown(&mut self) {
        trace!("shutdown context");
        if self.shutdown_status == ShutdownStatus::None {
            self.shutdown_status = ShutdownStatus::Normal;
        }
    }

    /// Stop the current event loop immediately.
    ///
    /// Abort only stops the current `execute` loop. The stopped status is cleared
    /// when `execute` is called again.
    pub fn abort(&mut self) {
        trace!("abort context");
        self.shutdown_status = ShutdownStatus::Stopped;
    }

    /// Get the current simulation time
    ///
    /// Returns the current time in the simulation. The behavior depends on execution state:
    /// * During execution: returns the time of the currently executing plan or callback
    /// * Before execution: returns the start time (if set via [`Context::set_start_time`]), or `0.0`
    ///
    /// The time can be negative if a negative start time was set before execution.
    #[must_use]
    pub fn get_current_time(&self) -> f64 {
        self.current_time.or(self.start_time).unwrap_or(0.0)
    }

    /// Set the start time for the simulation. Must be finite.
    ///
    /// * Call before `Context.execute()`.
    /// * `start_time` must be finite (not NaN or infinite).
    /// * May be called only once.
    /// * If plans are already scheduled, `start_time` must be earlier than or equal to
    ///   the earliest scheduled plan time.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// * `start_time` is NaN or infinite.
    /// * the start time was already set.
    /// * `Context::execute()` has been called.
    /// * `start_time` is later than the earliest scheduled plan time.
    pub fn set_start_time(&mut self, start_time: f64) {
        assert!(
            !start_time.is_nan() && !start_time.is_infinite(),
            "Start time {start_time} must be finite"
        );
        assert!(
            self.start_time.is_none(),
            "Start time has already been set. It can only be set once."
        );
        assert!(
            self.current_time.is_none(),
            "Start time cannot be set after execution has begun."
        );
        if let Some(next_time) = self.plan_queue.next_time() {
            assert!(
                start_time <= next_time,
                "Start time {} is later than the earliest scheduled plan time {}. Remove or reschedule existing plans first.",
                start_time,
                next_time
            );
        }
        self.start_time = Some(start_time);
    }

    /// Get the start time that was set via `set_start_time`, or `None` if not set.
    #[must_use]
    pub fn get_start_time(&self) -> Option<f64> {
        self.start_time
    }

    /// Execute the simulation until callbacks and plans are exhausted and shutdown
    /// work is complete.
    pub fn execute(&mut self) {
        trace!("entering event loop");

        if self.shutdown_status == ShutdownStatus::Stopped {
            self.shutdown_status = ShutdownStatus::None;
        }

        if self.current_time.is_none() {
            self.current_time = Some(self.start_time.unwrap_or(0.0));
        }

        // Start plan loop
        loop {
            if self.shutdown_status == ShutdownStatus::Stopped {
                self.shutdown_status = ShutdownStatus::None;
                break;
            }

            self.execute_single_step();
            self.execution_profiler.refresh();
        }

        let stats = self.get_execution_statistics();
        if self.print_execution_statistics {
            print_execution_statistics(&stats);
            #[cfg(feature = "profiling")]
            crate::profiling::print_profiling_data();
        } else {
            log_execution_statistics(&stats);
        }
    }

    /// Executes a single callback, plan, or shutdown status transition.
    pub fn execute_single_step(&mut self) {
        // Callbacks always have priority over plan selection. This remains true
        // even in `Stopped` during manual stepping; `Stopped` only stops the
        // `execute` loop, not the ability to explicitly step callbacks manually.
        if let Some(callback) = self.callback_queue.pop_front() {
            trace!("calling callback");
            callback(self);
            return;
        }

        // No callback is available, so the shutdown status determines which
        // plan queue, if any, can provide the next unit of work.
        match self.shutdown_status {
            ShutdownStatus::None => {
                // Normal execution may advance simulation time to the next
                // regular plan only while non-passive regular work remains. Once no
                // non-passive regular plans remain, enter normal shutdown to drain
                // current-time regular work without advancing time.
                if let Some(plan) = self.plan_queue.pop_next_if_active() {
                    trace!("calling plan at {:.6}", plan.time);
                    self.current_time = Some(plan.time);
                    (plan.data)(self);
                } else {
                    self.shutdown_status = ShutdownStatus::Normal;
                }
            }
            ShutdownStatus::Normal => {
                // Normal shutdown drains only regular plans scheduled at the
                // current simulation time. Future regular plans must remain in
                // the queue so a later `execute` call can run them.
                if let Some(plan) = self.plan_queue.pop_next_at(self.get_current_time()) {
                    trace!("calling plan at {:.6}", plan.time);
                    (plan.data)(self);
                } else {
                    self.shutdown_status = ShutdownStatus::ShutdownTimePlans;
                }
            }
            ShutdownStatus::ShutdownTimePlans => {
                // Once shutdown-time draining begins, do not return to the
                // regular plan queue during this execution pass.
                if let Some(plan) = self.plan_queue.pop_next_shutdown() {
                    trace!("calling shutdown-time plan");
                    (plan.data)(self);
                } else {
                    self.shutdown_status = ShutdownStatus::Stopped;
                }
            }
            ShutdownStatus::Stopped => {
                // `execute` exits before calling `execute_single_step` in this
                // state. This arm supports manual single-step use after a prior
                // stop by consuming the stopped status when no callback exists.
                self.shutdown_status = ShutdownStatus::None;
            }
        }
    }

    #[must_use]
    pub fn get_execution_statistics(&mut self) -> ExecutionStatistics {
        #[allow(unused_mut)]
        let mut stats = self.execution_profiler.compute_final_statistics();
        #[cfg(feature = "profiling")]
        {
            stats.max_plans_in_flight = self.plan_queue.max_plans_in_flight;
            stats.max_plan_queue_memory_in_use = self.plan_queue.max_memory_in_use;
        }
        stats
    }
}

pub trait ContextBase: Sized {
    fn subscribe_to_event<E: IxaEvent>(
        &mut self,
        handler: impl Fn(&mut Context, E) + 'static,
    ) -> EventListenerId<E>;
    fn unsubscribe_from_event<E: IxaEvent>(&mut self, listener_id: &EventListenerId<E>) -> bool;
    fn emit_event<E: IxaEvent>(&mut self, event: E);
    fn add_plan(&mut self, time: f64, callback: impl FnOnce(&mut Context) + 'static) -> PlanId;
    fn add_plan_with_phase(
        &mut self,
        time: f64,
        callback: impl FnOnce(&mut Context) + 'static,
        phase: ExecutionPhase,
    ) -> PlanId;
    fn add_passive_plan(
        &mut self,
        time: f64,
        callback: impl FnOnce(&mut Context) + 'static,
    ) -> PlanId;
    fn add_passive_plan_with_phase(
        &mut self,
        time: f64,
        callback: impl FnOnce(&mut Context) + 'static,
        phase: ExecutionPhase,
    ) -> PlanId;
    fn add_shutdown_plan(&mut self, callback: impl FnOnce(&mut Context) + 'static) -> PlanId;
    fn add_shutdown_plan_with_phase(
        &mut self,
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
    #[must_use]
    fn get_data_mut<T: DataPlugin>(&mut self, plugin: T) -> &mut T::DataContainer;
    #[must_use]
    fn get_data<T: DataPlugin>(&self, plugin: T) -> &T::DataContainer;
    #[must_use]
    fn get_current_time(&self) -> f64;
    #[must_use]
    fn get_execution_statistics(&mut self) -> ExecutionStatistics;
    fn abort(&mut self);
}
impl ContextBase for Context {
    delegate::delegate! {
        to self {
            fn subscribe_to_event<E: IxaEvent>(&mut self, handler: impl Fn(&mut Context, E) + 'static) -> EventListenerId<E>;
            fn unsubscribe_from_event<E: IxaEvent>(&mut self, listener_id: &EventListenerId<E>) -> bool;
            fn emit_event<E: IxaEvent>(&mut self, event: E);
            fn add_plan(&mut self, time: f64, callback: impl FnOnce(&mut Context) + 'static) -> PlanId;
            fn add_plan_with_phase(&mut self, time: f64, callback: impl FnOnce(&mut Context) + 'static, phase: ExecutionPhase) -> PlanId;
            fn add_passive_plan(&mut self, time: f64, callback: impl FnOnce(&mut Context) + 'static) -> PlanId;
            fn add_passive_plan_with_phase(&mut self, time: f64, callback: impl FnOnce(&mut Context) + 'static, phase: ExecutionPhase) -> PlanId;
            fn add_shutdown_plan(&mut self, callback: impl FnOnce(&mut Context) + 'static) -> PlanId;
            fn add_shutdown_plan_with_phase(&mut self, callback: impl FnOnce(&mut Context) + 'static, phase: ExecutionPhase) -> PlanId;
            fn add_periodic_plan_with_phase(&mut self, period: f64, callback: impl Fn(&mut Context) + 'static, phase: ExecutionPhase);
            fn cancel_plan(&mut self, plan_id: &PlanId);
            fn queue_callback(&mut self, callback: impl FnOnce(&mut Context) + 'static);
            fn get_data_mut<T: DataPlugin>(&mut self, plugin: T) -> &mut T::DataContainer;
            fn get_data<T: DataPlugin>(&self, plugin: T) -> &T::DataContainer;
            fn get_current_time(&self) -> f64;
            fn get_execution_statistics(&mut self) -> ExecutionStatistics;
            fn abort(&mut self);
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    // We allow defining items that are never used to test macros.
    #![allow(dead_code)]
    use std::cell::RefCell;
    use std::marker::PhantomData;

    use super::*;
    use crate::{
        define_data_plugin, define_entity, define_property, with, ContextEntitiesExt, IxaEvent,
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

    fn add_passive_plan(context: &mut Context, time: f64, value: u32) -> PlanId {
        context.add_passive_plan(time, move |context| {
            context.get_data_mut(ComponentA).push(value);
        })
    }

    fn add_passive_plan_with_phase(
        context: &mut Context,
        time: f64,
        value: u32,
        phase: ExecutionPhase,
    ) -> PlanId {
        context.add_passive_plan_with_phase(
            time,
            move |context| {
                context.get_data_mut(ComponentA).push(value);
            },
            phase,
        )
    }

    #[test]
    #[should_panic(expected = "Time inf is invalid")]
    fn infinite_plan_time() {
        let mut context = Context::new();
        add_plan(&mut context, f64::INFINITY, 0);
    }

    #[test]
    #[should_panic(expected = "Time NaN is invalid")]
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

    #[derive(IxaEvent)]
    struct Event1 {
        pub data: usize,
    }

    #[derive(IxaEvent)]
    struct Event2 {
        pub data: usize,
    }

    struct NotCopy;

    #[derive(IxaEvent)]
    struct GenericEvent<T> {
        pub data: usize,
        _marker: PhantomData<T>,
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
    fn derive_ixa_event_implements_copy_for_generic_events() {
        fn assert_clone<T: Clone>() {}
        fn assert_copy<T: Copy>() {}
        assert_clone::<GenericEvent<NotCopy>>();
        assert_copy::<GenericEvent<NotCopy>>();

        let mut context = Context::new();
        let obs_data = Rc::new(RefCell::new(0));
        let obs_data_clone = Rc::clone(&obs_data);

        context.subscribe_to_event::<GenericEvent<NotCopy>>(move |_, event| {
            *obs_data_clone.borrow_mut() = event.data;
        });

        let event = GenericEvent::<NotCopy> {
            data: 5,
            _marker: PhantomData,
        };
        let copied_event = event;

        assert_eq!(copied_event.data, 5);
        context.emit_event(copied_event);
        context.execute();
        assert_eq!(*obs_data.borrow(), 5);
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
    fn unsubscribe_from_event_before_emit() {
        let mut context = Context::new();
        let obs_data = Rc::new(RefCell::new(0));
        let obs_data_clone = Rc::clone(&obs_data);

        let listener_id = context.subscribe_to_event::<Event1>(move |_, event| {
            *obs_data_clone.borrow_mut() = event.data;
        });

        assert!(context.has_event_handlers::<Event1>());
        assert!(context.unsubscribe_from_event(&listener_id));
        assert!(!context.has_event_handlers::<Event1>());

        context.emit_event(Event1 { data: 1 });
        context.execute();
        assert_eq!(*obs_data.borrow(), 0);
    }

    #[test]
    fn unsubscribe_from_event_preserves_other_listeners_in_order() {
        let mut context = Context::new();
        let observed = Rc::new(RefCell::new(Vec::new()));

        let observed_clone = Rc::clone(&observed);
        context.subscribe_to_event::<Event1>(move |_, event| {
            observed_clone.borrow_mut().push(event.data);
        });
        let observed_clone = Rc::clone(&observed);
        let listener_to_unsubscribe = context.subscribe_to_event::<Event1>(move |_, event| {
            observed_clone.borrow_mut().push(event.data + 10);
        });
        let observed_clone = Rc::clone(&observed);
        context.subscribe_to_event::<Event1>(move |_, event| {
            observed_clone.borrow_mut().push(event.data + 20);
        });

        assert!(context.unsubscribe_from_event(&listener_to_unsubscribe));

        context.emit_event(Event1 { data: 1 });
        context.execute();
        assert_eq!(*observed.borrow(), vec![1, 21]);
    }

    #[test]
    fn unsubscribe_from_event_does_not_affect_other_event_types() {
        let mut context = Context::new();
        let obs_data1 = Rc::new(RefCell::new(0));
        let obs_data1_clone = Rc::clone(&obs_data1);
        let obs_data2 = Rc::new(RefCell::new(0));
        let obs_data2_clone = Rc::clone(&obs_data2);

        let listener_id = context.subscribe_to_event::<Event1>(move |_, event| {
            *obs_data1_clone.borrow_mut() = event.data;
        });
        context.subscribe_to_event::<Event2>(move |_, event| {
            *obs_data2_clone.borrow_mut() = event.data;
        });

        assert!(context.unsubscribe_from_event(&listener_id));

        context.emit_event(Event1 { data: 1 });
        context.emit_event(Event2 { data: 2 });
        context.execute();
        assert_eq!(*obs_data1.borrow(), 0);
        assert_eq!(*obs_data2.borrow(), 2);
    }

    #[test]
    fn unsubscribe_from_event_does_not_cancel_already_queued_callback() {
        let mut context = Context::new();
        let obs_data = Rc::new(RefCell::new(0));
        let obs_data_clone = Rc::clone(&obs_data);

        let listener_id = context.subscribe_to_event::<Event1>(move |_, event| {
            *obs_data_clone.borrow_mut() = event.data;
        });

        context.emit_event(Event1 { data: 1 });
        assert!(context.unsubscribe_from_event(&listener_id));
        context.execute();
        assert_eq!(*obs_data.borrow(), 1);
    }

    #[test]
    fn unsubscribe_from_event_returns_false_when_already_unsubscribed() {
        let mut context = Context::new();
        let listener_id = context.subscribe_to_event::<Event1>(move |_, _| {});

        assert!(context.unsubscribe_from_event(&listener_id));
        assert!(!context.unsubscribe_from_event(&listener_id));
    }

    #[test]
    fn unsubscribe_from_event_returns_false_for_unknown_listener_id() {
        let mut context1 = Context::new();
        let mut context2 = Context::new();
        let obs_data = Rc::new(RefCell::new(0));
        let obs_data_clone = Rc::clone(&obs_data);

        context1.subscribe_to_event::<Event1>(move |_, _| {});
        let unknown_listener_id = context1.subscribe_to_event::<Event1>(move |_, _| {});
        context2.subscribe_to_event::<Event1>(move |_, event| {
            *obs_data_clone.borrow_mut() = event.data;
        });

        assert!(!context2.unsubscribe_from_event(&unknown_listener_id));

        context2.emit_event(Event1 { data: 1 });
        context2.execute();
        assert_eq!(*obs_data.borrow(), 1);
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
    fn shutdown_runs_current_time_plans_and_preserves_future_plan() {
        let mut context = Context::new();
        add_plan(&mut context, 1.0, 1);
        context.add_plan(1.5, |context| {
            context.get_data_mut(ComponentA).push(2);
            context.shutdown();
        });
        add_plan(&mut context, 1.5, 3);
        add_plan(&mut context, 2.0, 2);
        context.execute();
        assert_eq!(context.get_current_time(), 1.5);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2, 3]);

        context.execute();
        assert_eq!(context.get_current_time(), 2.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2, 3, 2]);
    }

    #[test]
    fn shutdown_runs_queued_callbacks() {
        let mut context = Context::new();
        add_plan(&mut context, 1.0, 1);
        context.add_plan(1.5, |context| {
            context.queue_callback(|context| {
                context.get_data_mut(ComponentA).push(3);
            });
            context.shutdown();
        });
        context.execute();
        assert_eq!(context.get_current_time(), 1.5);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 3]);
    }

    #[test]
    fn shutdown_runs_queued_events() {
        let mut context = Context::new();
        let obs_data = Rc::new(RefCell::new(0));
        let obs_data_clone = Rc::clone(&obs_data);
        context.subscribe_to_event::<Event1>(move |_, event| {
            *obs_data_clone.borrow_mut() = event.data;
        });
        context.emit_event(Event1 { data: 1 });
        context.shutdown();
        context.execute();
        assert_eq!(*obs_data.borrow(), 1);
    }

    #[test]
    fn shutdown_runs_regular_plans_at_current_time_all_phases() {
        let mut context = Context::new();
        context.add_plan_with_phase(
            1.0,
            |context| {
                context.get_data_mut(ComponentA).push(1);
            },
            ExecutionPhase::First,
        );
        context.add_plan(1.0, |context| {
            context.get_data_mut(ComponentA).push(2);
            context.shutdown();
        });
        add_plan(&mut context, 1.0, 3);
        add_plan_with_phase(&mut context, 1.0, 4, ExecutionPhase::Last);
        add_plan(&mut context, 2.0, 5);

        context.execute();

        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2, 3, 4]);
    }

    #[test]
    fn shutdown_time_plans_run_after_current_time_plans() {
        let mut context = Context::new();
        context.add_shutdown_plan(|context| {
            context.get_data_mut(ComponentA).push(3);
        });
        context.add_plan(1.0, |context| {
            context.get_data_mut(ComponentA).push(1);
            context.shutdown();
        });
        add_plan(&mut context, 1.0, 2);

        context.execute();

        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2, 3]);
    }

    #[test]
    fn shutdown_plan_phase_order_is_respected() {
        let mut context = Context::new();
        context.add_shutdown_plan_with_phase(
            |context| {
                context.get_data_mut(ComponentA).push(3);
            },
            ExecutionPhase::Last,
        );
        context.add_shutdown_plan_with_phase(
            |context| {
                context.get_data_mut(ComponentA).push(1);
            },
            ExecutionPhase::First,
        );
        context.add_shutdown_plan(|context| {
            context.get_data_mut(ComponentA).push(2);
        });
        context.execute();

        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2, 3]);
    }

    #[test]
    fn shutdown_plan_callbacks_are_drained() {
        let mut context = Context::new();
        context.add_shutdown_plan(|context| {
            context.get_data_mut(ComponentA).push(1);
            context.queue_callback(|context| {
                context.get_data_mut(ComponentA).push(2);
            });
        });
        context.add_shutdown_plan(|context| {
            context.get_data_mut(ComponentA).push(3);
        });

        context.execute();

        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2, 3]);
    }

    #[test]
    fn shutdown_time_does_not_return_to_regular_queue() {
        let mut context = Context::new();
        context.add_shutdown_plan(|context| {
            context.get_data_mut(ComponentA).push(1);
            context.add_plan(context.get_current_time(), |context| {
                context.get_data_mut(ComponentA).push(3);
            });
        });
        context.add_shutdown_plan(|context| {
            context.get_data_mut(ComponentA).push(2);
        });

        context.execute();
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2]);

        context.execute();
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2, 3]);
    }

    #[test]
    fn passive_only_initial_time_runs_during_normal_shutdown() {
        let mut context = Context::new();
        add_passive_plan(&mut context, 0.0, 1);
        add_passive_plan(&mut context, 1.0, 2);

        context.execute();

        assert_eq!(context.get_current_time(), 0.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);
    }

    #[test]
    fn passive_plans_at_final_non_passive_time_run_across_phases() {
        let mut context = Context::new();
        add_passive_plan_with_phase(&mut context, 1.0, 1, ExecutionPhase::First);
        add_plan(&mut context, 1.0, 2);
        add_passive_plan_with_phase(&mut context, 1.0, 3, ExecutionPhase::Last);

        context.execute();

        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2, 3]);
    }

    #[test]
    fn passive_future_plan_survives_until_later_non_passive_work() {
        let mut context = Context::new();
        add_plan(&mut context, 1.0, 1);
        add_passive_plan(&mut context, 2.0, 2);

        context.execute();

        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);

        add_plan(&mut context, 2.0, 3);
        context.execute();

        assert_eq!(context.get_current_time(), 2.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2, 3]);
    }

    #[test]
    fn cancel_plan_can_cancel_shutdown_plan() {
        let mut context = Context::new();
        let to_cancel = context.add_shutdown_plan(|context| {
            context.get_data_mut(ComponentA).push(1);
        });
        context.cancel_plan(&to_cancel);

        context.execute();

        assert_eq!(*context.get_data_mut(ComponentA), Vec::<u32>::new());
    }

    #[test]
    fn abort_inside_plan_stops_execute_loop() {
        let mut context = Context::new();
        context.add_plan(1.0, |context| {
            context.get_data_mut(ComponentA).push(1);
            context.queue_callback(|context| {
                context.get_data_mut(ComponentA).push(2);
            });
            context.abort();
        });
        add_plan(&mut context, 2.0, 3);

        context.execute();
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);

        context.execute();
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2, 3]);
    }

    #[test]
    fn abort_before_execute_does_not_poison_later_execute() {
        let mut context = Context::new();
        context.abort();
        add_plan(&mut context, 1.0, 1);

        context.execute();

        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);
    }

    #[test]
    fn abort_during_normal_shutdown_exits_immediately() {
        let mut context = Context::new();
        context.add_plan(1.0, |context| {
            context.get_data_mut(ComponentA).push(1);
            context.shutdown();
        });
        context.add_plan(1.0, |context| {
            context.get_data_mut(ComponentA).push(2);
            context.abort();
        });
        add_plan(&mut context, 1.0, 3);

        context.execute();
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2]);

        context.execute();
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2, 3]);
    }

    #[test]
    fn shutdown_does_not_restart_stopped_status() {
        let mut context = Context::new();
        context.abort();
        context.shutdown();
        add_plan(&mut context, 1.0, 1);

        context.execute();

        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);
    }

    #[test]
    fn execute_single_step_runs_one_status_transition() {
        let mut context = Context::new();
        context.add_shutdown_plan(|context| {
            context.get_data_mut(ComponentA).push(1);
        });

        context.execute_single_step();
        assert_eq!(*context.get_data_mut(ComponentA), Vec::<u32>::new());

        context.execute_single_step();
        assert_eq!(*context.get_data_mut(ComponentA), Vec::<u32>::new());

        context.execute_single_step();
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);
    }

    #[test]
    fn execute_single_step_stopped_runs_callback_then_resets() {
        let mut context = Context::new();
        context.abort();
        context.queue_callback(|context| {
            context.get_data_mut(ComponentA).push(1);
        });
        add_plan(&mut context, 0.0, 2);

        context.execute_single_step();
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);

        context.execute_single_step();
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);

        context.execute_single_step();
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2]);
    }

    #[test]
    fn periodic_plan_self_schedules() {
        // checks whether the periodic plan schedules itself passively without
        // keeping execution alive after non-passive plans are exhausted.
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
        assert_eq!(context.get_current_time(), 1.5);

        assert_eq!(*context.get_data(ComponentA), vec![0, 1]); // time 0.0 and 1.0
    }

    // Tests for negative time handling

    #[test]
    fn negative_plan_time_allowed() {
        let mut context = Context::new();
        context.set_start_time(-1.0);
        add_plan(&mut context, -1.0, 1);
        context.execute();
        assert_eq!(context.get_current_time(), -1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);
    }

    #[test]
    fn add_plan_get_current_time() {
        let mut context = Context::new();
        let current_time = context.get_current_time();
        add_plan(&mut context, current_time, 1);
        context.execute();
        assert_eq!(context.get_current_time(), 0.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);
    }

    #[test]
    fn multiple_negative_plans() {
        let mut context = Context::new();
        context.set_start_time(-3.0);
        add_plan(&mut context, -3.0, 1);
        add_plan(&mut context, -1.0, 3);
        add_plan(&mut context, -2.0, 2);
        context.execute();
        assert_eq!(context.get_current_time(), -1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2, 3]);
    }

    #[test]
    fn negative_and_positive_plans() {
        let mut context = Context::new();
        context.set_start_time(-1.0);
        add_plan(&mut context, -1.0, 1);
        add_plan(&mut context, 1.0, 3);
        add_plan(&mut context, 0.0, 2);
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2, 3]);
    }

    #[test]
    fn get_current_time_before_execute_defaults() {
        let mut context = Context::new();
        assert_eq!(context.get_current_time(), 0.0);

        context.set_start_time(-2.0);
        assert_eq!(context.get_current_time(), -2.0);
    }

    #[test]
    fn get_current_time_initializes_to_zero_when_all_positive() {
        let mut context = Context::new();
        let seen_time = Rc::new(RefCell::new(f64::NAN));
        let seen_time_clone = Rc::clone(&seen_time);
        context.queue_callback(move |ctx| {
            *seen_time_clone.borrow_mut() = ctx.get_current_time();
        });
        context.execute();
        assert_eq!(*seen_time.borrow(), 0.0);
    }

    #[test]
    fn get_current_time_initializes_to_zero_when_empty() {
        let mut context = Context::new();
        context.execute();
        assert_eq!(context.get_current_time(), 0.0);
    }

    #[test]
    fn get_current_time_initializes_to_zero_with_plan() {
        let mut context = Context::new();
        add_plan(&mut context, 0.0, 1);
        context.execute();
        assert_eq!(context.get_current_time(), 0.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);
    }

    #[test]
    #[should_panic(expected = "Time -1 is invalid")]
    fn negative_time_from_callback_panics() {
        let mut context = Context::new();
        context.queue_callback(|context| {
            context.get_data_mut(ComponentA).push(1);
            add_plan(context, -1.0, 2);
        });
        add_plan(&mut context, 1.0, 3);
        context.execute();
    }

    #[test]
    fn large_negative_time() {
        let mut context = Context::new();
        context.set_start_time(-1_000_000.0);
        add_plan(&mut context, -1_000_000.0, 1);
        context.execute();
        assert_eq!(context.get_current_time(), -1_000_000.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);
    }

    #[test]
    fn very_small_negative_time() {
        let mut context = Context::new();
        context.set_start_time(-1e-10);
        add_plan(&mut context, -1e-10, 1);
        context.execute();
        assert_eq!(context.get_current_time(), -1e-10);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);
    }

    #[test]
    fn negative_time_ordering_with_phases() {
        let mut context = Context::new();
        context.set_start_time(-1.0);
        add_plan_with_phase(&mut context, -1.0, 1, ExecutionPhase::Normal);
        add_plan_with_phase(&mut context, -1.0, 3, ExecutionPhase::Last);
        add_plan_with_phase(&mut context, -1.0, 2, ExecutionPhase::First);
        context.execute();
        assert_eq!(context.get_current_time(), -1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![2, 1, 3]);
    }

    #[test]
    #[should_panic(expected = "Time 4 is invalid")]
    fn cannot_schedule_plan_before_current_time() {
        let mut context = Context::new();
        add_plan(&mut context, 5.0, 1);
        context.add_plan(5.0, |context| {
            // At time 5.0, we cannot schedule a plan at time 4.0
            add_plan(context, 4.0, 2);
        });
        context.execute();
    }

    #[test]
    fn get_current_time_multiple_calls_before_execute() {
        let mut context = Context::new();
        context.set_start_time(-2.0);
        add_plan(&mut context, -2.0, 1);
        context.execute();
        assert_eq!(context.get_current_time(), -2.0);
    }

    #[test]
    fn negative_plan_can_add_positive_plan() {
        let mut context = Context::new();
        context.set_start_time(-1.0);
        add_plan(&mut context, -1.0, 1);
        context.add_plan(-1.0, |context| {
            add_plan(context, 2.0, 2);
        });
        context.execute();
        assert_eq!(context.get_current_time(), 2.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2]);
    }

    #[test]
    fn negative_plan_can_schedule_negative_plan() {
        let mut context = Context::new();
        context.set_start_time(-2.0);
        add_plan(&mut context, -2.0, 1);
        context.add_plan(-2.0, |context| {
            add_plan(context, -1.0, 2);
        });
        context.execute();
        assert_eq!(context.get_current_time(), -1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2]);
    }

    #[test]
    #[should_panic(expected = "Start time has already been set. It can only be set once.")]
    fn set_start_time_only_once() {
        let mut context = Context::new();
        context.set_start_time(1.0);
        context.set_start_time(2.0);
    }

    // Additional coverage around time and plans

    #[test]
    #[should_panic(expected = "Start time NaN must be finite")]
    fn set_start_time_nan_panics() {
        let mut context = Context::new();
        context.set_start_time(f64::NAN);
    }

    #[test]
    #[should_panic(expected = "Start time inf must be finite")]
    fn set_start_time_inf_panics() {
        let mut context = Context::new();
        context.set_start_time(f64::INFINITY);
    }

    #[test]
    fn set_start_time_equal_to_earliest_plan_allowed() {
        let mut context = Context::new();
        context.set_start_time(-2.0);
        add_plan(&mut context, -2.0, 1);
        context.execute();
        assert_eq!(context.get_current_time(), -2.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1]);
    }

    // Note: adding a plan earlier than current_time after setting start time
    // is already covered by `add_plan_less_than_current_time_panics`.

    #[test]
    fn set_start_time_with_only_callbacks_keeps_time() {
        let mut context = Context::new();
        context.set_start_time(5.0);
        context.queue_callback(|ctx| {
            ctx.get_data_mut(ComponentA).push(42);
        });
        context.execute();
        assert_eq!(context.get_current_time(), 5.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![42]);
    }

    #[test]
    fn multiple_plans_final_time_is_last() {
        let mut context = Context::new();
        add_plan(&mut context, 1.0, 1);
        add_plan(&mut context, 3.0, 3);
        add_plan(&mut context, 2.0, 2);
        context.execute();
        assert_eq!(context.get_current_time(), 3.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2, 3]);
    }

    #[test]
    fn add_plan_same_time_fifo_and_phases() {
        let mut context = Context::new();
        add_plan_with_phase(&mut context, 1.0, 3, ExecutionPhase::Last);
        add_plan(&mut context, 1.0, 1);
        add_plan_with_phase(&mut context, 1.0, 2, ExecutionPhase::First);
        add_plan(&mut context, 1.0, 4);
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![2, 1, 4, 3]);
    }

    #[test]
    #[should_panic(expected = "Time -2 is invalid")]
    fn add_plan_less_than_current_time_panics() {
        let mut context = Context::new();
        context.set_start_time(-1.0);
        add_plan(&mut context, -1.0, 1);
        // Attempt to schedule before current time
        add_plan(&mut context, -2.0, 2);
    }

    #[test]
    #[should_panic(expected = "Period must be greater than 0")]
    fn add_periodic_plan_zero_period_panics() {
        let mut context = Context::new();
        context.add_periodic_plan_with_phase(0.0, |_ctx| {}, ExecutionPhase::Normal);
    }

    #[test]
    #[should_panic(expected = "Period must be greater than 0")]
    fn add_periodic_plan_nan_panics() {
        let mut context = Context::new();
        context.add_periodic_plan_with_phase(f64::NAN, |_ctx| {}, ExecutionPhase::Normal);
    }

    #[test]
    #[should_panic(expected = "Period must be greater than 0")]
    fn add_periodic_plan_inf_panics() {
        let mut context = Context::new();
        context.add_periodic_plan_with_phase(f64::INFINITY, |_ctx| {}, ExecutionPhase::Normal);
    }

    #[test]
    fn shutdown_status_reset() {
        // This test verifies that shutdown_status is properly reset after
        // being acted upon. This allows the context to be reused after shutdown.
        let mut context = Context::new();
        let _: PersonId = context.add_entity(with!(Person, Age(50))).unwrap();

        // Schedule a plan at time 0.0 that calls shutdown
        context.add_plan(0.0, |ctx| {
            ctx.shutdown();
        });

        // First execute - should run until shutdown
        context.execute();
        assert_eq!(context.get_current_time(), 0.0);
        assert_eq!(context.get_entity_count::<Person>(), 1);

        // Add a new plan at time 2.0
        context.add_plan(2.0, |ctx| {
            let _: PersonId = ctx.add_entity(with!(Person, Age(50))).unwrap();
        });

        // Second execute - should execute the new plan
        // If shutdown_status wasn't reset, this would immediately break
        // without executing the plan, leaving population at 1.
        context.execute();
        assert_eq!(context.get_current_time(), 2.0);
        assert_eq!(
            context.get_entity_count::<Person>(),
            2,
            "If this fails, shutdown_status was not properly reset"
        );
    }
}
