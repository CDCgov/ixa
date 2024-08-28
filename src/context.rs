//! A manager for the state of a discrete-event simulation
//!
//! Defines a `Context` that is intended to provide the foundational mechanism
//! for storing and manipulating the state of a given simulation.
use std::{
    any::{Any, TypeId},
    collections::{HashMap, VecDeque},
    rc::Rc,
};

use crate::plan::{Id, Queue};

/// The common callback used by multiple `Context` methods for future events
type Callback = dyn FnOnce(&mut Context);

/// A handler for an event type `E`
type EventHandler<E> = dyn Fn(&mut Context, E);

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
/// current time. This allows modules to schedule actions for immediate
/// execution but outside of the current iteration of the event loop.
///
/// Modules can also emit 'events' that other modules can subscribe to handle by
/// event type. This allows modules to broadcast that specific things have
/// occurred and have other modules take turns reacting to these occurrences.
///
pub struct Context {
    plan_queue: Queue<Box<Callback>>,
    callback_queue: VecDeque<Box<Callback>>,
    event_handlers: HashMap<TypeId, Box<dyn Any>>,
    data_plugins: HashMap<TypeId, Box<dyn Any>>,
    current_time: f64,
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
        }
    }

    /// Register to handle emission of events of type E
    ///
    /// Handlers will be called upon event emission in order of subscription as
    /// queued `Callback`s with the appropriate event.
    #[allow(clippy::missing_panics_doc)]
    pub fn subscribe_to_event<E: Copy + 'static>(
        &mut self,
        handler: impl Fn(&mut Context, E) + 'static,
    ) {
        let handler_vec = self
            .event_handlers
            .entry(TypeId::of::<E>())
            .or_insert_with(|| Box::<Vec<Rc<EventHandler<E>>>>::default());
        let handler_vec: &mut Vec<Rc<EventHandler<E>>> = handler_vec.downcast_mut().unwrap();
        handler_vec.push(Rc::new(handler));
    }

    /// Emit and event of type E to be handled by registered receivers
    ///
    /// Receivers will handle events in the order that they have subscribed and
    /// are queued as callbacks
    #[allow(clippy::missing_panics_doc)]
    pub fn emit_event<E: Copy + 'static>(&mut self, event: E) {
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

    /// Add a plan to the future event list at the specified time
    ///
    /// Returns an `Id` for the newly-added plan that can be used to cancel it
    /// if needed.
    /// # Panics
    ///
    /// Panics if time is in the past, infinite, or NaN.
    pub fn add_plan(&mut self, time: f64, callback: impl FnOnce(&mut Context) + 'static) -> Id {
        assert!(!time.is_nan() && !time.is_infinite() && time >= self.current_time);
        self.plan_queue.add_plan(time, Box::new(callback))
    }

    /// Cancel a plan that has been added to the queue
    ///
    /// # Panics
    ///
    /// This function panics if you cancel a plan which has already been
    /// cancelled or executed.
    pub fn cancel_plan(&mut self, id: &Id) {
        self.plan_queue.cancel_plan(id);
    }

    /// Add a `Callback` to the queue to be executed before the next plan
    pub fn queue_callback(&mut self, callback: impl FnOnce(&mut Context) + 'static) {
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

    /// Get the current time in the simulation
    ///
    /// Returns the current time
    #[must_use]
    pub fn get_current_time(&self) -> f64 {
        self.current_time
    }

    /// Execute the simulation until the plan and callback queues are empty
    pub fn execute(&mut self) {
        // Start plan loop
        loop {
            // If there is a callback, run it.
            if let Some(callback) = self.callback_queue.pop_front() {
                callback(self);
                continue;
            }

            // There aren't any callbacks, so look at the first plan.
            if let Some(plan) = self.plan_queue.get_next_plan() {
                self.current_time = plan.time;
                (plan.data)(self);
            } else {
                // OK, there aren't any plans, so we're done.
                break;
            }
        }
    }
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
mod tests {
    use std::cell::RefCell;

    use super::*;

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

    fn add_plan(context: &mut Context, time: f64, value: u32) -> Id {
        context.add_plan(time, move |context| {
            context.get_data_container_mut(ComponentA).push(value);
        })
    }

    #[test]
    #[should_panic]
    fn negative_plan_time() {
        let mut context = Context::new();
        add_plan(&mut context, -1.0, 0);
    }

    #[test]
    #[should_panic]
    fn infinite_plan_time() {
        let mut context = Context::new();
        add_plan(&mut context, f64::INFINITY, 0);
    }

    #[test]
    #[should_panic]
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
        assert_eq!(*context.get_data_container_mut(ComponentA), vec![]);
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

    #[derive(Copy, Clone)]
    struct Event {
        pub data: usize,
    }

    #[derive(Copy, Clone)]
    struct Event2 {
        pub data: usize,
    }

    #[test]
    fn simple_event() {
        let mut context = Context::new();
        let obs_data = Rc::new(RefCell::new(0));
        let obs_data_clone = Rc::clone(&obs_data);

        context.subscribe_to_event::<Event>(move |_, event| {
            *obs_data_clone.borrow_mut() = event.data;
        });

        context.emit_event(Event { data: 1 });
        context.execute();
        assert_eq!(*obs_data.borrow(), 1);
    }

    #[test]
    fn multiple_events() {
        let mut context = Context::new();
        let obs_data = Rc::new(RefCell::new(0));
        let obs_data_clone = Rc::clone(&obs_data);

        context.subscribe_to_event::<Event>(move |_, event| {
            *obs_data_clone.borrow_mut() += event.data;
        });

        context.emit_event(Event { data: 1 });
        context.emit_event(Event { data: 2 });
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

        context.subscribe_to_event::<Event>(move |_, event| {
            *obs_data1_clone.borrow_mut() = event.data;
        });
        context.subscribe_to_event::<Event>(move |_, event| {
            *obs_data2_clone.borrow_mut() = event.data;
        });
        context.emit_event(Event { data: 1 });
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

        context.subscribe_to_event::<Event>(move |_, event| {
            *obs_data1_clone.borrow_mut() = event.data;
        });
        context.subscribe_to_event::<Event2>(move |_, event| {
            *obs_data2_clone.borrow_mut() = event.data;
        });
        context.emit_event(Event { data: 1 });
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

        context.emit_event(Event { data: 1 });
        context.subscribe_to_event::<Event>(move |_, event| {
            *obs_data_clone.borrow_mut() = event.data;
        });

        context.execute();
        assert_eq!(*obs_data.borrow(), 0);
    }
}
