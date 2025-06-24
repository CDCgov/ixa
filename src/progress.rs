//! Provides functions to set up and update a progress bar.
//!
//! A progress bar has a label, a maximum progress value, and its current progress, which
//! starts at zero. The maximum and current progress values are constrained to be of type
//! `usize`. However, convenience methods are provided for the common case of a progress bar
//! for the timeline that take `f64` time values and rounds them to nearest integers for you.
//!
//! Only one progress bar can be active at a time. If you try to set a second progress bar, the
//! new progress bar will replace this first. This is useful if you want to track the progress
//! of a simulation in multiple phases. Keep in mind, however, that if you define a timeline
//! progress bar, the `Context` will try to update it in its event loop with the current time,
//! which might not be what you want if you have replaced the progress bar with a new one.
//!
//! # Timeline Progress Bar
//!
//! ```ignore
//! /// Initialize the progress bar with the maximum time until the simulation ends.
//! pub fn init_timeline_progress_bar(max_time: f64);
//! /// Updates the progress bar with the current time. Finalizes the progress bar when
//! /// `current_time >= max_time`.
//! pub fn update_timeline_progress(mut current_time: f64);
//! ```
//!
//! # Custom Progress Bar
//!
//! If the timeline is not a good indication of progress for your simulation, you can set up a
//! custom progress bar.
//!
//! ```ignore
//! /// Initializes a custom progress bar with the given label and max value.
//! pub fn init_custom_progress_bar(label: &str, max_value: usize);
//!
//! /// Updates the current value of the custom progress bar.
//! pub fn update_custom_progress(current_value: usize);
//!
//! /// Increments the custom progress bar by 1. Use this if you don't want to keep track of the
//! /// current value.
//! pub fn increment_custom_progress()
//! ```
//!
//! # Custom Example: People Infected
//!
//! Suppose you want a progress bar that tracks how much of the population has been infected (or
//! infected and then recovered). You first initialize a custom progress bar before executing
//! the simulation.
//!
//! ```ignore
//! use crate::progress_bar::{init_custom_progress_bar};
//!
//! init_custom_progress_bar("People Infected", POPULATION_SIZE);
//! ```
//!
//! To update the progress bar, we need to listen to the infection status property change event.
//!
//! ```ignore
//! use crate::progress_bar::{increment_custom_progress};
//!
//! // You might already have this event defined for other purposes.
//! pub type InfectionStatusEvent = PersonPropertyChangeEvent<InfectionStatus>;
//!
//! // This will handle the status change event, updating the progress bar
//! // if there is a new infection.
//! fn handle_infection_status_change(context: &mut Context, event: InfectionStatusEvent) {
//!   // We only increment the progress bar when a new infection occurs.
//!   if (InfectionStatusValue::Susceptible, InfectionStatusValue::Infected)
//!   		== (event.previous, event.current)
//!   {
//!     increment_custom_progress();
//!   }
//! }
//!
//! // Be sure to subscribe to the event when you initialize the context.
//! pub fn init(context: &mut Context) -> Result<(), IxaError> {
//!     // ... other initialization code ...
//!     context.subscribe_to_event::<InfectionStatusEvent>(handle_infection_status_change);
//!     // ...
//!     Ok(())
//! }
//! ```
//!

use crate::log::{trace, warn};
use progress_bar::{
    finalize_progress_bar, inc_progress_bar, init_progress_bar, set_progress_bar_action,
    set_progress_bar_progress, Color, Style,
};
// Requires at least rustc@1.70
use std::sync::OnceLock;

/// We want to store the original `f64` max value, not the `usize` we initialized the progress
/// bar with.
pub(crate) static MAX_TIME: OnceLock<f64> = OnceLock::new();

/// Initialize the progress bar with the maximum time until the simulation ends.
pub fn init_timeline_progress_bar(max_time: f64) {
    trace!(
        "initializing timeline progress bar with max time {}",
        max_time
    );
    MAX_TIME
        .set(max_time)
        .expect("Timeline progress already initialized");
    init_progress_bar(max_time.round() as usize);
    set_progress_bar_action("Time", Color::Blue, Style::Bold);
}

/// Updates the timeline progress bar with the current time.
pub(crate) fn update_timeline_progress(mut current_time: f64) {
    if let Some(max_time) = MAX_TIME.get() {
        if current_time >= *max_time {
            current_time = *max_time;
        }
        set_progress_bar_progress(current_time.round() as usize);
        // It's possible that `progress.round() == max_time.round()` but `progress < max_time`.
        // We only finalize if they are equal as floats.
        if current_time == *max_time {
            finalize_progress_bar();
        }
    } else {
        warn!("attempted to update timeline progress bar before it was initialized");
    }
}

/// Initializes a custom progress bar with the given label and max value.
///
/// Note: If you attempt to set two progress bars, the second progress bar will replace the first.
pub fn init_custom_progress_bar(label: &str, max_value: usize) {
    trace!(
        "initializing custom progress bar with label {} and max value {}",
        label,
        max_value
    );
    init_progress_bar(max_value);
    set_progress_bar_action(label, Color::Blue, Style::Bold);
}

/// Updates the current value of the custom progress bar.
pub fn update_custom_progress(current_value: usize) {
    set_progress_bar_progress(current_value);
}

/// Increments the custom progress bar by 1. Use this if you don't want to keep track of the
/// current value.
pub fn increment_custom_progress() {
    inc_progress_bar();
}
