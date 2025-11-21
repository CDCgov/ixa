//!
//! We only have one error case, namely when a value is out of range. Instances of `FIPSError` are constructed
//! with the `FIPSError::from_*_code()` constructor methods in the `FIPSCode::encode_*()` constructor methods.
//!

use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

use crate::{CountyCode, DataCode, IdCode, SettingCategoryCode, StateCode, TractCode};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct FIPSError {
    parameter_name: &'static str,
    value: u64,
    min: u64,
    max: u64,
}

impl Display for FIPSError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "value {} provided for {} is outside valid range of {}..{}",
            self.value, self.parameter_name, self.min, self.max
        )
    }
}

impl Error for FIPSError {}

impl FIPSError {
    #[must_use]
    pub fn new(parameter_name: &'static str, value: u64, min: u64, max: u64) -> Self {
        Self {
            parameter_name,
            value,
            min,
            max,
        }
    }

    // Convenience constructors for the code types. These should be kept in sync with the module-level documentation
    // in `fips_code.rs`.

    /// This one is unique in that it represents an error converting a (presumably valid) [`StateCode`] to a [`USState`]
    /// variant. We lie a little bit and claim that values in 1..57 are valid when 3, 7, 14, 43, and 52 are not.
    #[must_use]
    pub fn from_us_state(value: StateCode) -> Self {
        Self {
            parameter_name: "USState Code",
            value: value as u64,
            min: 1,
            max: 57, // 1..57
        }
    }

    #[must_use]
    pub fn from_state_code(value: StateCode) -> Self {
        Self {
            parameter_name: "StateCode",
            value: value as u64,
            min: 1,
            max: 100, // Two decimal digits
        }
    }

    #[must_use]
    pub fn from_county_code(value: CountyCode) -> Self {
        Self {
            parameter_name: "CountyCode",
            value: value as u64,
            min: 0,
            max: 1000, // Three decimal digits
        }
    }

    #[must_use]
    pub fn from_tract_code(value: TractCode) -> Self {
        Self {
            parameter_name: "TractCode",
            value: value as u64,
            min: 0,
            max: 1_000_000, // Six decimal digits
        }
    }

    #[must_use]
    pub fn from_setting_category_code(value: SettingCategoryCode) -> Self {
        Self {
            parameter_name: "SettingCategoryCode",
            value: value as u64,
            min: 0,
            max: 16, // 2^4
        }
    }

    #[must_use]
    pub fn from_id_code(value: IdCode) -> Self {
        Self {
            parameter_name: "IdCode",
            value: value as u64,
            min: 0,
            max: 16_384, // 2^14
        }
    }

    #[must_use]
    pub fn from_data_code(value: DataCode) -> Self {
        Self {
            parameter_name: "DataCode",
            value: value as u64,
            min: 0,
            max: 512, // 2^9
        }
    }
}

// Tested in `fips_code.rs`.
