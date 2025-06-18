//! Without this wrapper, when a log message is written to the console while the progress bar
//! is active, artifacts from the part of the progress bar that were not overwritten by the
//! log message will remain.

use log::Record;
use log4rs::encode::{Encode, Write};

/// Wraps a PatternEncoder and prepends whatever is written to it to clear the current line.
#[derive(Debug)]
pub struct PBWrapperEncoder {
    inner: Box<dyn Encode>,
}

impl PBWrapperEncoder {
    pub fn new(inner: Box<dyn Encode>) -> Self {
        Self { inner }
    }
}

impl Encode for PBWrapperEncoder {
    fn encode(&self, w: &mut dyn Write, record: &Record) -> Result<(), anyhow::Error> {
        // This prefix clears the entire line and returns the cursor to the beginning.
        w.write_all("\x1B[2K\r".as_bytes())?;

        // Delegate to the inner encoder
        self.inner.encode(w, record)
    }
}
