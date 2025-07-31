use crate::{HashSet, PluginContext};
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

/// A collection of `TypeId`s of all `DataPlugin` types linked into the code.
static DATA_PLUGINS: LazyLock<Mutex<RefCell<HashSet<TypeId>>>> =
    LazyLock::new(|| Mutex::new(RefCell::new(HashSet::default())));

pub fn add_data_plugin_to_registry<T: DataPlugin>() {
    DATA_PLUGINS
        .lock()
        .unwrap()
        .borrow_mut()
        .insert(TypeId::of::<T>());
}

pub fn get_data_plugin_ids() -> Vec<TypeId> {
    DATA_PLUGINS
        .lock()
        .unwrap()
        .borrow()
        .iter()
        .copied()
        .collect()
}

pub fn get_data_plugin_count() -> usize {
    DATA_PLUGINS.lock().unwrap().borrow().len()
}

/// Global data plugin index counter, keeps track of the index that will be assigned to the next
/// data plugin that requests an index.
///
/// Instead of storing data plugins in a `HashMap` in `Context`, we store them in a vector. To fetch
/// the data plugin, we ask the data plugin type for the index into `Context::data_plugins` at
/// which an instance of the data plugin type should be stored. Accessing a data plugin, then, is
/// just an index into an array.
static NEXT_DATA_PLUGIN_INDEX: Mutex<usize> = Mutex::new(0);

/// Acquires a global lock on the next available plugin index, but only increments it if we
/// successfully initialize the provided index. (Must be `pub`, as it's called from within a macro.)
pub fn initialize_data_plugin_index(plugin_index: &AtomicUsize) -> usize {
    // Acquire a global lock.
    let mut guard = NEXT_DATA_PLUGIN_INDEX.lock().unwrap();
    let candidate = *guard;

    // Try to claim the candidate index. Here we guard against the potential race condition that
    // another instance of this plugin in another thread just initialized the index prior to us
    // obtaining the lock. If the index has been initialized beneath us, we do not update
    // `NEXT_DATA_PLUGIN_INDEX`, we just return the value `plugin_index` was initialized to.
    match plugin_index.compare_exchange(usize::MAX, candidate, Ordering::AcqRel, Ordering::Acquire)
    {
        Ok(_) => {
            // We won the race — increment the global next plugin index and return the new index
            *guard += 1;
            candidate
        }
        Err(existing) => {
            // Another thread beat us — don’t increment the global next plugin index,
            // just return existing
            existing
        }
    }
}

/// A trait for objects that can provide data containers to be held by `Context`
pub trait DataPlugin: Any {
    type DataContainer;

    fn init(context: &impl PluginContext) -> Self::DataContainer;

    /// Returns the index into `Context::data_plugins`, the vector of data plugins, where
    /// the instance of this data plugin can be found.
    fn index_within_context() -> usize;
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

            fn index_within_context() -> usize {
                // This static must be initialized with a compile-time constant expression.
                // We use `usize::MAX` as a sentinel to mean "uninitialized". This
                // static variable is shared among all instances of this data plugin type.
                static INDEX: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(usize::MAX);

                // Fast path: already initialized.
                let index = INDEX.load(std::sync::atomic::Ordering::Relaxed);
                if index != usize::MAX {
                    return index;
                }

                // Slow path: initialize it.
                $crate::initialize_data_plugin_index(&INDEX)
            }
        }

        $crate::paste::paste! {
            $crate::ctor::declarative::ctor!{
                #[ctor]
                fn [<_register_plugin_$data_plugin:snake>]() {
                    $crate::add_data_plugin_to_registry::<$data_plugin>()
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Context;

    #[test]
    #[should_panic(
        expected = "No data plugin found with index = 1000. You must use the `define_data_plugin!` macro to create a data plugin."
    )]
    fn test_wrong_data_plugin_impl_index_oob() {
        // Suppose a user doesn't use the `define_data_plugin` macro and tries to implement it
        // themselves. What error modes are possible? First lets try an obviously out-of-bounds
        // index.
        struct MyDataPlugin;
        impl DataPlugin for MyDataPlugin {
            type DataContainer = Vec<u32>;

            fn init(_context: &impl PluginContext) -> Self::DataContainer {
                vec![]
            }

            fn index_within_context() -> usize {
                1000 // arbitrarily out of bounds
            }
        }

        let context = Context::new();
        let container = context.get_data(MyDataPlugin);
        println!("{}", container.len());
    }

    // We attempt a collision with this plugin
    define_data_plugin!(LegitDataPlugin, Vec<u32>, vec![]);
    #[should_panic(
        expected = "TypeID does not match data plugin type. You must use the `define_data_plugin!` macro to create a data plugin."
    )]
    #[test]
    fn test_wrong_data_plugin_impl_wrong_type() {
        // Suppose a user doesn't use the `define_data_plugin` macro and tries
        // to implement it themselves. What error modes are possible? Here we
        // test for an index collision.
        struct MyOtherDataPlugin;
        impl DataPlugin for MyOtherDataPlugin {
            type DataContainer = Vec<u8>;

            fn init(_context: &impl PluginContext) -> Self::DataContainer {
                vec![]
            }

            fn index_within_context() -> usize {
                // Several plugins are registered in a test context, so an index of 1 should
                // collide with another plugin of a different type.
                LegitDataPlugin::index_within_context()
            }
        }

        let context = Context::new();
        // Make sure the legit plugin is initialized first.
        let _ = context.get_data(LegitDataPlugin);

        let container = context.get_data(MyOtherDataPlugin); // Panic here
                                                             // Some arbitrary code involving `container`.
        println!("{}", container.len());
    }
}
