//! A manager for the state of a discrete-event simulation
//!
//! Defines a `Context` that is intended to provide the foundational mechanism
//! for storing and manipulating the state of a given simulation.
use crate::{HashMap, HashMapExt};
use crate::debugger::enter_debugger;
use crate::plan::{PlanId, Queue};
use crate::trace;
use std::{
    any::{Any, TypeId},
    collections::VecDeque,
    rc::Rc,
};

/// The common callback used by multiple `Context` methods for future events
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
#[derive(PartialEq, Eq, Ord, Clone, Copy, PartialOrd)]
pub enum ExecutionPhase {
    First,
    Normal,
    Last,
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
/// take turns manipulating the Context through a mutable reference. Modules
/// store data in the simulation using the `DataPlugin` trait that allows them
/// to retrieve data by type.
///
/// The future event list of the simulation is a queue of `Callback` objects -
/// called `plans` - that will assume control of the Context at a future point
/// in time and execute the logic in the associated `FnOnce(&mut Context)`
/// closure. Modules can add plans to this queue through the `Context`.
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
    data_plugins: HashMap<TypeId, Box<dyn Any>>,
    current_time: f64,
    shutdown_requested: bool,
}

impl Context {
    /// Create a new empty `Context`
    #[must_use]
    pub fn new() -> Context {
        Context {
            plan_queue: Queue::new(),
            callback_queue: VecDeque::new(),
            event_handlers: HashMap::new(),
            data_plugins: HashMap::new(),
            current_time: 0.0,
            shutdown_requested: false,
        }
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

    /// Emit and event of type E to be handled by registered receivers
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
    /// Returns a `PlanId` for the newly-added plan that can be used to cancel it
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
    /// Returns a `PlanId` for the newly-added plan that can be used to cancel it
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
        self.plan_queue.cancel_plan(plan_id);
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
    /// `DataPlugin`
    ///
    /// If the data container has not been already added to the `Context` then
    /// this function will use the `DataPlugin::create_data_container` method
    /// to construct a new data container and store it in the `Context`.
    ///
    /// Returns a mutable reference to the data container
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn get_data_container_mut<T: DataPlugin>(
        &mut self,
        _data_plugin: T,
    ) -> &mut T::DataContainer {
        self.data_plugins
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(T::create_data_container()))
            .downcast_mut::<T::DataContainer>()
            .unwrap() // Will never panic as data container has the matching type
    }

