/*!

A logger for WASM builds.

*/

use crate::log::{LogConfiguration, ModuleLogConfiguration, DEFAULT_LOG_PATTERN};
use fern::{Dispatch, FormatCallback};
use log::{LevelFilter, Record};
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};
use std::fmt::Arguments;

fn format_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}Z", now.as_secs(), now.subsec_millis())
}

impl LogConfiguration {
    /// Sets up logging using `fern` according to this configuration.
    pub fn set_config(&self) {
        let formatter = move |out: FormatCallback, message: &Arguments, record: &Record| {
            out.finish(format_args!(
                "{} [{}] {}",
                format_timestamp(),
                record.level(),
                message
            ))
        };

        // Start the base dispatcher
        let mut base = Dispatch::new()
            .format(formatter)
            .level(self.global_log_level)
            .chain(io::stdout());

        // Add per-module overrides
        for (module_name, module_config) in &self.module_configurations {
            base = base.level_for(module_name.clone(), module_config.level);
        }

        // Apply the configuration
        base.apply().expect("Failed to initialize fern logger");
    }
}
