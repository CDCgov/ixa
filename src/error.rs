//! Provides [`IxaError`] and wraps other errors.
use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
#[allow(clippy::module_name_repetitions)]
/// Provides [`IxaError`] and maps to other errors to
/// convert to an [`IxaError`]
pub enum IxaError {
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    JsonError(#[from] serde_json::Error),
    #[error(transparent)]
    CsvError(#[from] csv::Error),
    #[error(transparent)]
    Utf8Error(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),

    #[error("duplicate property {name}")]
    DuplicateProperty { name: String },
    #[error("entry already exists")]
    EntryAlreadyExists,
    #[error("no global property: {name}")]
    NoGlobalProperty { name: String },
    #[error("property {name} is not set")]
    PropertyNotSet { name: String },

    #[error("illegal value for `{field}`: {value}")]
    IllegalGlobalPropertyValue { field: String, value: String },

    #[error(
        "the same property appears in both position {first_index} and {second_index} in the property list"
    )]
    DuplicatePropertyInPropertyList {
        first_index: usize,
        second_index: usize,
    },

    #[error("breakpoint time {time} is in the past")]
    BreakpointTimeInPast { time: f64 },
    #[error("attempted to delete a nonexistent breakpoint {id}")]
    BreakpointNotFound { id: u32 },

    #[error("invalid key in pair: {pair}")]
    InvalidLogLevelKey { pair: String },
    #[error("invalid value in pair: {pair}")]
    InvalidLogLevelValue { pair: String },
    #[error("invalid log level: {level}")]
    InvalidLogLevel { level: String },

    #[error("invalid log level format: {log_level}")]
    InvalidLogLevelFormat { log_level: String },

    #[error("cannot make edge to self")]
    CannotMakeEdgeToSelf,
    #[error("invalid weight")]
    InvalidWeight,
    #[error("edge already exists")]
    EdgeAlreadyExists,
    #[error("can't sample from empty list")]
    CannotSampleFromEmptyList,

    #[error("initialization list is missing required properties")]
    MissingRequiredInitializationProperties,
}
