/// Defines a global property with the following parameters:
/// * `$global_property`: Name for the identifier type of the global property
/// * `$value`: The type of the property's value
/// * `$validate`: A function (or closure) that checks the validity of the property (optional)
#[macro_export]
macro_rules! define_global_property {
    ($global_property:ident, $value:ty, $validate: expr) => {
        #[derive(Copy, Clone)]
        pub struct $global_property;

        impl $crate::global_properties::GlobalProperty for $global_property {
            type Value = $value;

            fn new() -> Self {
                $global_property
            }

            fn validate(val: &$value) -> Result<(), $crate::error::IxaError> {
                $validate(val)
            }
        }

        $crate::paste::paste! {
            $crate::ctor::declarative::ctor!{
                #[ctor]
                fn [<$global_property:snake _register>]() {
                    let module = module_path!();
                    let mut name = module.split("::").next().unwrap().to_string();
                    name += ".";
                    name += stringify!($global_property);
                    $crate::global_properties::add_global_property::<$global_property>(&name);
                }
            }
        }
    };

    ($global_property: ident, $value: ty) => {
        define_global_property!($global_property, $value, |_| { Ok(()) });
    };
}
pub use define_global_property;

/// Define a new edge type for use with `network`.
///
/// Defines a new edge type of type `$edge_type`, with inner type `$value`.
/// Use `()` for `$value` to have no inner type.
#[allow(unused_macros)]
#[macro_export]
macro_rules! define_edge_type {
    ($edge_type:ident, $value:ty) => {
        #[derive(Debug, Copy, Clone)]
        pub struct $edge_type;

        impl $crate::network::EdgeType for $edge_type {
            type Value = $value;
        }
    };
}
pub use define_edge_type;

/// Use this macro to define a unique report type
#[macro_export]
macro_rules! define_report {
    ($name:ident) => {
        impl $crate::Report for $name {
            fn type_id(&self) -> std::any::TypeId {
                std::any::TypeId::of::<$name>()
            }

            fn serialize(&self, writer: &mut $crate::csv::Writer<std::fs::File>) {
                writer.serialize(self).unwrap();
            }
        }
    };
}
pub use define_report;

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
pub use define_data_plugin;

#[macro_export]
macro_rules! assert_almost_eq {
    ($a:expr, $b:expr, $prec:expr $(,)?) => {
        if !$crate::numeric::almost_eq($a, $b, $prec) {
            panic!(
                "assertion failed: `abs(left - right) < {:e}`, (left: `{}`, right: `{}`)",
                $prec, $a, $b
            );
        }
    };
}
pub use assert_almost_eq;
