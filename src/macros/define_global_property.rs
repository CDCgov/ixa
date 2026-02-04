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
