//! This module provides a lightweight, feature-gated profiling interface for simulations.
//! It tracks event counts and measures elapsed time for named operations ("spans"), and can
//! export the results to the console and to a JSON file together with execution statistics. It supports:
//!
//! - Event counting – track how often named events occur during a run.
//! - Rate calculation – compute rates (events per second) since the first count.
//! - Span timing – measure time intervals with automatic closing on drop.
//! - Coverage – report how much of total runtime is covered by any span via a special
//!   "Total Measured" span.
//! - Computed statistics – define custom, derived metrics over collected data.
//! - A default computed statistic, infection forecasting efficiency.
//!
//! Feature flag: all functionality is gated behind the `profiling` feature (enabled by default).
//! When the feature is disabled, the public API remains available but becomes a no-op and gets
//! optimized away by the compiler, so you can leave profiling calls in your code at zero cost.
//!
//! ## Example console output
//! ```ignore
//! Span Label                           Count          Duration  % runtime
//! ----------------------------------------------------------------------
//! load_synth_population                    1       950us 792ns      0.36%
//! infection_attempt                     1035     6ms 33us 91ns      2.28%
//! sample_setting                        1035     3ms 66us 52ns      1.16%
//! get_contact                           1035   1ms 135us 202ns      0.43%
//! schedule_next_forecasted_infection    1286  22ms 329us 102ns      8.44%
//! Total Measured                        1385  23ms 897us 146ns      9.03%
//!
//! Event Label                     Count  Rate (per sec)
//! -----------------------------------------------------
//! property progression               36          136.05
//! recovery                           27          102.04
//! accepted infection attempt      1,035        3,911.50
//! forecasted infection            1,286        4,860.09
//!
//! Infection Forecasting Efficiency: 80.48%
//! ```
//!
//! ## API functions
//! - `increment_named_count`
//! - `open_span`
//! - `close_span`
//! - `print_profiling_data`
//! - `print_named_counts`
//! - `print_named_spans`
//! - `print_computed_statistics`
//! - `add_computed_statistic`
//!
//! All of the above functions are no-ops without the `profiling` feature.
//!
//! ## Basic usage
//!
//! Count an event:
//! ```rust,ignore
//! increment_named_count("forecasted infection");
//! increment_named_count("accepted infection attempt");
//! ```
//!
//! Time an operation:
//! ```rust,ignore
//! let span = open_span("forecast loop");
//! // operation code here (algorithm, function call, etc.)
//! close_span(span); // optional; dropping the span also closes it
//! ```
//!
//! You can also rely on RAII to auto-close a span at the end of scope:
//! ```rust,ignore
//! fn complicated_function() {
//!     let _span = open_span("complicated function");
//!     // Complicated control flow here, maybe with lots of `return` points.
//! } // `_span` goes out of scope, automatically closed.
//! ```
//!
//! Printing results to the console:
//! ```rust,ignore
//! // Call after the simulation completes
//! print_profiling_data();
//! ```
//! Prints spans, counts, and any computed statistics via the functions
//! `print_named_spans()`, `print_named_counts()`, `print_computed_statistics()`,
//! which you can use individually if you prefer.
//!
//! Writing results to JSON together with execution statistics:
//! ```rust,ignore
//! use ixa::Context; // your simulation context
//! use crate::profiling::ProfilingContextExt;
//!
//! fn finalize(mut context: Context) {
//!     // Ensure Params::profiling_data_path is set, and report options specify
//!     // output_dir/file_prefix/overwrite. This writes a pretty JSON file with:
//!     //   date_time, execution_statistics, named_counts, named_spans, computed_statistics
//!     context.write_profiling_data();
//! }
//! ```
//!
//! Special names and coverage
//! - Spans may overlap or nest. The sum of all individual span durations will not
//!   generally equal total runtime. A special span named `"Total Measured"` is open
//!   if and only if any other span is open. It tells you how much of the total running
//!   time is covered by some span.
//!
//! ## Computed statistics
//!
//! You can register custom computed statistics that derive values from the current
//! `ProfilingData`. Use `add_computed_statistic(label, description, computer, printer)`
//! to add one. The relevant API is:
//!
//! ```rust, ignore
//! // Not exactly as implemented for technical reasons.
//! pub fn add_computed_statistic(
//!     // The label used in the profiling JSON file
//!     label: &'static str,
//!     /// Description of the statistic. Used in the JSON report.
//!     description: &'static str,
//!     /// A function that takes a reference to the `ProfilingData` and computes a value
//!     computer: CustomStatisticComputer,
//!     /// A function that prints the computed value to the console.
//!     printer: CustomStatisticPrinter,
//! );
//!
//! pub type CustomStatisticComputer<T> =
//!     Box<dyn (Fn(&ProfilingData) -> Option<T>) + Send + Sync>;
//! pub type CustomStatisticPrinter<T> = Box<dyn (Fn(T)) + Send + Sync>;
//!
//! // The "computer" gets an immutable reference to all counts and spans and to the start time.
//! pub fn add_computed_statistic<T: ComputableType>(
//!     label: &'static str,
//!     description: &'static str,
//!     computer: CustomStatisticComputer<T>,
//!     printer: CustomStatisticPrinter<T>,
//! )
//! ```
//!
//! The "computer" returns an option for cases when a statistic is only conditionally
//! defined. The "printer" takes the computed value and prints it to the console.
//!
//! Computed statistics are printed by `print_computed_statistics()` and included in the
//! JSON report under `computed_statistics` (with label, description, and value).
//!
//!
//! Example of using `"forecasted infection"` and `"accepted infection attempt"`.
//! ```rust,ignore
//! context.add_plan(next_time, move |context| {
//!     increment_named_count("forecasted infection");
//!     if evaluate_forecast(context, person, forecasted_total_infectiousness) {
//!         if let Some(setting_id) = context.get_setting_for_contact(person) {
//!             if let Some(next_contact) = infection_attempt(context, person, setting_id) {
//!                 increment_named_count("accepted infection attempt");
//!                 context.infect_person(next_contact, Some(person), None, None);
//!             }
//!         }
//!     }
//!     schedule_next_forecasted_infection(context, person);
//! });
//! ```
#![allow(dead_code)]
#![allow(unused_imports)]

