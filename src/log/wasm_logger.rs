/*!

A logger for WASM target that logs to the JavaScript console.

*/

#[cfg(not(test))]
use fern::Dispatch;
#[cfg(not(test))]
use log::LevelFilter;
use log::{Level, Record};
#[cfg(not(test))]
use wasm_bindgen::JsValue;
#[cfg(not(test))]
use web_sys::console;

#[cfg(not(test))]
use crate::log::LOG_CONFIGURATION;
use crate::log::{LogConfiguration, ModuleLogConfiguration};

impl LogConfiguration {
    #[cfg(not(test))]
    pub fn set_config(&mut self) {
        if !self.initialized {
            self.init()
        }
    }

    #[cfg(not(test))]
    fn init(&mut self) {
        // Setup fern with custom filtering and formatting
        Dispatch::new()
            .level(LevelFilter::Trace) // Set to lowest; we manually filter
            .filter(|metadata| {
                let config = LOG_CONFIGURATION.lock().unwrap();
                config.should_log(metadata.target(), metadata.level())
            })
            .chain(fern::Output::call(|record| {
                let rec = BrowserRecord::from(record);
                rec.emit_to_console();
            }))
            .apply()
            .expect("Could not set up logging");
        self.initialized = true;
    }

    fn should_log(&self, target: &str, level: log::Level) -> bool {
        // Check exact or longest-prefix match in module_configurations
        let mut longest_match = None;
        for (prefix, config) in &self.module_configurations {
            if target.starts_with(prefix)
                && longest_match.as_ref().is_none_or(
                    |(m, _): &(&String, &ModuleLogConfiguration)| m.len() < prefix.len(),
                )
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
    #[cfg(not(test))]
    file_line: &'static str,
    text: &'static str,
}

const STYLE: Style = Style {
    trace: "color: gray",
    debug: "color: blue",
    info: "color: black",
    warn: "color: orange",
    error: "color: red; font-weight: bold",
    #[cfg(not(test))]
    file_line: "color: gray",
    text: "",
};

struct BrowserRecord {
    level: Level,
    level_style: String,
    message: String,
    text_style: String,
}

impl From<&Record<'_>> for BrowserRecord {
    fn from(record: &Record) -> Self {
        let level = record.level();
        let level_style = match level {
            Level::Trace => STYLE.trace,
            Level::Debug => STYLE.debug,
            Level::Info => STYLE.info,
            Level::Warn => STYLE.warn,
            Level::Error => STYLE.error,
        }
        .to_string();

        let message = format!(
            "%c{:<5}%c {:>20}:{:<4} %c\n{}",
            record.level(),
            record.file().unwrap_or_else(|| record.target()),
            record
                .line()
                .map_or("[unknown]".to_string(), |l| l.to_string()),
            record.args()
        );

        BrowserRecord {
            level,
            level_style,
            message,
            text_style: STYLE.text.to_string(),
        }
    }
}

#[cfg(not(test))]
impl BrowserRecord {
    pub fn emit_to_console(&self) {
        let console_fn = match self.level {
            Level::Error => console::error_4,
            Level::Warn => console::warn_4,
            Level::Info => console::info_4,
            Level::Debug => console::log_4,
            Level::Trace => console::debug_4,
        };

        let message = JsValue::from_str(&self.message);

        let level_style = JsValue::from_str(&self.level_style);
        let location_style = JsValue::from_str(STYLE.file_line);
        let text_style = JsValue::from_str(&self.text_style);

        console_fn(&message, &level_style, &location_style, &text_style);
    }
}

#[cfg(test)]
mod tests {
    use log::{Level, LevelFilter, Record};

    use super::*;
    use crate::HashMap;

    #[test]
    fn should_log_global_level() {
        let config = LogConfiguration {
            global_log_level: LevelFilter::Info,
            module_configurations: HashMap::default(),
            root_handle: None,
        };

        assert!(config.should_log("any::module", Level::Info));
        assert!(!config.should_log("any::module", Level::Debug));
    }

    #[test]
    fn should_log_per_module_override() {
        let mut modules = HashMap::default();
        modules.insert(
            "my::mod".to_string(),
            ModuleLogConfiguration {
                module: "my::mod".to_string(),
                level: LevelFilter::Debug,
            },
        );

        let config = LogConfiguration {
            global_log_level: LevelFilter::Warn,
            module_configurations: modules,
            root_handle: None,
        };

        assert!(config.should_log("my::mod", Level::Debug)); // overridden
        assert!(!config.should_log("my::mod", Level::Trace)); // overridden too low
        assert!(!config.should_log("other::mod", Level::Info)); // falls back to global
    }

    #[test]
    fn module_filtering_prefers_longest_match() {
        let mut modules = HashMap::default();
        modules.insert(
            "a".to_string(),
            ModuleLogConfiguration {
                module: "a".to_string(),
                level: LevelFilter::Warn,
            },
        );
        modules.insert(
            "a::b".to_string(),
            ModuleLogConfiguration {
                module: "a::b".to_string(),
                level: LevelFilter::Debug,
            },
        );

        let config = LogConfiguration {
            global_log_level: LevelFilter::Error,
            module_configurations: modules,
            root_handle: None,
        };

        // Should match "a::b", not "a"
        assert!(config.should_log("a::b::c", Level::Debug));
        assert!(!config.should_log("a::b::c", Level::Trace));
    }

    #[test]
    fn browser_record_formats_message() {
        let level = Level::Info;
        let file = Some("src/lib.rs");
        let line = Some(42);
        let record = Record::builder()
            .level(level)
            .target("my::module")
            .file(file)
            .line(line)
            .args(format_args!("Hello from WASM"))
            .build();

        let browser_rec = BrowserRecord::from(&record);
        assert!(browser_rec.message.contains("Hello from WASM"));
        assert!(browser_rec.message.contains("src/lib.rs"));
        assert!(browser_rec.message.contains("42"));
        assert!(browser_rec.message.contains("INFO"));
        assert_eq!(browser_rec.level, Level::Info);
        assert_eq!(browser_rec.level_style, STYLE.info);
        assert_eq!(browser_rec.text_style, STYLE.text);
    }

    #[test]
    fn browser_record_formats_unknown_location() {
        let level = Level::Warn;
        let record = Record::builder()
            .level(level)
            .target("module")
            .file(None)
            .line(None)
            .args(format_args!("Something went wrong"))
            .build();

        let browser_rec = BrowserRecord::from(&record);
        assert!(browser_rec.message.contains("[unknown]"));
        assert!(browser_rec.message.contains("Something went wrong"));
        assert_eq!(browser_rec.level_style, STYLE.warn);
    }
}
