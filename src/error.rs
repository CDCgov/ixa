use std::fmt::{self, Debug, Display};
use std::io;

#[allow(clippy::module_name_repetitions)]

/// Provides `IxaError` and maps to other errors to
/// convert to an `IxaError`
#[derive(Debug)]
pub enum IxaError {
    IoError(io::Error),
    JsonError(serde_json::Error),
    ReportError(String),
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

impl std::error::Error for IxaError {}

impl Display for IxaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error: {self:?}")?;
        Ok(())
    }
}
