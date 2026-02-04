/// Helper for `define_data_plugin`
#[macro_export]
macro_rules! __define_data_plugin {
    ($data_plugin:ident, $data_container:ty, |$ctx:ident| $body:expr) => {
        struct $data_plugin;

        impl $crate::DataPlugin for $data_plugin {
            type DataContainer = $data_container;

            fn init<C: $crate::PluginContext>($ctx: &C) -> Self::DataContainer {
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
