/*!

An enum for states as represented by FIPS Geographic Region Codes. Note that the `FIPSCode` encoded type only uses six
bits to encode the state code, which can accommodate codes <= 63. Thus, it is best to only use `FIPSCode` for actual
proper states.

*/

use crate::StateCode;
use std::fmt::Display;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum USState {
    AL = 1,
    AK = 2,
    AmericanSamoa = 3, // (FIPS 5-1 reserved code)
    AZ = 4,
    AR = 5,
    CA = 6,
    CanalZone = 7, // (FIPS 5-1 reserved code)
    CO = 8,
    CT = 9,
    DE = 10,
    DC = 11, // District of Columbia
    FL = 12,
    GA = 13,
    Guam = 14, // (FIPS 5-1 reserved code)
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
    PuertoRico = 43, // (FIPS 5-1 reserved code)
    RI = 44,
    SC = 45,
    SD = 46,
    TN = 47,
    TX = 48,
    UT = 49,
    VT = 50,
    VA = 51,
    VirginIslandsOfTheUS = 52, // (FIPS 5-1 reserved code)
    WA = 53,
    WV = 54,
    WI = 55,
    WY = 56,
    AS = 60,
    FM = 64,
    GU = 66,
    JohnstonAtoll = 67,
    MH = 68,
    MP = 69,
    PW = 70,
    MidwayIslands = 71,
    PR = 72,
    UM = 74,
    NavassaIsland = 76,
    VI = 78,
    WakeIsland = 79,
    BakerIsland = 81,
    HowlandIsland = 84,
    JarvisIsland = 86,
    KingmanReef = 89,
    PalmyraAtoll = 95,

    // Invalid / unassigned values:
    //    62, 63, ??
    //    80, 82, 83, 85, 87, 88, 90, ??

    // EAS Maritime values:
    //    57, 58, 59, 61,
    //    65, 73, 75, 77,
    //    91, 92, 93, 94, 96, 97, 98

    // We don't use the EAS Maritime Areas, but what the heck, we'll include them.
    PacificCoastFromWashingtonToCalifornia = 57, // EAS Maritime area
    AlaskanCoast = 58,                           // EAS Maritime area
    HawaiianCoast = 59,                          // EAS Maritime area
    AmericanSamoaWaters = 61,                    // EAS Maritime area
    MarianaIslandsWatersIncludingGuam = 65,      // EAS Maritime area
    AtlanticCoastFromMaineToVirginia = 73,       // EAS Maritime area
    AtlanticCoastFromNorthCarolinaToFloridaAndTheCoastsOfPuertoRicoAndVirginIslands = 75, // EAS Maritime area
    GulfOfMexico = 77,                           // EAS Maritime area
    LakeSuperior = 91,                           // EAS Maritime area
    LakeMichigan = 92,                           // EAS Maritime area
    LakeHuron = 93,                              // EAS Maritime area
    StClairRiverDetroitRiverAndLakeStClair = 94, // EAS Maritime area
    LakeErie = 96,                               // EAS Maritime area
    NiagaraRiverAndLakeOntario = 97,             // EAS Maritime area
    StLawrenceRiver = 98,                        // EAS Maritime area
}

impl USState {
    /// Returns true is `self` is a state or District of Columbia
    pub fn is_state(&self) -> bool {
        let v = *self as StateCode;
        v <= 56u8 && ![3u8, 7, 14, 43, 52].contains(&v)
    }

    /// Returns true if `self` is a state (not including District of Columbia)
    pub fn is_proper_state(&self) -> bool {
        self.is_state() && *self as StateCode != 11
    }

    /// Returns true if `self` is one of the EAS maritime region codes.
    pub fn is_eas_maritime(&self) -> bool {
        [57u8, 58, 59, 61, 65, 73, 75, 77, 91, 92, 93, 94, 96, 97, 98]
            .contains(&(*self as StateCode))
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

    pub fn as_str(&self) -> &'static str {
        match self {
            USState::AL => "AL",
            USState::AK => "AK",
            USState::AZ => "AZ",
            USState::AR => "AR",
            USState::CA => "CA",
            USState::CO => "CO",
            USState::CT => "CT",
            USState::DE => "DE",
            USState::DC => "DC", // District of Columbia
            USState::FL => "FL",
            USState::GA => "GA",
            USState::HI => "HI",
            USState::ID => "ID",
            USState::IL => "IL",
            USState::IN => "IN",
            USState::IA => "IA",
            USState::KS => "KS",
            USState::KY => "KY",
            USState::LA => "LA",
            USState::ME => "ME",
            USState::MD => "MD",
            USState::MA => "MA",
            USState::MI => "MI",
            USState::MN => "MN",
            USState::MS => "MS",
            USState::MO => "MO",
            USState::MT => "MT",
            USState::NE => "NE",
            USState::NV => "NV",
            USState::NH => "NH",
            USState::NJ => "NJ",
            USState::NM => "NM",
            USState::NY => "NY",
            USState::NC => "NC",
            USState::ND => "ND",
            USState::OH => "OH",
            USState::OK => "OK",
            USState::OR => "OR",
            USState::PA => "PA",
            USState::RI => "RI",
            USState::SC => "SC",
            USState::SD => "SD",
            USState::TN => "TN",
            USState::TX => "TX",
            USState::UT => "UT",
            USState::VT => "VT",
            USState::VA => "VA",
            USState::WA => "WA",
            USState::WV => "WV",
            USState::WI => "WI",
            USState::WY => "WY",
            _ => unimplemented!("Only states are supported."),
        }
    }
}

impl Display for USState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        assert_eq!(USState::AK.to_string(), "AK");
        assert_eq!(USState::NavassaIsland.to_string(), "NavassaIsland");
    }

    #[test]
    fn test_is_state() {
        assert!(USState::AK.is_state());
        assert!(USState::DC.is_state());
        assert!(!USState::NavassaIsland.is_state());
        assert!(!USState::LakeHuron.is_state());
    }

    #[test]
    fn test_is_proper_state() {
        assert!(USState::AK.is_proper_state());
        assert!(!USState::DC.is_proper_state());
        assert!(!USState::NavassaIsland.is_proper_state());
        assert!(!USState::LakeHuron.is_proper_state());
    }

    #[test]
    fn test_is_eas_maritime() {
        assert!(!USState::PuertoRico.is_eas_maritime());
        assert!(!USState::VA.is_eas_maritime());
        assert!(USState::GulfOfMexico.is_eas_maritime());
        assert!(!USState::JarvisIsland.is_eas_maritime());
        assert!(USState::AmericanSamoaWaters.is_eas_maritime());
    }

    #[test]
    fn test_decode() {
        assert_eq!(USState::PuertoRico, USState::decode(43).unwrap());
        assert_eq!(USState::MidwayIslands, USState::decode(71).unwrap());
        assert_eq!(USState::MN, USState::decode(27).unwrap());
        assert_eq!(USState::LakeSuperior, USState::decode(91).unwrap());

        assert!(USState::decode(99).is_err());
        assert!(USState::decode(62).is_err());
        assert!(USState::decode(63).is_err());
        assert!(USState::decode(80).is_err());
        assert!(USState::decode(90).is_err());
        assert!(USState::decode(0).is_err());
    }

    #[test]
    fn test_encode() {
        assert_eq!(USState::PuertoRico.encode(), 43u8);
        assert_eq!(USState::MidwayIslands.encode(), 71u8);
        assert_eq!(USState::MN.encode(), 27u8);
        assert_eq!(USState::LakeSuperior.encode(), 91u8);
    }
}
