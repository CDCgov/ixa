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
//! Logging is _disabled_ by default. Logging messages can be enabled by passing the command line
//! option `--log-level <level>`. Log messages can also be controlled programmatically. Logging
//! can be enabled/disabled from code using the functions:
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
#[cfg(all(not(target_arch = "wasm32"), feature = "logging"))]
mod standard_logger;

#[cfg(all(target_arch = "wasm32", feature = "logging"))]
mod wasm_logger;

#[cfg(not(feature = "logging"))]
mod null_logger;

pub use log::{debug, error, info, trace, warn, LevelFilter};
use std::collections::hash_map::Entry;

use crate::HashMap;
#[cfg(all(not(target_arch = "wasm32"), feature = "logging"))]
use log4rs::Handle;
use std::sync::LazyLock;
use std::sync::{Mutex, MutexGuard};

// Logging disabled
const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::Off;
// Default module specific filters
const DEFAULT_MODULE_FILTERS: [(&str, LevelFilter); 1] = [
    // `rustyline` logs are noisy.
    ("rustyline", LevelFilter::Off),
];

/// A global instance of the logging configuration.
static LOG_CONFIGURATION: LazyLock<Mutex<LogConfiguration>> = LazyLock::new(Mutex::default);

/// Different log level filters can be applied to the log messages emitted from different modules
/// according to the module path (e.g. `"ixa::people"`). These are stored in the global
/// `LogConfiguration`.
#[derive(Debug, PartialEq)]
struct ModuleLogConfiguration {
    /// The module path this configuration applies to
    module: String,
    /// The maximum log level for this module path
    level: LevelFilter,
}

impl From<(&str, LevelFilter)> for ModuleLogConfiguration {
    fn from((module, level): (&str, LevelFilter)) -> Self {
        Self {
            module: module.to_string(),
            level,
        }
    }
}

/// Holds logging configuration. It's primary responsibility is to keep track of the filter levels
/// of modules and hold a handle to the global logger.
///
/// Because loggers are globally installed, only one instance of this struct should exist. The
/// public API are free functions which fetch the singleton and call the appropriate member
/// function.
#[derive(Debug)]
pub(in crate::log) struct LogConfiguration {
    /// The "default" level filter for modules ("targets") without an explicitly set filter. A
    /// global filter level of `LevelFilter::Off` disables logging.
    pub(in crate::log) global_log_level: LevelFilter,
    pub(in crate::log) module_configurations: HashMap<String, ModuleLogConfiguration>,

    #[cfg(all(not(target_arch = "wasm32"), feature = "logging"))]
    /// Handle to the `log4rs` logger.
    root_handle: Option<Handle>,

    #[cfg(all(target_arch = "wasm32", feature = "logging"))]
    initialized: bool,
}

impl Default for LogConfiguration {
    fn default() -> Self {
        let module_configurations = DEFAULT_MODULE_FILTERS
            .map(|(module, level)| (module.to_string(), (module, level).into()));
        let module_configurations = HashMap::from_iter(module_configurations);
        Self {
            global_log_level: DEFAULT_LOG_LEVEL,
            module_configurations,

            #[cfg(all(not(target_arch = "wasm32"), feature = "logging"))]
            root_handle: None,

            #[cfg(all(target_arch = "wasm32", feature = "logging"))]
            initialized: false,
        }
    }
}

impl LogConfiguration {
    pub(in crate::log) fn set_log_level(&mut self, level: LevelFilter) {
        self.global_log_level = level;
        self.set_config();
    }

    /// Returns true if the configuration was mutated, false otherwise.
    fn insert_module_filter(&mut self, module: &String, level: LevelFilter) -> bool {
        match self.module_configurations.entry(module.clone()) {
            Entry::Occupied(mut entry) => {
                let module_config = entry.get_mut();
                if module_config.level == level {
                    // Don't bother building a setting a new config
                    return false;
                }
                module_config.level = level;
            }

            Entry::Vacant(entry) => {
                let new_configuration = ModuleLogConfiguration {
                    module: module.to_string(),
                    level,
                };
                entry.insert(new_configuration);
            }
        }
        true
    }

    pub(in crate::log) fn set_module_filter<S: ToString>(
        &mut self,
        module: &S,
        level: LevelFilter,
    ) {
        if self.insert_module_filter(&module.to_string(), level) {
            self.set_config();
        }
    }

