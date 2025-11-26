use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

use crate::{HashSet, PluginContext};

/// A collection of [`TypeId`]s of all [`DataPlugin`] types linked into the code.
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
/// Instead of storing data plugins in a [`HashMap`] in [`Context`], we store them in a vector. To fetch
/// the data plugin, we ask the data plugin type for the index into [`Context::data_plugins`] at
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
    // For a justification of the data ordering, see:
    //     https://github.com/CDCgov/ixa/pull/477#discussion_r2244302872
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

/// A trait for objects that can provide data containers to be held by [`Context`](crate::Context)
pub trait DataPlugin: Any {
    type DataContainer;

    fn init<C: PluginContext>(context: &C) -> Self::DataContainer;

    /// Returns the index into `Context::data_plugins`, the vector of data plugins, where
    /// the instance of this data plugin can be found.
    fn index_within_context() -> usize;
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};
    use std::thread;

    use super::*;
    use crate::{define_data_plugin, Context};

    // We attempt an out-of-bounds index with a plugin
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

            fn init<C: PluginContext>(_context: &C) -> Self::DataContainer {
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

    // We attempt a collision with a plugin
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

            fn init<C: PluginContext>(_context: &C) -> Self::DataContainer {
                vec![]
            }

            fn index_within_context() -> usize {
                // Several plugins are registered in a test context, so an index of 1 should
                // collide with another plugin of a different type.
                LegitDataPlugin::index_within_context()
            }
        }

        let context = Context::new();
        // Make sure the legit plugin is initialized first
        let _ = context.get_data(LegitDataPlugin);

        // Panics here:
        let container = context.get_data(MyOtherDataPlugin);
        // Some arbitrary code involving `container`
        println!("{}", container.len());
    }

    // Test thread safety of `initialize_data_plugin_index`.
    #[test]
    fn test_multithreaded_plugin_init() {
        struct DataPluginContainerA;
        define_data_plugin!(DataPluginA, DataPluginContainerA, DataPluginContainerA);
        struct DataPluginContainerB;
        define_data_plugin!(DataPluginB, DataPluginContainerB, DataPluginContainerB);
        struct DataPluginContainerC;
        define_data_plugin!(DataPluginC, DataPluginContainerC, DataPluginContainerC);
        struct DataPluginContainerD;
        define_data_plugin!(DataPluginD, DataPluginContainerD, DataPluginContainerD);

        // Plugin accessors
        let accessors: Vec<&(dyn Fn(&Context) + Send + Sync)> = vec![
            &|ctx: &Context| {
                let _ = ctx.get_data(DataPluginA);
            },
            &|ctx: &Context| {
                let _ = ctx.get_data(DataPluginB);
            },
            &|ctx: &Context| {
                let _ = ctx.get_data(DataPluginC);
            },
            &|ctx: &Context| {
                let _ = ctx.get_data(DataPluginD);
            },
        ];

        let num_threads = 20;
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = Vec::with_capacity(num_threads);

        for i in 0..num_threads {
            let barrier = Arc::clone(&barrier);
            let accessor = accessors[i % accessors.len()];

            let handle = thread::spawn(move || {
                let context = Context::new();
                barrier.wait();
                accessor(&context);
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }
    }
}
