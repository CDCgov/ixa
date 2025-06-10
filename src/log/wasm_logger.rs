/*!

A logger for WASM builds.

*/

use crate::log::{LogConfiguration, ModuleLogConfiguration, LOG_CONFIGURATION};
use log::{LevelFilter};
use std::time::{SystemTime, UNIX_EPOCH};
use fern::Dispatch;

// fn format_timestamp() -> String {
//     let now = SystemTime::now()
//         .duration_since(UNIX_EPOCH)
//         .unwrap_or_default();
//     format!("{}.{:03}Z", now.as_secs(), now.subsec_millis())
// }

impl LogConfiguration {
    pub fn set_config(&mut self) {
        if !self.initialized {
            self.init()
        }
    }
    
    fn init(&mut self) {
        // Setup fern with custom filtering and formatting
        Dispatch::new()
            .level(LevelFilter::Trace) // Set to lowest; we manually filter
            .filter(|metadata| {
                let config = LOG_CONFIGURATION.lock().unwrap();
                config.should_log(metadata.target(), metadata.level())
            })
            .chain(fern::Output::call(console_log::log))
            .apply()
            .expect("Could not set up logging");
        self.initialized = true;
    }

    fn should_log(&self, target: &str, level: log::Level) -> bool {
        // Check exact or longest-prefix match in module_configurations
        let mut longest_match = None;
        for (prefix, config) in &self.module_configurations {
            if target.starts_with(prefix)
                && longest_match.as_ref().map_or(true, |(m, _): &(&String, &ModuleLogConfiguration)| m.len() < prefix.len())
            {
                longest_match = Some((prefix, config));
            }
        }

        match longest_match {
            Some((_, ModuleLogConfiguration { level: module_level, .. })) => level <= *module_level,
            None => level <= self.global_log_level,
        }
    }
}