    pub(in crate::log) fn set_module_filters<S: ToString>(
        &mut self,
        module_filters: &[(&S, LevelFilter)],
    ) {
        let mut mutated: bool = false;
        for (module, level) in module_filters {
            mutated |= self.insert_module_filter(&module.to_string(), *level);
        }
        if mutated {
            self.set_config();
        }
    }

    pub(in crate::log) fn remove_module_filter(&mut self, module: &str) {
        if self.module_configurations.remove(module).is_some() {
            self.set_config();
        }
    }
}

// The public API

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
    let mut log_configuration = get_log_configuration();
    log_configuration.set_log_level(level);
}

/// Sets a level filter for the given module path.
pub fn set_module_filter(module_path: &str, level_filter: LevelFilter) {
    let mut log_configuration = get_log_configuration();
    log_configuration.set_module_filter(&module_path, level_filter);
}

/// Removes a module-specific level filter for the given module path. The global level filter will
/// apply to the module.
pub fn remove_module_filter(module_path: &str) {
    let mut log_configuration = get_log_configuration();
    log_configuration.remove_module_filter(module_path);
}

/// Sets the level filters for a set of modules according to the provided map. Use this instead of
/// `set_module_filter()` to set filters in bulk.
#[allow(clippy::implicit_hasher)]
pub fn set_module_filters<S: ToString>(module_filters: &[(&S, LevelFilter)]) {
    let mut log_configuration = get_log_configuration();
    log_configuration.set_module_filters(module_filters);
}

/// Fetches a mutable reference to the global `LogConfiguration`.
fn get_log_configuration() -> MutexGuard<'static, LogConfiguration> {
    LOG_CONFIGURATION.lock().expect("Mutex poisoned")
}

#[cfg(test)]
mod tests {
    use super::{get_log_configuration, remove_module_filter, set_log_level, set_module_filters};
    use log::{error, trace, LevelFilter};
    use std::sync::{LazyLock, Mutex};

    // Force logging tests to run serially for consistent behavior.
    static TEST_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(Mutex::default);

    #[test]
    fn test_set_log_level() {
        let _guard = TEST_MUTEX.lock().expect("Mutex poisoned");
        set_log_level(LevelFilter::Trace);
        set_log_level(LevelFilter::Error);
        {
            let config = get_log_configuration();
            assert_eq!(config.global_log_level, LevelFilter::Error);
            // Note: `log::max_level()` is not necessarily accurate when global filtering is done
            //       by the `log4rs::Root` logger. The following assert may not be satisfied.
            // assert_eq!(log::max_level(), LevelFilter::Error);
            error!("test_set_log_level: global set to error");
            trace!("test_set_log_level: NOT EMITTED");
        }
        set_log_level(LevelFilter::Trace);
        {
            let config = get_log_configuration();
            assert_eq!(config.global_log_level, LevelFilter::Trace);
            assert_eq!(log::max_level(), LevelFilter::Trace);
            trace!("test_set_log_level: global set to trace");
        }
    }

    #[test]
    fn test_set_remove_module_filters() {
        let _guard = TEST_MUTEX.lock().expect("Mutex poisoned");
        // Initialize logging
        set_log_level(LevelFilter::Trace);
        {
            let config = get_log_configuration();
            // There is only one filer...
            assert_eq!(config.module_configurations.len(), 1);
            // ...and that filter is for `rustyline`
            let expected = ("rustyline", LevelFilter::Off).into();
            assert_eq!(
                config.module_configurations.get("rustyline"),
                Some(&expected)
            );
        }

        let filters: [(&&str, LevelFilter); 2] = [
            (&"rustyline", LevelFilter::Error),
            (&"ixa", LevelFilter::Debug),
        ];
        // Install new filters
        set_module_filters(&filters);

        // The filters are now the set of filters we just installed
        {
            let config = get_log_configuration();
            assert_eq!(config.module_configurations.len(), 2);
            for (module_path, level) in &filters {
                assert_eq!(
                    config.module_configurations.get(**module_path),
                    Some(&((**module_path, *level).into()))
                );
            }
        }

        // Remove one filter
        remove_module_filter("rustyline");
        // Check that it was removed
        {
            let config = get_log_configuration();
            // There is only one filer...
            assert_eq!(config.module_configurations.len(), 1);
            // ...and that filter is for `ixa`
            assert_eq!(
                config.module_configurations.get("ixa"),
                Some(&("ixa", LevelFilter::Debug).into())
            );
        }
    }
}
