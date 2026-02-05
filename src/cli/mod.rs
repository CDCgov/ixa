mod build_command;
mod dev_command;
mod main;
mod new_command;
mod profile_command;
mod run_command;
mod utils;

// Commands here
mod commands {
    pub use super::build_command::build;
    pub use super::dev_command::dev;
    pub use super::new_command::new;
    pub use super::profile_command::profile;
    pub use super::run_command::run;
}

pub use main::*;
