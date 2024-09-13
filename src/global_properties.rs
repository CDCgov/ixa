use crate::context::Context;
use crate::define_data_plugin;
use std::any::{Any, TypeId};
use std::collections::HashMap;

#[macro_export]
macro_rules! define_global_property {
    ($global_property:ident, $value:ty) => {
        #[derive(Copy, Clone)]
        pub struct $global_property;

        impl $crate::global_properties::GlobalProperty for $global_property {
            type Value = $value;
        }
    };
}

pub trait GlobalProperty: Any {
    type Value: Any;
}

pub use define_global_property;

struct GlobalPropertiesDataContainer {
    global_property_container: HashMap<TypeId, Box<dyn Any>>,
}

define_data_plugin!(
    GlobalPropertiesPlugin,
    GlobalPropertiesDataContainer,
    GlobalPropertiesDataContainer {
        global_property_container: HashMap::default(),
    }
);

pub trait ContextGlobalPropertiesExt {
    fn set_global_property_value<T: GlobalProperty + 'static>(&mut self, property: T, value: T::Value);
    fn get_global_property_value<T: GlobalProperty + 'static>(&self, property: T) -> &T::Value;
}

impl GlobalPropertiesDataContainer {
    fn set_global_property_value<T: GlobalProperty + 'static>(&mut self, _property: T, value: T::Value) {
        let _data_container = self
            .global_property_container
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(value));
    }

    fn get_global_property_value<T: GlobalProperty + 'static>(&self, _property: T) -> &T::Value {
        let data_container = self
            .global_property_container
            .get(&TypeId::of::<T>())
            .expect("Global property not initialized");
        data_container.downcast_ref::<T::Value>().unwrap()
    }
}

impl ContextGlobalPropertiesExt for Context {
    fn set_global_property_value<T: GlobalProperty + 'static>(&mut self, property: T, value: T::Value) {
        let data_container = self.get_data_container_mut(GlobalPropertiesPlugin);
        data_container.set_global_property_value(property, value)
    }

    fn get_global_property_value<T: GlobalProperty + 'static>(
        &self,
        property: T,
    ) -> &T::Value {
        let data_container = self.get_data_container(GlobalPropertiesPlugin).unwrap();
        data_container.get_global_property_value(property)
    }
}