mod computed_statistic;
mod data;
mod display;
mod file;
mod reporting;

use std::path::Path;
#[cfg(feature = "profiling")]
use std::time::Instant;

pub use computed_statistic::*;
pub use data::*;
pub use display::*;
use file::write_profiling_data_to_file;
pub use reporting::*;

use crate::{error, Context, ContextReportExt};

#[cfg(test)]
/// Publicly expose access to profiling data only for testing.
pub fn get_profiling_data() -> std::sync::MutexGuard<'static, ProfilingData> {
    profiling_data()
}

// "Magic" constants used in this module
/// The distinguished total measured time label.
#[cfg(feature = "profiling")]
const TOTAL_MEASURED: &str = "Total Measured";
#[cfg(feature = "profiling")]
const NAMED_SPANS_HEADERS: &[&str] = &["Span Label", "Count", "Duration", "% runtime"];
#[cfg(feature = "profiling")]
const NAMED_COUNTS_HEADERS: &[&str] = &["Event Label", "Count", "Rate (per sec)"];

pub struct Span {
    #[cfg(feature = "profiling")]
    label: &'static str,
    #[cfg(feature = "profiling")]
    start_time: Instant,
}

impl Span {
    fn new(#[allow(unused_variables)] label: &'static str) -> Self {
        Self {
            #[cfg(feature = "profiling")]
            label,
            #[cfg(feature = "profiling")]
            start_time: Instant::now(),
        }
    }
}

#[cfg(feature = "profiling")]
impl Drop for Span {
    fn drop(&mut self) {
        let mut container = profiling_data();
        container.close_span(self);
    }
}
