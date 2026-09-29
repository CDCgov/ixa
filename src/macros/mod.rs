mod assert_almost_eq;
mod define_data_plugin;
mod define_global_property;
mod define_report;
mod define_rng;
mod edge_impl;
mod entity_impl;
mod property_impl;
mod schedule_relative;
mod value_change_counts;
mod with;

/// Writes one plain, newline-terminated message to the user-facing output.
///
/// Output is written to stdout on native targets and to `console.log` on Wasm.
/// Unlike diagnostic logging, it is not affected by logging features, levels,
/// or module filters.
///
/// # Examples
///
/// ```
/// use ixa::output;
///
/// output!("simulation complete at time {}", 42.0);
/// ```
#[macro_export]
macro_rules! output {
    () => {
        $crate::macros::__output(format_args!(""))
    };
    ($($arg:tt)+) => {
        $crate::macros::__output(format_args!($($arg)*))
    };
}

#[doc(hidden)]
pub fn __output(args: std::fmt::Arguments<'_>) {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&args.to_string().into());

    #[cfg(not(target_arch = "wasm32"))]
    println!("{args}");
}
