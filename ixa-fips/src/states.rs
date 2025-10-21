//! An enum for U.S. states as represented by FIPS Geographic Region Codes. This is a minimal subset of FIPS state codes
//! which have been stable for every FIPS standard revision so far.
//! See <https://www.census.gov/library/reference/code-lists/ansi.html#states>.
//!
//! Note that the `FIPSCode` encoded type only uses six bits to encode the state code, which can accommodate codes <= 63.
//! Thus, it is best to only use `FIPSCode` for these states.

use strum::{AsRefStr, FromRepr};

use crate::errors::FIPSError;
use crate::StateCode;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, AsRefStr, FromRepr)]
#[repr(u8)]
pub enum USState {
    AL = 1,
    AK = 2,
    AZ = 4,
    AR = 5,
    CA = 6,
    CO = 8,
    CT = 9,
    DE = 10,
    DC = 11, // District of Columbia
    FL = 12,
    GA = 13,
    HI = 15,
    ID = 16,
    IL = 17,
    IN = 18,
    IA = 19,
    KS = 20,
    KY = 21,
    LA = 22,
    ME = 23,
    MD = 24,
    MA = 25,
    MI = 26,
    MN = 27,
    MS = 28,
    MO = 29,
    MT = 30,
    NE = 31,
    NV = 32,
    NH = 33,
    NJ = 34,
    NM = 35,
    NY = 36,
    NC = 37,
    ND = 38,
    OH = 39,
    OK = 40,
    OR = 41,
    PA = 42,
    RI = 44,
    SC = 45,
    SD = 46,
    TN = 47,
    TX = 48,
    UT = 49,
    VT = 50,
    VA = 51,
    WA = 53,
    WV = 54,
    WI = 55,
    WY = 56,
}

impl USState {
    /// Returns true if `self` is a state or District of Columbia
    #[must_use]
    pub fn is_state(&self) -> bool {
        USState::is_state_code(*self as StateCode)
    }

    /// Returns true if the given state code is a state or District of Columbia
    #[must_use]
    pub fn is_state_code(value: StateCode) -> bool {
        value <= 56u8 && ![3u8, 7, 14, 43, 52].contains(&value)
    }

    /// Returns the numeric FIPS code for this state.
    #[must_use]
    pub fn encode(&self) -> StateCode {
        *self as StateCode
    }

    /// Returns the state for the given numeric FIPS code.
    /// Returns `Err(FIPSError)` if the code is invalid.
    pub fn decode(value: StateCode) -> Result<USState, FIPSError> {
        Self::from_repr(value).ok_or(FIPSError::from_us_state(value))
    }
}

impl From<USState> for StateCode {
    fn from(value: USState) -> Self {
        value.encode()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        assert_eq!(USState::AK.as_ref(), "AK");
    }

    #[test]
    fn test_is_state() {
        assert!(USState::AK.is_state());
        assert!(USState::DC.is_state());
    }

    #[test]
    fn test_decode() {
        assert_eq!(USState::DC, USState::decode(11).unwrap());
        assert_eq!(USState::MN, USState::decode(27).unwrap());

        assert!(USState::decode(99).is_err());
        assert!(USState::decode(62).is_err());
        assert!(USState::decode(63).is_err());
        assert!(USState::decode(80).is_err());
        assert!(USState::decode(90).is_err());
        assert!(USState::decode(0).is_err());
    }

    #[test]
    fn test_encode() {
        assert_eq!(USState::DC.encode(), 11u8);
        assert_eq!(USState::MN.encode(), 27u8);
    }
}
