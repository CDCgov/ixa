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
//! a simple disease transmission model might consist of the
//! following modules:
//! * A population loader that initializes the set of people represented
//!   by the simulation.
//! * An infection seeder that introduces the pathogen into the population.
//! * A disease progression manager that transitions infected people through
//!   stages of disease until recovery.
//! * A transmission manager that models the process of an infected
//!   person trying to infect susceptible people in the population.
//!
//! ## Features
//!
//! - **`debugger`**: enables the interactive debugger, an interactive console-based REPL
//!   (Read-Eval-Print Loop) that allows you to pause simulation execution, inspect state, and
//!   control simulation flow through commands like breakpoints, population queries, and
//!   step-by-step execution.
//! - **`web_api`**: enables the web API, an HTTP-based remote control interface that allows
//!   external applications to monitor simulation state, control execution, and query data through
//!   REST endpoints. This feature implies the `debugger` feature.
//!
pub mod context;
pub use context::{Context, ExecutionPhase, IxaEvent, PluginContext};

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

pub mod tabulator;
pub use tabulator::Tabulator;

pub mod report;
pub use report::{ConfigReportOptions, ContextReportExt, Report};

pub mod runner;
pub use runner::{run_with_args, run_with_custom_args, BaseArgs};

#[cfg(feature = "debugger")]
pub mod debugger;

pub mod log;
pub use log::{
    debug, disable_logging, enable_logging, error, info, set_log_level, set_module_filter,
    set_module_filters, trace, warn, LevelFilter,
};

#[cfg(feature = "progress_bar")]
pub mod progress;

#[cfg(feature = "debugger")]
pub mod external_api;
mod hashing;
pub mod numeric;

#[cfg(feature = "web_api")]
pub mod web_api;

// Re-export for macros
pub use csv;
pub use ctor;
pub use paste;
pub use rand;

// Deterministic hashing data structures
pub use crate::hashing::{HashMap, HashMapExt, HashSet, HashSetExt};

// Preludes
pub mod prelude;

pub mod prelude_for_plugins {
    pub use crate::context::PluginContext;
    pub use crate::define_data_plugin;
    pub use crate::error::IxaError;
    pub use crate::prelude::*;
    pub use crate::IxaEvent;
    pub use ixa_derive::IxaEvent;
}

mod execution_stats;

#[cfg(all(target_arch = "wasm32", feature = "debugger"))]
compile_error!(
    "Target `wasm32` and feature `debugger` are mutually exclusive — enable at most one."
);

#[cfg(all(target_arch = "wasm32", feature = "progress_bar"))]
compile_error!(
    "Target `wasm32` and feature `progress_bar` are mutually exclusive — enable at most one."
);
