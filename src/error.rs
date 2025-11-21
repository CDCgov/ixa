//! Provides [`IxaError`] and wraps other errors.
use std::fmt::{self, Debug, Display};
use std::io;

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
/// Provides [`IxaError`] and maps to other errors to
/// convert to an [`IxaError`]
pub enum IxaError {
    IoError(io::Error),
    JsonError(serde_json::Error),
    CsvError(csv::Error),
    Utf8Error(std::string::FromUtf8Error),
    ParseIntError(std::num::ParseIntError),
    IxaError(String),
}

impl From<io::Error> for IxaError {
    fn from(error: io::Error) -> Self {
        IxaError::IoError(error)
    }
}

impl From<serde_json::Error> for IxaError {
    fn from(error: serde_json::Error) -> Self {
        IxaError::JsonError(error)
    }
}

impl From<csv::Error> for IxaError {
    fn from(error: csv::Error) -> Self {
        IxaError::CsvError(error)
    }
}

impl From<std::string::FromUtf8Error> for IxaError {
    fn from(error: std::string::FromUtf8Error) -> Self {
        IxaError::Utf8Error(error)
    }
}

impl From<std::num::ParseIntError> for IxaError {
    fn from(error: std::num::ParseIntError) -> Self {
        IxaError::ParseIntError(error)
    }
}

impl From<String> for IxaError {
    fn from(error: String) -> Self {
        IxaError::IxaError(error)
    }
}

impl From<&str> for IxaError {
    fn from(error: &str) -> Self {
        IxaError::IxaError(error.to_string())
    }
}

impl std::error::Error for IxaError {}

impl Display for IxaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error: {self:?}")?;
        Ok(())
    }
}
