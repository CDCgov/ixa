/// Use this to define a unique type which will be used as a key to retrieve  
/// an independent rng instance when calling `Context::get_rng`.  
#[macro_export]
macro_rules! define_rng {
    ($random_id:ident) => {
        #[derive(Copy, Clone)]
        struct $random_id;

        impl $crate::random::RngId for $random_id {
            type RngType = $crate::rand::rngs::SmallRng;

            fn get_name() -> &'static str {
                stringify!($random_id)
            }
        }

        // This ensures that you can't define two RngIds with the same name
        $crate::paste::paste! {
            #[doc(hidden)]
            #[no_mangle]
            #[allow(non_upper_case_globals)]
            pub static [<rng_name_duplication_guard_ $random_id>]: () = ();
        }
    };
}
pub use define_rng;
