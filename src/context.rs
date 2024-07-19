use std::{
    any::{Any, TypeId},
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, HashSet, VecDeque},
    rc::Rc,
};

use derivative::Derivative;

pub trait Component: Any {
    fn init(context: &mut Context);
}

pub trait Plugin: Any {
    type DataContainer;

    fn get_data_container() -> Self::DataContainer;
}

#[macro_export]
macro_rules! define_plugin {
    ($plugin:ident, $data_container:ty, $default: expr) => {
        struct $plugin {}

        impl $crate::context::Plugin for $plugin {
            type DataContainer = $data_container;

            fn get_data_container() -> Self::DataContainer {
                $default
            }
        }
    };
}
pub use define_plugin;

pub struct PlanId {
    pub id: u64,
}

#[derive(Derivative)]
#[derivative(Eq, PartialEq, Debug)]
pub struct TimedPlan {
    pub time: f64,
    plan_id: u64,
    #[derivative(PartialEq = "ignore", Debug = "ignore")]
    pub callback: Box<dyn FnOnce(&mut Context)>,
}

impl Ord for TimedPlan {
    fn cmp(&self, other: &Self) -> Ordering {
        let time_ordering = self.time.partial_cmp(&other.time).unwrap().reverse();
        if time_ordering == Ordering::Equal {
            // Break time ties in order of plan id
            self.plan_id.cmp(&other.plan_id).reverse()
        } else {
            time_ordering
        }
    }
}

impl PartialOrd for TimedPlan {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug)]
struct PlanQueue {
    queue: BinaryHeap<TimedPlan>,
    invalid_set: HashSet<u64>,
    plan_counter: u64,
}

impl PlanQueue {
    pub fn new() -> PlanQueue {
        PlanQueue {
            queue: BinaryHeap::new(),
            invalid_set: HashSet::new(),
            plan_counter: 0,
        }
    }

    pub fn add_plan(&mut self, time: f64, callback: impl FnOnce(&mut Context) + 'static) -> PlanId {
        // Add plan to queue and increment counter
        let plan_id = self.plan_counter;
        self.queue.push(TimedPlan {
            time,
            plan_id,
            callback: Box::new(callback),
        });
        self.plan_counter += 1;
        PlanId { id: plan_id }
    }

    pub fn cancel_plan(&mut self, id: PlanId) {
        self.invalid_set.insert(id.id);
    }

    pub fn get_next_timed_plan(&mut self) -> Option<TimedPlan> {
        loop {
            let next_timed_plan = self.queue.pop();
            match next_timed_plan {
                Some(timed_plan) => {
                    if self.invalid_set.contains(&timed_plan.plan_id) {
                        self.invalid_set.remove(&timed_plan.plan_id);
                    } else {
                        return Some(timed_plan);
                    }
                }
                None => {
                    return None;
                }
            }
        }
    }
}

type Callback = dyn FnOnce(&mut Context);
type EventHandler<E> = dyn Fn(&mut Context, E);
pub struct Context {
    plan_queue: PlanQueue,
    callback_queue: VecDeque<Box<Callback>>,
    event_handlers: HashMap<TypeId, Box<dyn Any>>,
    immediate_event_handlers: HashMap<TypeId, Box<dyn Any>>,
    plugin_data: HashMap<TypeId, Box<dyn Any>>,
    time: f64,
}

impl Context {
    pub fn new() -> Context {
        Context {
            plan_queue: PlanQueue::new(),
            callback_queue: VecDeque::new(),
            event_handlers: HashMap::new(),
            immediate_event_handlers: HashMap::new(),
            plugin_data: HashMap::new(),
            time: 0.0,
        }
    }

    pub fn add_plan(&mut self, time: f64, callback: impl FnOnce(&mut Context) + 'static) -> PlanId {
        // TODO: Handle invalid times (past, NAN, etc)
        self.plan_queue.add_plan(time, callback)
    }

    pub fn cancel_plan(&mut self, id: PlanId) {
        self.plan_queue.cancel_plan(id);
    }

    pub fn queue_callback(&mut self, callback: impl FnOnce(&mut Context) + 'static) {
        self.callback_queue.push_back(Box::new(callback));
    }

    fn add_plugin<T: Plugin>(&mut self) {
        self.plugin_data
            .insert(TypeId::of::<T>(), Box::new(T::get_data_container()));
    }

    pub fn get_data_container_mut<T: Plugin>(&mut self) -> &mut T::DataContainer {
        let type_id = &TypeId::of::<T>();
        if !self.plugin_data.contains_key(type_id) {
            self.add_plugin::<T>();
        }
        let data_container = self
            .plugin_data
            .get_mut(type_id)
            .unwrap()
            .downcast_mut::<T::DataContainer>();
        match data_container {
            Some(x) => x,
            None => panic!("Plugin data container of incorrect type"),
        }
    }

    pub fn get_data_container<T: Plugin>(&self) -> Option<&T::DataContainer> {
        let type_id = &TypeId::of::<T>();
        if !self.plugin_data.contains_key(type_id) {
            return None;
        }
        let data_container = self
            .plugin_data
            .get(type_id)
            .unwrap()
            .downcast_ref::<T::DataContainer>();
        match data_container {
            Some(x) => Some(x),
            None => panic!("Plugin data container of incorrect type"),
        }
    }

