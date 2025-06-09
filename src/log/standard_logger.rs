use crate::log::{LogConfiguration, ModuleLogConfiguration, DEFAULT_LOG_PATTERN};
use log::LevelFilter;
use log4rs::{
    config::{
        runtime::ConfigBuilder,
        Appender,
        Logger,
        Root
    },
    append::console::ConsoleAppender,
    encode::pattern::PatternEncoder,
    Config
};
use std::collections::hash_map::Entry;

impl From<&ModuleLogConfiguration> for Logger {
    fn from(module_config: &ModuleLogConfiguration) -> Self {
        Logger::builder().build(module_config.module.clone(), module_config.level)
    }
}

impl LogConfiguration {
    /// Sets the global logger to conform to this `LogConfiguration`.
    fn set_config(&mut self) {
        let stdout: ConsoleAppender = ConsoleAppender::builder()
            .encoder(Box::new(PatternEncoder::new(DEFAULT_LOG_PATTERN)))
            .build();
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
