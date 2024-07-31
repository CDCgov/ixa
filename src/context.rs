use std::{
    any::{Any, TypeId},
    collections::{HashMap, VecDeque},
};

pub trait DataPlugin: Any {
    type DataContainer;

    fn create_data_container() -> Self::DataContainer;
}

#[macro_export]
macro_rules! define_data_plugin {
    ($plugin:ident, $data_container:ty, $default: expr) => {
        struct $plugin {}

        impl $crate::context::DataPlugin for $plugin {
            type DataContainer = $data_container;

            fn create_data_container() -> Self::DataContainer {
                $default
            }
        }
    };
}
pub use define_data_plugin;

use crate::plan::{PlanId, PlanQueue};

type Callback = dyn FnOnce(&mut Context);
pub struct Context {
    plan_queue: PlanQueue,
    callback_queue: VecDeque<Box<Callback>>,
    data_plugins: HashMap<TypeId, Box<dyn Any>>,
    current_time: f64,
}

impl Context {
    pub fn new() -> Context {
        Context {
            plan_queue: PlanQueue::new(),
            callback_queue: VecDeque::new(),
            data_plugins: HashMap::new(),
            current_time: 0.0,
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

    fn add_plugin<T: DataPlugin>(&mut self) {
        self.data_plugins
            .insert(TypeId::of::<T>(), Box::new(T::create_data_container()));
    }

    pub fn get_data_container_mut<T: DataPlugin>(&mut self) -> &mut T::DataContainer {
        let type_id = &TypeId::of::<T>();
        if !self.data_plugins.contains_key(type_id) {
            self.add_plugin::<T>();
        }
        self.data_plugins
            .get_mut(type_id)
            .unwrap()
            .downcast_mut::<T::DataContainer>()
            .unwrap()
    }

    pub fn get_data_container<T: DataPlugin>(&self) -> Option<&T::DataContainer> {
        let type_id = &TypeId::of::<T>();
        if !self.data_plugins.contains_key(type_id) {
            return None;
        }
        self.data_plugins
            .get(type_id)
            .unwrap()
            .downcast_ref::<T::DataContainer>()
    }

    pub fn get_current_time(&self) -> f64 {
        self.current_time
    }

    pub fn execute(&mut self) {
        // Start plan loop
        loop {
            // If there is a callback, run it.
            if let Some(callback) = self.callback_queue.pop_front() {
                callback(self);
                continue;
            }

            // There aren't any callbacks, so look at the first timed plan.
            if let Some(timed_plan) = self.plan_queue.get_next_timed_plan() {
                self.current_time = timed_plan.time;
                (timed_plan.callback)(self);
            } else {
                // OK, there aren't any timed plans, so we're done.
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

#[cfg(test)]
mod tests {
    use super::*;

    define_data_plugin!(ComponentA, u32, 0);

    impl ComponentA {
        fn increment_counter(context: &mut Context) {
            *(context.get_data_container_mut::<ComponentA>()) += 1;
        }

        fn init(context: &mut Context) {
            context.add_plan(1.0, Self::increment_counter);
        }
    }

    #[test]
    fn test_component_and_planning() {
        let mut context = Context::new();
        ComponentA::init(&mut context);
        assert_eq!(context.get_current_time(), 0.0);
        assert_eq!(*context.get_data_container_mut::<ComponentA>(), 0);
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_container_mut::<ComponentA>(), 1);
        let plan_to_cancel = context.add_plan(3.0, ComponentA::increment_counter);
        context.add_plan(2.0, ComponentA::increment_counter);
        context.cancel_plan(plan_to_cancel);
        context.execute();
        assert_eq!(context.get_current_time(), 2.0);
        assert_eq!(*context.get_data_container_mut::<ComponentA>(), 2);
    }
}
