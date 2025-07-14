use crate::{HashSet, PluginContext};
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::sync::{LazyLock, Mutex};

static DATA_PLUGINS: LazyLock<Mutex<RefCell<HashSet<TypeId>>>> =
    LazyLock::new(|| Mutex::new(RefCell::new(HashSet::default())));

pub fn add_plugin_to_registry<T: DataPlugin>() {
    DATA_PLUGINS
        .lock()
        .unwrap()
        .borrow_mut()
        .insert(TypeId::of::<T>());
}

pub fn get_plugin_ids() -> Vec<TypeId> {
    DATA_PLUGINS
        .lock()
        .unwrap()
        .borrow()
        .iter()
        .copied()
        .collect()
}

/// A trait for objects that can provide data containers to be held by `Context`
pub trait DataPlugin: Any {
    type DataContainer;

    fn init(context: &impl PluginContext) -> Self::DataContainer;
}

/// Helper for `define_data_plugin`
#[macro_export]
macro_rules! __define_data_plugin {
    ($data_plugin:ident, $data_container:ty, |$ctx:ident| $body:expr) => {
        struct $data_plugin;

        impl $crate::DataPlugin for $data_plugin {
            type DataContainer = $data_container;

            fn init($ctx: &impl $crate::PluginContext) -> Self::DataContainer {
                $body
            }
        }

        $crate::paste::paste! {
            #[$crate::ctor::ctor]
            fn [<_register_plugin_$data_plugin:snake>]() {
                $crate::add_plugin_to_registry::<$data_plugin>()
            }
        }
    };
}

/// Defines a new type for storing data in Context.
#[macro_export]
macro_rules! define_data_plugin {
    ($data_plugin:ident, $data_container:ty, |$ctx:ident| $body:expr) => {
        $crate::__define_data_plugin!($data_plugin, $data_container, |$ctx| $body);
    };

    ($data_plugin:ident, $data_container:ty, $default: expr) => {
        $crate::__define_data_plugin!($data_plugin, $data_container, |_context| $default);
    };
}
