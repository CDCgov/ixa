use log4rs::append::console::ConsoleAppender;
use log4rs::config::runtime::ConfigBuilder;
use log4rs::config::{Appender, Logger, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Config;

#[cfg(feature = "progress_bar")]
use super::progress_bar_encoder::PBWrapperEncoder;
use crate::log::{LogConfiguration, ModuleLogConfiguration};

// Use an ISO 8601 timestamp format and color coded level tag
const DEFAULT_LOG_PATTERN: &str = "{d(%Y-%m-%dT%H:%M:%SZ)} {h({l})} {t} - {m}{n}";

impl From<&ModuleLogConfiguration> for Logger {
    fn from(module_config: &ModuleLogConfiguration) -> Self {
        Logger::builder().build(module_config.module.clone(), module_config.level)
    }
}

impl LogConfiguration {
    /// Sets the global logger to conform to this [`LogConfiguration`].
    pub(in crate::log) fn set_config(&mut self) {
        let encoder = Box::new(PatternEncoder::new(DEFAULT_LOG_PATTERN));
        // Appends an ANSI escape code to clear to end of line.
        #[cfg(feature = "progress_bar")]
        let encoder = Box::new(PBWrapperEncoder::new(encoder));
        let stdout: ConsoleAppender = ConsoleAppender::builder().encoder(encoder).build();
        let mut config: ConfigBuilder =
            Config::builder().appender(Appender::builder().build("stdout", Box::new(stdout)));

        // Add module specific configuration
        for module_config in self.module_configurations.values() {
            config = config.logger(module_config.into());
        }

        // The `Root` determines the global log level
        let root = Root::builder()
            .appender("stdout")
            .build(self.global_log_level);
        let new_config = match config.build(root) {
            Err(e) => {
                panic!("failed to build config: {e}");
            }
            Ok(config) => config,
        };

        match self.root_handle {
            Some(ref mut handle) => {
                // The global logger has already been initialized
                handle.set_config(new_config);
            }

            None => {
                // The global logger has not yet been initialized
                self.root_handle = Some(log4rs::init_config(new_config).unwrap());
            }
        }
    }
}
