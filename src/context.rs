//! A manager for the state of a discrete-event simulation
//!
//! Defines a [`Context`] that is intended to provide the foundational mechanism
//! for storing and manipulating the state of a given simulation.
use std::any::{Any, TypeId};
use std::cell::OnceCell;
use std::collections::VecDeque;
use std::fmt::{Display, Formatter};
use std::rc::Rc;

use crate::data_plugin::DataPlugin;
use crate::execution_stats::{
    log_execution_statistics, print_execution_statistics, ExecutionProfilingCollector,
    ExecutionStatistics,
};
use crate::plan::{PlanId, Queue};
#[cfg(feature = "progress_bar")]
use crate::progress::update_timeline_progress;
#[cfg(feature = "debugger")]
use crate::{debugger::enter_debugger, plan::PlanSchedule};
use crate::{get_data_plugin_count, trace, warn, ContextPeopleExt, HashMap, HashMapExt};

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
    data_plugins: Vec<OnceCell<Box<dyn Any>>>,
    #[cfg(feature = "debugger")]
    breakpoints_scheduled: Queue<Box<Callback>, ExecutionPhase>,
    current_time: f64,
    start_time: Option<f64>,
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
            data_plugins,
            #[cfg(feature = "debugger")]
            breakpoints_scheduled: Queue::new(),
            current_time: 0.0,
            start_time: None,
            shutdown_requested: false,
            #[cfg(feature = "debugger")]
            break_requested: false,
            #[cfg(feature = "debugger")]
            breakpoints_enabled: true,
            execution_profiler: ExecutionProfilingCollector::new(),
            print_execution_statistics: false,
        }
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
            !time.is_nan()
                && !time.is_infinite()
                && time >= self.current_time,
            "Time {time} is invalid: must be finite and not less than the current time ({}). Consider calling set_start_time() before scheduling plans.",
            self.current_time
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
    #[allow(clippy::missing_panics_doc)]
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
    /// Returns the current time    #[must_use]
    pub fn get_current_time(&self) -> f64 {
        self.current_time
    }

    /// Set the start time for the simulation. Must be finite.
    ///
    /// This should be called before `Context.execute()`.
    /// Can be called only once.
    ///
    /// # Panics
    ///
    /// Panics if `start_time` is infinite or NaN.
    pub fn set_start_time(&mut self, start_time: f64) {
        assert!(
            !start_time.is_nan() && !start_time.is_infinite(),
            "Start time {start_time} must be finite"
        );
        assert!(
            self.start_time.is_none(),
            "Start time has already been set. It can only be set once."
        );
        if let Some(next_time) = self.plan_queue.next_time() {
            assert!(
                start_time >= next_time,
                "Start time {} is earlier than the earliest scheduled plan time {}. Remove or reschedule existing plans first.",
                start_time,
                next_time
            );
        }
        self.start_time = Some(start_time);
        self.current_time = start_time;
    }

    /// Get the start time that was set via `set_start_time`, or `None` if not set.
    #[must_use]
    pub fn get_start_time(&self) -> Option<f64> {
        self.start_time
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

        // Initialize `self.current_time` based on start_time or earliest plan
        if let Some(start_time) = self.start_time {
            self.current_time = start_time;
        } else {
            self.current_time = match self.plan_queue.next_time() {
                Some(t) if t < 0.0 => t,
                _ => 0.0,
            };
        }

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
    use std::cell::RefCell;

    use ixa_derive::IxaEvent;

    use super::*;
    use crate::define_data_plugin;

    define_data_plugin!(ComponentA, Vec<u32>, vec![]);

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
    fn get_current_time_initializes_to_min_negative_plan() {
        let mut context = Context::new();
        context.set_start_time(-5.0);
        add_plan(&mut context, -5.0, 1);
        add_plan(&mut context, 10.0, 2);
        context.execute();
        assert_eq!(context.get_current_time(), 10.0);
        assert_eq!(*context.get_data_mut(ComponentA), vec![1, 2]);
    }

    #[test]
    fn get_current_time_initializes_to_zero_when_all_positive() {
        let mut context = Context::new();
        add_plan(&mut context, 1.0, 1);
        add_plan(&mut context, 2.0, 2);
        context.execute();
        assert_eq!(context.get_current_time(), 2.0);
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
    fn negative_periodic_plan() {
        let mut context = Context::new();
        context.set_start_time(-2.0);
        let call_count = Rc::new(RefCell::new(0));
        let call_count_clone = Rc::clone(&call_count);

        context.add_plan(-2.0, move |context| {
            *call_count_clone.borrow_mut() += 1;
            context.get_data_mut(ComponentA).push(-2i32 as u32);
        });
        context.execute();
        assert_eq!(context.get_current_time(), -2.0);
        assert_eq!(*call_count.borrow(), 1);
    }

    #[test]
    #[should_panic(expected = "Start time has already been set. It can only be set once.")]
    fn set_start_time_only_once() {
        let mut context = Context::new();
        context.set_start_time(1.0);
        context.set_start_time(2.0);
    }

    #[test]
    #[should_panic(expected = "Start time 0 is earlier than the earliest scheduled plan time 1")]
    fn set_start_time_rejects_earlier_than_existing_plans() {
        let mut context = Context::new();
        add_plan(&mut context, 1.0, 1);
        context.set_start_time(0.0);
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
}
