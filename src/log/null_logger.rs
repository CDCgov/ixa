/*!

This module provides a "logger" that does not output anything anywhere but satisfies the public API.

*/

use crate::log::LogConfiguration;

impl LogConfiguration {
    /// Sets the global logger to conform to this `LogConfiguration`.
    pub(in crate::log) fn set_config(&mut self) {
        // No global logger. We still keep up appearances.
        log::set_max_level(self.global_log_level);
    }
}
