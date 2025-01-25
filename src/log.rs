//! The `log` module defines an interface to Ixa's internal logging facilities. Logging messages about
//! internal behavior of Ixa. This is not to be confused with _reporting_, which is model-level concept
//! for Ixa users to record data about running models.
//!
//! Model authors can nonetheless use Ixa's logging facilities to output messages. This module
//! (re)exports the five logging macros: `error!`, `warn!`, `info!`, `debug!` and `trace!` where
//! `error!` represents the highest-priority log messages and `trace!` the lowest. To emit a log
//! message, simply use one of these macros in your code:
//!
//! ```rust
//! use ixa::{info};
//!
//! pub fn do_a_thing() {
//!     info!("A thing is being done.");
//! }
//! ```
//!
//! Logging is _disabled_ by default. Log messages are enabled/disabled using the functions:
//!
//!  - `enable_logging()`: turns on all log messages
//!  - `disable_logging()`: turns off all log messages
//!  - `set_log_level(level: LevelFilter)`: enables only log messages with priority at least `level`
//!
//! In addition, per-module filtering of messages can be configured using `set_module_filter()` /
//! `set_module_filters()` and `remove_module_filter()`:
//!
//! ```rust
//! use ixa::log::{set_module_filter, remove_module_filter, set_module_filters, LevelFilter,
//! enable_logging, set_log_level};
//!
//! pub fn setup_logging() {
//!     // Enable `info` log messages globally.
//!     set_log_level(LevelFilter::Info);
//!     // Disable Ixa's internal logging messages.
//!     set_module_filter("ixa", LevelFilter::Off);
//!     // Enable all log messages for the `transmission_manager` module.
//!     set_module_filter("transmission_manager", LevelFilter::Trace);
//! }
//! ```

use env_logger::{Builder, Logger, WriteStyle};
pub use log::{debug, error, info, trace, warn, LevelFilter};
use log_reload::{ReloadHandle, ReloadLog};

use std::cell::OnceCell;
use std::collections::HashMap;
use std::sync::Mutex;

// Logging disabled.
const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::Off;
// Automatically determine if output supports color.
const DEFAULT_LOG_STYLE: WriteStyle = WriteStyle::Auto;
// Default module specific filters
const DEFAULT_MODULE_FILTERS: [(&str, LevelFilter); 1] = [
    ("rustyline", LevelFilter::Off),
    // ("ixa", LevelFilter::Off),
];

/// A global instance of the logging configuration.
static mut LOG_CONFIGURATION: OnceCell<Mutex<LogConfiguration>> = OnceCell::new();

/// Holds logging configuration so the configuration can persist across reinitialization of the
/// global logger.
///
/// Neither `env_logger::Builder` nor `env_logger::Logger` can be modified once constructed. This
/// struct serves as a mutable proxy for `env_logger::Builder`. Because the global logger cannot
/// be initialized more than once, we use `log_reload::ReloadLog` as the global logger, which
/// serves as a wrapper around the real logger that allows us to swap out the inner logger after
/// initialization.
struct LogConfiguration {
    /// The "default" level filter for modules ("targets") without an explicitly set filter. A
    /// global filter level of `LevelFilter::Off` disables logging.
    global_log_level: LevelFilter,
    /// Whether to colorize output.
    log_style: WriteStyle,
    /// Holds module ("target") specific level filters
    module_level: HashMap<String, LevelFilter>,
    /// A handle to the logger that can reload or modify its inner wrapped logger.
    log_handle: Option<ReloadHandle<Logger>>,
}

impl Default for LogConfiguration {
    fn default() -> Self {
        let module_level = HashMap::from(
            DEFAULT_MODULE_FILTERS.map(|(module, level)| (module.to_string(), level)),
        );

        LogConfiguration {
            global_log_level: DEFAULT_LOG_LEVEL,
            log_style: DEFAULT_LOG_STYLE,
            module_level,
            log_handle: None,
        }
    }
}

impl LogConfiguration {
    /// Constructs an `env_logger::Logger` with the current configuration. This is analogous to
    /// `env_logger::Builder::build()`. This method does not install the logger.
    pub fn build(&self) -> Logger {
        let mut builder = Builder::new();

        builder
            .filter_level(self.global_log_level)
            .write_style(self.log_style);
        // Add module specific filters.
        for (module, filter) in &self.module_level {
            builder.filter(Some(module), *filter);
        }

        builder.build()
    }
}

/// Enables the logger with no global level filter / full logging. Equivalent to
/// `set_log_level(LevelFilter::Trace)`.
pub fn enable_logging() {
    set_log_level(LevelFilter::Trace);
}

/// Disables logging completely. Equivalent to `set_log_level(LevelFilter::Off)`.
pub fn disable_logging() {
    set_log_level(LevelFilter::Off);
}

/// Sets the global log level. A global filter level of `LevelFilter::Off` disables logging.
pub fn set_log_level(level: LevelFilter) {
    {
        let log_configuration = get_log_configuration();
        log_configuration.global_log_level = level;
    }
    set_logger();
}

/// Sets a level filter for the given module path.
pub fn set_module_filter(module_path: &str, level_filter: LevelFilter) {
    {
        let log_configuration = get_log_configuration();
        log_configuration
            .module_level
            .insert(module_path.to_string(), level_filter);
    }
    set_logger();
}

/// Removes a module-specific level filter for the given module path. The global level filter will
/// apply to the module.
pub fn remove_module_filter(module_path: &str) {
    {
        let log_configuration = get_log_configuration();
        log_configuration.module_level.remove(module_path);
    }
    set_logger();
}

/// Sets the level filters for a set of modules according to the provided map. Use this instead of
/// `set_module_filter()` to set filters in bulk.
#[allow(clippy::implicit_hasher)]
pub fn set_module_filters(module_filters: &HashMap<&str, LevelFilter>) {
    {
        let log_configuration = get_log_configuration();
        log_configuration.module_level.extend(
            module_filters
                .iter()
                .map(|(module_path, level)| ((*module_path).to_string(), *level)),
        );
    }
    set_logger();
}

/// Fetches a mutable reference to the global `LogConfiguration`.
fn get_log_configuration() -> &'static mut LogConfiguration {
    // Silence lint about mutable global variables.
    #[allow(static_mut_refs)]
    unsafe {
        if let Some(mutex) = LOG_CONFIGURATION.get_mut() {
            mutex.get_mut().unwrap()
        } else {
            _ = LOG_CONFIGURATION.set(Mutex::default());
            LOG_CONFIGURATION.get_mut().unwrap().get_mut().unwrap()
        }
    }
}

/// Initializes or replaces the existing global logger with a logger described by the global
/// log configuration.
fn set_logger() {
    let log_configuration = get_log_configuration();
    let logger = log_configuration.build();
    trace!("Setting logger");

    match &log_configuration.log_handle {
        None => {
            // Logger has not been initialized.
            let wrapping_logger = ReloadLog::new(logger);
            log_configuration.log_handle = Some(wrapping_logger.handle());
            let result = log::set_boxed_logger(Box::new(wrapping_logger))
                .map(|()| log::set_max_level(log_configuration.global_log_level));
            if let Err(error) = result {
                error!(
                    "tried to initialize a global logger that has already been set: {}",
                    error
                );
            }
        }

        Some(handle) => {
            // Replace the existing logger.
            if let Err(error) = handle.replace(logger) {
                error!("failed to set logger: {}", error);
            }
        }
    }
}
