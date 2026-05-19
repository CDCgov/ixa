//! Provides [`IxaError`] and wraps other errors.
use std::error::Error;
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

    #[error("illegal value for global property `{name}`: {source}")]
    IllegalGlobalPropertyValue {
        name: String,
        source: Box<dyn Error + Send + Sync + 'static>,
    },

    #[error(
        "the same property appears in both position {first_index} and {second_index} in the property list"
    )]
    DuplicatePropertyInPropertyList {
        first_index: usize,
        second_index: usize,
    },

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

#[cfg(test)]
mod tests {
    use super::IxaError;

    // `anyhow::Error` requires the wrapped error to be `Send + Sync + 'static`.
    // These tests guard against a regression where a new variant (or a boxed
    // source) loses those bounds and breaks interop with `anyhow`.

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn ixa_error_is_send_sync() {
        assert_send_sync::<IxaError>();
    }

    #[test]
    fn ixa_error_converts_to_anyhow() {
        fn returns_anyhow() -> anyhow::Result<()> {
            Err(IxaError::EntryAlreadyExists)?;
            Ok(())
        }
        let err = returns_anyhow().unwrap_err();
        assert!(err.downcast_ref::<IxaError>().is_some());
    }
}
