/*!

A logger for WASM builds that logs to the JavaScript console.

*/

use crate::log::{LogConfiguration, ModuleLogConfiguration, LOG_CONFIGURATION};
use fern::Dispatch;
use log::LevelFilter;
use log::{Level, Record};
use wasm_bindgen::JsValue;
use web_sys::console;

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
            .chain(fern::Output::call(log_to_browser_console))
            .apply()
            .expect("Could not set up logging");
        self.initialized = true;
    }

    fn should_log(&self, target: &str, level: log::Level) -> bool {
        // Check exact or longest-prefix match in module_configurations
        let mut longest_match = None;
        for (prefix, config) in &self.module_configurations {
            if target.starts_with(prefix)
                && longest_match
                    .as_ref()
                    .map_or(true, |(m, _): &(&String, &ModuleLogConfiguration)| {
                        m.len() < prefix.len()
                    })
            {
                longest_match = Some((prefix, config));
            }
        }

        match longest_match {
            Some((
                _,
                ModuleLogConfiguration {
                    level: module_level,
                    ..
                },
            )) => level <= *module_level,
            None => level <= self.global_log_level,
        }
    }
}

struct Style {
    trace: &'static str,
    debug: &'static str,
    info: &'static str,
    warn: &'static str,
    error: &'static str,
    file_line: &'static str,
    text: &'static str,
}

const STYLE: Style = Style {
    trace: "color: gray",
    debug: "color: blue",
    info: "color: black",
    warn: "color: orange",
    error: "color: red; font-weight: bold",
    file_line: "color: gray",
    text: "",
};

/// Logs to the browser console with styled formatting.
/// Intended to be used in `.chain(fern::Output::call(...))`
pub fn log_to_browser_console(record: &Record) {
    let console_fn = match record.level() {
        Level::Error => console::error_4,
        Level::Warn => console::warn_4,
        Level::Info => console::info_4,
        Level::Debug => console::log_4,
        Level::Trace => console::debug_4,
    };

    let message = format!(
        "%c{:<5}%c {:>20}:{:<4} %c\n{}",
        record.level(),
        record.file().unwrap_or_else(|| record.target()),
        record
            .line()
            .map_or("[unknown]".to_string(), |l| l.to_string()),
        record.args()
    );

    let level_style = JsValue::from_str(match record.level() {
        Level::Trace => STYLE.trace,
        Level::Debug => STYLE.debug,
        Level::Info => STYLE.info,
        Level::Warn => STYLE.warn,
        Level::Error => STYLE.error,
    });

    let file_line_style = JsValue::from_str(STYLE.file_line);
    let text_style = JsValue::from_str(STYLE.text);
    let message = JsValue::from_str(&message);

    console_fn(&message, &level_style, &file_line_style, &text_style);
}
