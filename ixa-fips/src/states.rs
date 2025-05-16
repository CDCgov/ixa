/*!

An enum for states as represented by FIPS Geographic Region Codes. Note that the `FIPSCode` encoded type only uses six
bits to encode the state code, which can accommodate codes <= 63. Thus, it is best to only use `FIPSCode` for actual
proper states.

*/

use crate::StateCode;
use strum::AsRefStr;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, AsRefStr)]
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
    pub fn is_state(&self) -> bool {
        let v = *self as StateCode;
        v <= 56u8 && ![3u8, 7, 14, 43, 52].contains(&v)
    }

    /// Returns true if `self` is a state (not including District of Columbia)
    pub fn is_proper_state(&self) -> bool {
        self.is_state() && *self as StateCode != 11
    }

    /// Returns the numeric FIPS code for this state.
    ///
    /// This representation only requires 6 bits if `self.is_state()`.
    pub fn encode(&self) -> StateCode {
        *self as StateCode
    }

    pub fn decode(value: StateCode) -> Result<USState, ()> {
        if !Self::valid_code(value) {
            return Err(());
        }
        Ok(unsafe { std::mem::transmute(value) })
    }

    pub fn valid_code(code: StateCode) -> bool {
        // The list in this next line contains all values between 1 and 98 that are not assigned to a valid region.
        // Note that there are codes that are neither states nor EAS maritime region codes nor FIPS 5-1 reserved codes,
        // like Midway Islands, for example.
        code != 0 && code <= 98 && ![62u8, 63, 80, 82, 83, 85, 87, 88, 90].contains(&code)
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
    fn test_is_proper_state() {
        assert!(USState::AK.is_proper_state());
        assert!(!USState::DC.is_proper_state());
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
