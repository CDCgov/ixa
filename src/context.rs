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
    plan_queue: PlanQueue<Box<Callback>>,
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
        if time.is_nan() || time.is_infinite() || time < self.current_time {
            panic!("Invalid time value");
        }
        self.plan_queue.add_plan(time, Box::new(callback))
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

#[cfg(test)]
mod tests {
    use super::*;

    define_data_plugin!(ComponentA, Vec<u32>, vec![]);

    fn add_plan(context: &mut Context, time: f64, value: u32) -> PlanId {
        context.add_plan(time, move |context| {
            context.get_data_container_mut::<ComponentA>().push(value);
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
    fn empty_context() {
        let mut context = Context::new();
        context.execute();
        assert_eq!(context.get_current_time(), 0.0);        
    }

    #[test]    
    fn timed_plan_only() {
        let mut context = Context::new();
        add_plan(&mut context, 1.0, 1);
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_container_mut::<ComponentA>(), vec![1]);        
    }

    #[test]
    fn callback_only() {
        let mut context = Context::new();
        context.queue_callback(|context| {
            context.get_data_container_mut::<ComponentA>().push(1);            
        });
        context.execute();
        assert_eq!(context.get_current_time(), 0.0);        
        assert_eq!(*context.get_data_container_mut::<ComponentA>(), vec![1]);
    }

    #[test]
    fn callback_before_timed_plan() {
        let mut context = Context::new();
        context.queue_callback(|context| {
            context.get_data_container_mut::<ComponentA>().push(1);            
        });
        add_plan(&mut context, 1.0, 2);
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);                
        assert_eq!(*context.get_data_container_mut::<ComponentA>(), vec![1, 2]);        
    }

    #[test]
    fn callback_adds_timed_plan() {
        let mut context = Context::new();
        context.queue_callback(|context| {
            context.get_data_container_mut::<ComponentA>().push(1);
            add_plan(context, 1.0, 2);
            context.get_data_container_mut::<ComponentA>().push(3);            
        });
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);                
        assert_eq!(*context.get_data_container_mut::<ComponentA>(), vec![1, 3, 2]);        
    }

    #[test]
    fn callback_adds_callback_and_timed_plan() {
        let mut context = Context::new();
        context.queue_callback(|context| {
            context.get_data_container_mut::<ComponentA>().push(1);
            add_plan(context, 1.0, 2);
            context.queue_callback(|context| {
                context.get_data_container_mut::<ComponentA>().push(4);
            });
            context.get_data_container_mut::<ComponentA>().push(3);            
        });
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_container_mut::<ComponentA>(), vec![1, 3, 4, 2]);        
    }

    #[test]
    fn timed_plan_adds_callback_and_timed_plan() {
        let mut context = Context::new();
        context.add_plan(1.0, |context| {
            context.get_data_container_mut::<ComponentA>().push(1);
            // We add the plan first, but the callback will fire first.
            add_plan(context, 2.0, 3);                
            context.queue_callback(|context| {
                context.get_data_container_mut::<ComponentA>().push(2);
            });
        });
        context.execute();
        assert_eq!(context.get_current_time(), 2.0);
        assert_eq!(*context.get_data_container_mut::<ComponentA>(), vec![1, 2, 3]);        
    }

    #[test]
    fn cancel_plan() {
        let mut context = Context::new();
        let to_cancel = add_plan(&mut context, 2.0, 1);
        context.add_plan(1.0, move |context| {
            context.cancel_plan(to_cancel);
        });
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_container_mut::<ComponentA>(), vec![]);                
    }

    #[test]
    fn add_plan_with_current_time() {
        let mut context = Context::new();
        context.add_plan(1.0, move |context| {
            context.get_data_container_mut::<ComponentA>().push(1);
            add_plan(context, 1.0, 2);
            context.queue_callback(|context| {
                context.get_data_container_mut::<ComponentA>().push(3);
            });
        });
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_container_mut::<ComponentA>(), vec![1, 3, 2]);                
    }        

    #[test]
    fn plans_at_same_time_fire_in_order() {
        let mut context = Context::new();
        add_plan(&mut context, 1.0, 1);
        add_plan(&mut context, 1.0, 2);
        context.execute();
        assert_eq!(context.get_current_time(), 1.0);
        assert_eq!(*context.get_data_container_mut::<ComponentA>(), vec![1, 2]);                
        
    }
}