    /// Retrieve a reference to the data container associated with a
    /// `DataPlugin`
    ///
    /// Returns a reference to the data container if it exists or else `None`
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn get_data_container<T: DataPlugin>(&self, _data_plugin: T) -> Option<&T::DataContainer> {
        if let Some(data) = self.data_plugins.get(&TypeId::of::<T>()) {
            data.downcast_ref::<T::DataContainer>()
        } else {
            None
        }
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

    /// Execute the simulation until the plan and callback queues are empty
    pub fn execute(&mut self) {
        trace!("entering event loop");
        // Start plan loop
        loop {
            if self.shutdown_requested {
                break;
            }

            // If there is a callback, run it.
            if let Some(callback) = self.callback_queue.pop_front() {
                trace!("calling callback");
                callback(self);
                continue;
            }

            // There aren't any callbacks, so look at the first plan.
            if let Some(plan) = self.plan_queue.get_next_plan() {
                trace!("calling plan at {}", plan.time);
                self.current_time = plan.time;
                (plan.data)(self);
            } else {
                trace!("No callbacks or plans; exiting event loop");
                // OK, there aren't any plans, so we're done.
                break;
            }
        }
    }
}

// TODO(cym4@cdc.gov): This is a temporary hack to let you
// run a plan with mutable references to both the context
// and a plugin's data. In the future we hope to make a
// convenient public API for this, which is why it's not
// public now.
pub(crate) fn run_with_plugin<T: DataPlugin>(
    context: &mut Context,
    f: impl Fn(&mut Context, &mut T::DataContainer),
) {
    // Temporarily take the data container out of context so that
    // we can operate on context.
    let mut data_container_box = context.data_plugins.remove(&TypeId::of::<T>()).unwrap();
    let data_container = data_container_box
        .downcast_mut::<T::DataContainer>()
        .unwrap();

    // Call the function.
    f(context, data_container);

    // Put the data container back into context.
    context
        .data_plugins
        .insert(TypeId::of::<T>(), data_container_box);
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

/// A trait for objects that can provide data containers to be held by `Context`
pub trait DataPlugin: Any {
    type DataContainer;

    fn create_data_container() -> Self::DataContainer;
}

/// Defines a new type for storing data in Context.
#[macro_export]
macro_rules! define_data_plugin {
    ($plugin:ident, $data_container:ty, $default: expr) => {
        struct $plugin;

        impl $crate::context::DataPlugin for $plugin {
            type DataContainer = $data_container;

            fn create_data_container() -> Self::DataContainer {
                $default
            }
        }
    };
}
pub use define_data_plugin;

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use ixa_derive::IxaEvent;

    define_data_plugin!(ComponentA, Vec<u32>, vec![]);

    #[test]
    fn empty_context() {
        let mut context = Context::new();
        context.execute();
        assert_eq!(context.get_current_time(), 0.0);
    }

    #[test]
    fn get_data_container() {
        let mut context = Context::new();
        context.get_data_container_mut(ComponentA).push(1);
        assert_eq!(*context.get_data_container(ComponentA).unwrap(), vec![1],);
    }

    #[test]
    fn get_uninitialized_data_container() {
        let context = Context::new();
        assert!(context.get_data_container(ComponentA).is_none());
    }

    fn add_plan(context: &mut Context, time: f64, value: u32) -> PlanId {
        context.add_plan(time, move |context| {
            context.get_data_container_mut(ComponentA).push(value);
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
                context.get_data_container_mut(ComponentA).push(value);
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
        assert_eq!(*context.get_data_container_mut(ComponentA), vec![1]);
    }

    #[test]
    fn callback_only() {
        let mut context = Context::new();
        context.queue_callback(|context| {
            context.get_data_container_mut(ComponentA).push(1);
        });
        context.execute();
        assert_eq!(context.get_current_time(), 0.0);
        assert_eq!(*context.get_data_container_mut(ComponentA), vec![1]);
    }

    #[test]
    fn callback_before_timed_plan() {
        let mut context = Context::new();
        context.queue_callback(|context| {
            context.get_data_container_mut(ComponentA).push(1);
        });
        add_plan(&mut context, 1.0, 2);
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_container_mut(ComponentA), vec![1, 2]);
    }

    #[test]
    fn callback_adds_timed_plan() {
        let mut context = Context::new();
        context.queue_callback(|context| {
            context.get_data_container_mut(ComponentA).push(1);
            add_plan(context, 1.0, 2);
            context.get_data_container_mut(ComponentA).push(3);
        });
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_container_mut(ComponentA), vec![1, 3, 2]);
    }

    #[test]
    fn callback_adds_callback_and_timed_plan() {
        let mut context = Context::new();
        context.queue_callback(|context| {
            context.get_data_container_mut(ComponentA).push(1);
            add_plan(context, 1.0, 2);
            context.queue_callback(|context| {
                context.get_data_container_mut(ComponentA).push(4);
            });
            context.get_data_container_mut(ComponentA).push(3);
        });
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(
            *context.get_data_container_mut(ComponentA),
            vec![1, 3, 4, 2]
        );
    }

    #[test]
    fn timed_plan_adds_callback_and_timed_plan() {
        let mut context = Context::new();
        context.add_plan(1.0, |context| {
            context.get_data_container_mut(ComponentA).push(1);
            // We add the plan first, but the callback will fire first.
            add_plan(context, 2.0, 3);
            context.queue_callback(|context| {
                context.get_data_container_mut(ComponentA).push(2);
            });
        });
        context.execute();
        assert_eq!(context.get_current_time(), 2.0);
        assert_eq!(*context.get_data_container_mut(ComponentA), vec![1, 2, 3]);
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
        assert_eq!(*context.get_data_container_mut(ComponentA), test_vec);
    }

    #[test]
    fn add_plan_with_current_time() {
        let mut context = Context::new();
        context.add_plan(1.0, move |context| {
            context.get_data_container_mut(ComponentA).push(1);
            add_plan(context, 1.0, 2);
            context.queue_callback(|context| {
                context.get_data_container_mut(ComponentA).push(3);
            });
        });
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_container_mut(ComponentA), vec![1, 3, 2]);
    }

    #[test]
    fn plans_at_same_time_fire_in_order() {
        let mut context = Context::new();
        add_plan(&mut context, 1.0, 1);
        add_plan(&mut context, 1.0, 2);
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_container_mut(ComponentA), vec![1, 2]);
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
        assert_eq!(
            *context.get_data_container_mut(ComponentA),
            vec![3, 4, 1, 2, 5, 6]
        );
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
        assert_eq!(*context.get_data_container_mut(ComponentA), vec![1]);
    }

    #[test]
    fn shutdown_cancels_callbacks() {
        let mut context = Context::new();
        add_plan(&mut context, 1.0, 1);
        context.add_plan(1.5, |context| {
            // Note that we add the callback *before* we call shutdown
            // but shutdown cancels everything.
            context.queue_callback(|context| {
                context.get_data_container_mut(ComponentA).push(3);
            });
            context.shutdown();
        });
        context.execute();
        assert_eq!(context.get_current_time(), 1.5);
        assert_eq!(*context.get_data_container_mut(ComponentA), vec![1]);
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
                context.get_data_container_mut(ComponentA).push(time as u32);
            },
            ExecutionPhase::Last,
        );
        context.add_plan(1.0, move |_context| {});
        context.add_plan(1.5, move |_context| {});
        context.execute();
        assert_eq!(context.get_current_time(), 2.0);

        assert_eq!(
            *context.get_data_container(ComponentA).unwrap(),
            vec![0, 1, 2]
        ); // time 0.0, 1.0, and 2.0
    }
}