    pub fn get_time(&self) -> f64 {
        self.time
    }

    pub fn add_component<T: Component>(&mut self) {
        T::init(self);
    }

    fn add_handlers<E: Copy + 'static>(
        event_handlers: &mut HashMap<TypeId, Box<dyn Any>>,
        callback: impl Fn(&mut Context, E) + 'static,
    ) {
        let callback_vec = event_handlers
            .entry(TypeId::of::<E>())
            .or_insert_with(|| Box::<Vec<Rc<EventHandler<E>>>>::default());
        let callback_vec: &mut Vec<Rc<EventHandler<E>>> = callback_vec.downcast_mut().unwrap();
        callback_vec.push(Rc::new(callback));
    }

    pub fn subscribe_to_event<E: Copy + 'static>(
        &mut self,
        callback: impl Fn(&mut Context, E) + 'static,
    ) {
        Self::add_handlers(&mut self.event_handlers, callback);
    }

    pub fn subscribe_immediately_to_event<E: Copy + 'static>(
        &mut self,
        callback: impl Fn(&mut Context, E) + 'static,
    ) {
        Self::add_handlers(&mut self.immediate_event_handlers, callback);
    }

    fn collect_callbacks<E: Copy + 'static>(
        event_handlers: &HashMap<TypeId, Box<dyn Any>>,
        event: E,
    ) -> Vec<Box<Callback>> {
        let mut callbacks_to_return = Vec::<Box<Callback>>::new();
        let callback_vec = event_handlers.get(&TypeId::of::<E>());
        if let Some(callback_vec) = callback_vec {
            let callback_vec: &Vec<Rc<EventHandler<E>>> = callback_vec.downcast_ref().unwrap();
            if !callback_vec.is_empty() {
                for callback in callback_vec {
                    let internal_callback = Rc::clone(callback);
                    callbacks_to_return
                        .push(Box::new(move |context| internal_callback(context, event)));
                }
            }
        }
        callbacks_to_return
    }

    pub fn release_event<E: Copy + 'static>(&mut self, event: E) {
        // Queue standard handlers
        for callback in Self::collect_callbacks(&self.event_handlers, event) {
            self.queue_callback(callback);
        }
        // Process immediate handlers
        for callback in Self::collect_callbacks(&self.immediate_event_handlers, event) {
            callback(self);
        }
    }

    pub fn execute(&mut self) {
        // Execute callbacks if there are any in the queue
        loop {
            let callback = self.callback_queue.pop_front();
            match callback {
                Some(callback) => callback(self),
                None => break,
            }
        }
        // Start plan loop
        loop {
            let timed_plan = self.plan_queue.get_next_timed_plan();
            match timed_plan {
                Some(timed_plan) => {
                    self.time = timed_plan.time;
                    (timed_plan.callback)(self);
                    loop {
                        let callback = self.callback_queue.pop_front();
                        match callback {
                            Some(callback) => callback(self),
                            None => break,
                        }
                    }
                }
                None => break,
            }
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
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    define_plugin!(ComponentA, u32, 0);

    impl ComponentA {
        fn increment_counter(context: &mut Context) {
            *(context.get_data_container_mut::<ComponentA>()) += 1;
        }
    }

    impl Component for ComponentA {
        fn init(context: &mut Context) {
            context.add_plan(1.0, Self::increment_counter);
        }
    }

    #[test]
    fn test_component_and_planning() {
        let mut context = Context::new();
        context.add_component::<ComponentA>();
        assert_eq!(context.get_time(), 0.0);
        assert_eq!(*context.get_data_container_mut::<ComponentA>(), 0);
        context.execute();
        assert_eq!(context.get_time(), 1.0);
        assert_eq!(*context.get_data_container_mut::<ComponentA>(), 1);
        let plan_to_cancel = context.add_plan(3.0, ComponentA::increment_counter);
        context.add_plan(2.0, ComponentA::increment_counter);
        context.cancel_plan(plan_to_cancel);
        context.execute();
        assert_eq!(context.get_time(), 2.0);
        assert_eq!(*context.get_data_container_mut::<ComponentA>(), 2);
    }

    #[derive(Copy, Clone)]
    struct Event {
        pub data: usize,
    }
    #[test]
    fn test_events() {
        let mut context = Context::new();

        let obs_data = Rc::new(RefCell::new(0));
        let immediate_obs_data = Rc::new(RefCell::new(0));

        let obs_data_clone = Rc::clone(&obs_data);
        context.subscribe_to_event::<Event>(move |_, event| {
            *obs_data_clone.borrow_mut() = event.data;
        });

        let immediate_obs_data_clone = Rc::clone(&immediate_obs_data);
        context.subscribe_immediately_to_event::<Event>(move |_, event| {
            *immediate_obs_data_clone.borrow_mut() = event.data;
        });

        context.release_event(Event { data: 1 });
        assert_eq!(*immediate_obs_data.borrow(), 1);

        context.execute();
        assert_eq!(*obs_data.borrow(), 1);
    }
}
