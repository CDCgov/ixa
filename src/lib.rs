//! A framework for building discrete-event simulations
//!
//! Ixa is a framework designed to support the creation of large-scale
//! discrete event simulations. The primary use case is the construction of
//! agent-based models for disease transmission, but the approach is applicable
//! in a wide array of circumstances.
//!
//! The central object of an Ixa simulation is the `Context` that is
//! responsible for managing all the behavior of the simulation. All of the
//! simulation-specific logic is embedded in modules that rely on the `Context`
//! for core services such as:
//! * Maintaining a notion of time for the simulation
//! * Scheduling events to occur at some point in the future and executing them
//!   at that time
//! * Holding module-specific data so that the module and other modules can
//!   access it
//!
//! In practice, a simulation usually consists of a set of modules that work
//! together to provide all of the functions of the simulation. For instance,
//! For instance, a simple disease transmission model might consist of the
//! following modules:
//! * A population loader that initializes the set of people represented
//!   by the simulation.
//! * An infection seeder that introduces the pathogen into the population.
//! * A disease progression manager that transitions infected people through
//!   stages of disease until recovery.
//! * A transmission manager that models the process of an infected
//!   person trying to infect susceptible people in the population.
pub mod context;
pub use context::{Context, ExecutionPhase, IxaEvent};

pub mod error;
pub use error::IxaError;

pub mod global_properties;
pub use global_properties::{ContextGlobalPropertiesExt, GlobalProperty};

pub mod network;
pub use network::{ContextNetworkExt, Edge, EdgeType};

pub mod people;
pub use people::{
    ContextPeopleExt, PersonCreatedEvent, PersonId, PersonProperty, PersonPropertyChangeEvent,
};

pub mod plan;
pub mod random;
pub use random::{ContextRandomExt, RngId};

pub use people::tabulator::Tabulator;

pub mod report;
pub use report::{ConfigReportOptions, ContextReportExt, Report};

pub mod runner;
pub use runner::{run_with_args, run_with_custom_args, BaseArgs};

pub mod debugger;

pub mod log;
pub use log::{
    debug, disable_logging, enable_logging, error, info, set_log_level, set_module_filter,
    set_module_filters, trace, warn, LevelFilter,
};

pub mod external_api;
mod hashing;
pub mod web_api;

// Re-export for macros
pub use ctor;
pub use paste;
pub use rand;
pub use seq_macro::seq;

// Deterministic hashing data structures
pub use crate::hashing::{HashMap, HashMapExt, HashSet, HashSetExt};

pub use std::any::TypeId;

#[inline(always)]
pub fn type_of<T: 'static>() -> TypeId {
    TypeId::of::<T>()
}

#[cfg(test)]
mod tests {
    use assert_cmd::cargo::CargoError;
    pub fn run_external_runner(runner_name: &str) -> Result<assert_cmd::Command, CargoError> {
        assert_cmd::Command::new("cargo")
            .args(["build", "--bin", runner_name])
            .ok()
            .unwrap_or_else(|_| panic!("Failed to build {runner_name}"));
        assert_cmd::Command::cargo_bin(runner_name)
    }
}
