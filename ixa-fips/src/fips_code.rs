//! Defines the `FIPSCode` types to represent FIPS geographic region codes (and “code fragments”) very efficiently.
//!
//! # Encoding Scheme
//!
//! A table of how FIPS Geo IDs are structured is provided in the module-level documentation for [`crate::parser`]
//! (slightly modified from
//! [the source table in the standard](https://www.census.gov/programs-surveys/geography/guidance/geo-identifiers.html)).
//! The rows in the table up to and including Block (that is, all but the last five rows) form a linear order with
//! respect to prefix inclusion ("is prefix of"). This encoding scheme is for these codes. The last four rows are
//! treated separately.
//!
//! In the following table, we describe the data "fragments" and their storage requirements.
//!
//! |                                   | **Decimal Digits** | **Actual Max Value** | **Bits** |    **Capacity (`2^bits - 1`)** |
//! |:--------------------------------- | ------------------:| --------------------:| --------:| ------------------------------:|
//! | **Sate**                          |                  2 |                   56 |        7 |                            127 |
//! | **County**                        |                  3 |                  840 |       10 |                          1,023 |
//! | **Tract**                         |                  6 |              990,101 |       20 |                      1,048,575 |
//! | **Subtotal**                      |                    |                      |   **37** | **Bits needed for tract code** |
//! |                                   |                    |                      |          |                                |
//! | **Monotonically Increasing Id's** |                    |                      |          |                                |
//! | **homeId**                        |                  4 |                9,999 |       14 |                         16,383 |
//! | **publicschoolId**                |                  3 |                  999 |       10 |                          1,023 |
//! | **privateschoolId**               |                  4 |                1,722 |       11 |                          2,047 |
//! | **workplaceId**                   |                  5 |               14,938 |       14 |                         16,383 |
//! | **Max:**                          |                    |                      |   **14** |                                |
//! | **Total:**                        |                    |                      |   **51** |                                |
//!
//! State codes for states have values <= 56, but there are "state codes" for outlying areas, some historic codes, and
//! maritime extension codes in use in the wild. We therefore use an extra bit than strictly required to represent it.
//! To the 51 bits apparently required to store this data we add an additional 4 bits for a category tag to distinguish
//! between home, public school, private school, workplace, and cencus tract, a field useful for representing ASPR
//! synthetic population data, for example. Only 2 bits are required to distinguish these 4 categories, so the additional
//! 2 bits are left unused / for future use.
//!
//! We encode this data into a `u64` as follows:
//!
//!  | **Data**               |     **State** | **County** | **Tract** |  **Category Tag** | **Monotonically increasing ID number** | **Reserved / Unused** |
//!  |:---------------------- | -------------:| ----------:| ---------:| -----------------:| --------------------------------------:| ---------------------:|
//!  | **Bits**               |         63…57 |      57…47 |     46…27 |             26…23 |                                   22…9 |                   8…0 |
//!  | **Ex. Value**          | `AK`, `AZ`, … |        258 |   223,100 | `Home`, `Work`, … |                                 12,345 |                     0 |
//!  | **Bit Count**          |             7 |         10 |        20 |                 4 |                                     14 |                     9 |
//!  | **Capacity**           |           128 |      1,024 | 1,048,576 |                16 |                                 16,384 |                   512 |
//!  | **Decimal Digits**     |             2 |          3 |         6 |                 - |                                 3 to 5 |                     - |
//!  | **Max Observed Value** |            56 |        840 |   990,101 |                 4 |                                 14,938 |                     - |
//!
//! Observe that:
//!
//!  - We give the "category tag" 4 bits to allow up to 16 distinct categories. In some applications this field might be unused.
//!  - The least significant 9 bits is completely unused by this encoding. It may be used for application-specific storage.
//!  - The field for ID number only requires 10 bits for `publicschoolId`, for example. That is, the storage it requires
//!    depends on the category tag.
//!  - The category tag is encoded after the tract code but before the ID field so that numerical ordering coincides with
//!    the hierarchical ordering.
//!  - Likewise, the unused 9 bits are the least significant bits so that numerical ordering coincides with the
//!    hierarchical ordering modulo those bits.
//!
//! # Nonhierarchical FIPS Codes
//!
//! The encoding of the previous section excludes the nonhierarchical codes of the last five rows from the first table
//! above:
//!
//!  - Places
//!  - Congressional District (113th Congress)
//!  - State Legislative District (Upper Chamber)
//!  - State Legislative District (Lower Chamber)
//!  - ZCTA
//!
//! We could easily accommodate these codes as well in a variety of ways, e.g.:
//!  - assign each of these a category tag and store their corresponding code fragments in the ID field
//!  - use the 14 bits of the ID field and the unused 10 least significant bits, allowing the category tag to remain
//!    orthogonal
//!
//! We leave them unspecified until we have a use case for them.
use crate::{
    states::USState, CountyCode, DataCode, IdCode, SettingCategoryCode, StateCode, TractCode,
    CATEGORY_OFFSET, COUNTY_OFFSET, FOURTEEN_BIT_MASK, FOUR_BIT_MASK, ID_OFFSET, NINE_BIT_MASK,
    SEVEN_BIT_MASK, STATE_OFFSET, TEN_BIT_MASK, TRACT_OFFSET, TWENTY_BIT_MASK,
};
use std::{
    cmp::Ordering,
    fmt::{Debug, Display, Formatter},
    num::NonZero,
};

/// Encodes a hierarchical FIPS geographic region code in 64 bits. Excludes the nonhierarchical codes places,
/// congressional or state legislative districts, and ZIP code tabulation areas. (See the
/// [module level documentation](`crate::fips_code`).)
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct FIPSCode(NonZero<u64>);

impl FIPSCode {
    // region Constructors
    /// Constructs a new `FIPSCode` from a USState. Unlike the other constructors, this constructor is infallible.
    #[must_use]
    pub fn with_state(state: USState) -> Self {
        Self::new(state.into(), 0, 0, 0, 0, 0).unwrap()
    }
    /// Constructs a new `FIPSCode`.
    /// Returns `Err(())` if the data provided is out of range.
    pub fn with_state_code(state_code: StateCode) -> Result<Self, ()> {
        Self::new(state_code, 0, 0, 0, 0, 0)
    }
    /// Constructs a new `FIPSCode`.
    /// Returns `Err(())` if the data provided is out of range.
    pub fn with_county(state: StateCode, county: CountyCode) -> Result<Self, ()> {
        Self::new(state, county, 0, 0, 0, 0)
    }
    /// Constructs a new `FIPSCode`.
    /// Returns `Err(())` if the data provided is out of range.
    pub fn with_tract(state: StateCode, county: CountyCode, tract: TractCode) -> Result<Self, ()> {
        Self::new(state, county, tract, 0, 0, 0)
    }
    /// Constructs a new `FIPSCode`.
    /// Returns `Err(())` if the data provided is out of range.
    pub fn with_category(
        state: StateCode,
        county: CountyCode,
        tract: TractCode,
        category: SettingCategoryCode,
    ) -> Result<Self, ()> {
        Self::new(state, county, tract, category, 0, 0)
    }

    pub fn new(
        state: StateCode,
        county: CountyCode,
        tract: TractCode,
        category: SettingCategoryCode,
        id: IdCode,
        data: DataCode,
    ) -> Result<Self, ()> {
        let encoded: u64 = Self::encode_state(state)?
            | Self::encode_county(county)?
            | Self::encode_tract(tract)?
            | Self::encode_category(category)?
            | Self::encode_id(id)?
            | Self::encode_data(data)?;
        // At the very least, `USState.encode()` will return a non-zero value, so this unwrapping is safe.
        let encoded = NonZero::new(encoded).unwrap();
        Ok(Self(encoded))
    }
    // endregion Constructors

    // region Accessors

    /// Returns the FIPS STATE as a `USState` enum variant.
    /// Returns `Err(())` if 'USState' cannot represent the state code. Use `FIPSCode::state_code()` to
    /// retrieve the state code in this case.
    #[inline(always)]
    pub fn state(&self) -> Result<USState, ()> {
        USState::decode(self.state_code())
    }

    /// Returns the FIPS STATE code as a `StateCode` (a `u8`)
    #[inline(always)]
    #[must_use]
    pub fn state_code(&self) -> StateCode {
        // The state code occupies the 7 most significant bits, bits 57..63
        (self.0.get() >> STATE_OFFSET) as StateCode
    }

    /// Returns the numeric FIPS COUNTY code
    #[inline(always)]
    #[must_use]
    pub fn county_code(&self) -> CountyCode {
        // The county code occupies the 10 bits from bits 47..56
        ((self.0.get() >> COUNTY_OFFSET) as CountyCode) & TEN_BIT_MASK
    }

    /// Returns the numeric FIPS CENSUS TRACT code
    #[inline(always)]
    #[must_use]
    pub fn census_tract_code(&self) -> TractCode {
        // The census tract code occupies the 20 bits from bits 27..46
        ((self.0.get() >> TRACT_OFFSET) as TractCode) & TWENTY_BIT_MASK
    }

    /// Returns the numeric SETTING CATEGORY code
    #[inline(always)]
    #[must_use]
    pub fn category_code(&self) -> SettingCategoryCode {
        // The category code occupies the 4 bits from bits 23..26
        ((self.0.get() >> CATEGORY_OFFSET) as SettingCategoryCode) & FOUR_BIT_MASK
    }

    /// Returns the monotonically increasing ID number as a `u16`
    #[inline(always)]
    #[must_use]
    pub fn id(&self) -> IdCode {
        // The ID number occupies the 14 bits from bits 9..22
        ((self.0.get() >> ID_OFFSET) as IdCode) & FOURTEEN_BIT_MASK
    }

    /// Returns the unused data region occupying the 9 LSB
    #[inline(always)]
    #[must_use]
    pub fn data(&self) -> DataCode {
        self.0.get() as DataCode & NINE_BIT_MASK
    }
    // endregion Accessors

    // region Setters

    /// Creates a copy of `self` with the FIPS STATE set to `state`.
    #[must_use]
    pub fn set_state(&self, state: USState) -> Self {
        self.set_state_code(state.into()).unwrap()
    }

    /// Creates a copy of `self` with the FIPS STATE set to `state`.
    pub fn set_state_code(&self, state_code: StateCode) -> Result<Self, ()> {
        let mut expanded = ExpandedFIPSCode::from_fips_code(*self);
        expanded.state = state_code;
        expanded.to_fips_code()
    }

    /// Creates a copy of `self` with the FIPS COUNTY set to `county`.
    pub fn set_county(&self, county: CountyCode) -> Result<Self, ()> {
        let mut expanded = ExpandedFIPSCode::from_fips_code(*self);
        expanded.county = county;
        expanded.to_fips_code()
    }

    /// Creates a copy of `self` with the FIPS CENSUS TRACT set to `tract`.
    pub fn set_tract(&self, tract: TractCode) -> Result<Self, ()> {
        let mut expanded = ExpandedFIPSCode::from_fips_code(*self);
        expanded.tract = tract;
        expanded.to_fips_code()
    }

    /// Creates a copy of `self` with the setting category set to `category`.
    pub fn set_category(&self, category: SettingCategoryCode) -> Result<Self, ()> {
        let mut expanded = ExpandedFIPSCode::from_fips_code(*self);
        expanded.category = category;
        expanded.to_fips_code()
    }

    /// Creates a copy of `self` with the ID number set to `id`.
    pub fn set_id(&self, id: IdCode) -> Result<Self, ()> {
        let mut expanded = ExpandedFIPSCode::from_fips_code(*self);
        expanded.id = id;
        expanded.to_fips_code()
    }

    /// Creates a copy of `self` with the unused data region set to `data`.
    pub fn set_data(&self, data: DataCode) -> Result<Self, ()> {
        let mut expanded = ExpandedFIPSCode::from_fips_code(*self);
        expanded.data = data;
        expanded.to_fips_code()
    }

    // endregion Setters

    /// Sets the unused data region occupying the 10 LSB in place.
    /// Returns `Ok(())` if `data` is in range, `Err(())` otherwise.
    #[inline(always)]
    pub fn set_data_in_place(&mut self, data: u16) -> Result<(), ()> {
        if data <= NINE_BIT_MASK {
            let inverse_mask = !(NINE_BIT_MASK as u64);
            let code = (self.0.get() & inverse_mask) | ((data & NINE_BIT_MASK) as u64);
            // The result is guaranteed to be nonzero if the original code was valid, so unwrap will succeed.
            self.0 = NonZero::new(code).unwrap();
            Ok(())
        } else {
            Err(())
        }
    }

    /// Compares the given values without respect to the data region (the Least Significant Bits). Use the usual
    /// equality operators for comparing `FIPSCode`s including the data region.
    #[inline(always)]
    #[must_use]
    pub fn compare_non_data(&self, other: Self) -> Ordering {
        let inverse_mask = !(NINE_BIT_MASK as u64);
        let this = self.0.get() & inverse_mask;
        let other = other.0.get() & inverse_mask;

        this.cmp(&other)
    }

    // region Encoding
    // It is convenient to factor out the encode operations into their own functions.
    // These functions take numeric values and return encoded `u64` values. To encode
    // enum variants, call the `encode` function on the enum variant.

    #[inline(always)]
    fn encode_state(state: StateCode) -> Result<u64, ()> {
        // Validate
        if state <= SEVEN_BIT_MASK && state != 0 {
            Ok((state as u64) << STATE_OFFSET)
        } else {
            Err(())
        }
    }

    #[inline(always)]
    fn encode_county(county: CountyCode) -> Result<u64, ()> {
        // Validate
        if county <= TEN_BIT_MASK {
            Ok((county as u64) << COUNTY_OFFSET)
        } else {
            Err(())
        }
    }

    #[inline(always)]
    fn encode_tract(tract: TractCode) -> Result<u64, ()> {
        // Validate
        if tract <= TWENTY_BIT_MASK {
            Ok((tract as u64) << TRACT_OFFSET)
        } else {
            Err(())
        }
    }

    #[inline(always)]
    fn encode_category(setting_category: SettingCategoryCode) -> Result<u64, ()> {
        // Validate
        if setting_category <= FOUR_BIT_MASK {
            Ok((setting_category as u64) << CATEGORY_OFFSET)
        } else {
            Err(())
        }
    }

    #[inline(always)]
    fn encode_id(id: IdCode) -> Result<u64, ()> {
        // Validate
        if id <= FOURTEEN_BIT_MASK {
            Ok((id as u64) << ID_OFFSET)
        } else {
            Err(())
        }
    }

    #[inline(always)]
    fn encode_data(data: DataCode) -> Result<u64, ()> {
        // Validate
        if data <= NINE_BIT_MASK {
            Ok(data as u64)
        } else {
            Err(())
        }
    }
    // endregion Encoding
}

impl Display for FIPSCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", ExpandedFIPSCode::from_fips_code(*self))
    }
}

impl Debug for FIPSCode {
    /// Format the code as a string of hex digits with fields separated by dashes. Note that this is different
    /// from serializing to the original FIPS code encoding. Use `format_as_fips_code`/`format_as_fips_code`
    /// for that purpose.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}", self.state_code())?;
        write!(f, "-{:03}", self.county_code())?;
        write!(f, "-{:06}", self.census_tract_code())?;
        write!(f, "-{:01}", self.category_code())?;
        write!(f, "-{:05}", self.id())?;
        write!(f, "-{:03x}", self.data())?;
        Ok(())
    }
}

/// A struct that holds an expanded version of a `FIPSCode` in which all fields are represented by
/// their associated numeric types.
///
/// It is up to the client code to ensure the field values are within
/// range. See the module level docs for range constraints.
///
/// This struct is useful for converting raw data to/from a `FIPSCode` for temporary direct field access.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct ExpandedFIPSCode {
    pub state: StateCode,
    pub county: CountyCode,
    pub tract: TractCode,
    pub category: SettingCategoryCode,
    pub id: IdCode,
    pub data: DataCode,
}

impl ExpandedFIPSCode {
    #[must_use]
    pub fn from_fips_code(fips_code: FIPSCode) -> Self {
        Self {
            state: fips_code.state_code(),
            county: fips_code.county_code(),
            tract: fips_code.census_tract_code(),
            category: fips_code.category_code(),
            id: fips_code.id(),
            data: fips_code.data(),
        }
    }

    pub fn to_fips_code(&self) -> Result<FIPSCode, ()> {
        FIPSCode::new(
            self.state,
            self.county,
            self.tract,
            self.category,
            self.id,
            self.data,
        )
    }
}

impl Display for ExpandedFIPSCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Format the state if possible
        if let Ok(state) = USState::decode(self.state) {
            write!(f, "state: {}", state.as_ref())?;
        } else {
            write!(f, "state: {}", self.state)?;
        }
        // For the remaining fields, only print them if they are nonzero
        if self.county != 0 {
            write!(f, ", county: {}", self.county)?;
        }
        if self.tract != 0 {
            write!(f, ", tract: {}", self.tract)?;
        }
        if self.category != 0 {
            write!(f, ", setting: {}", self.category)?;
        }
        if self.id != 0 {
            write!(f, ", id: {}", self.id)?;
        }
        if self.data != 0 {
            write!(f, ", data field: {}", self.data)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(u8)]
    enum SettingCategory {
        Unspecified = 0,
        Home,
        School,
        Work,
        CensusTract,
    }

    impl From<SettingCategory> for SettingCategoryCode {
        fn from(value: SettingCategory) -> Self {
            value as SettingCategoryCode
        }
    }

    #[test]
    fn test_data_ranges() {
        // Encode functions
        assert!(FIPSCode::encode_state(SEVEN_BIT_MASK).is_ok());
        assert!(FIPSCode::encode_state(SEVEN_BIT_MASK + 1).is_err());
        assert!(FIPSCode::encode_county(TEN_BIT_MASK).is_ok());
        assert!(FIPSCode::encode_county(TEN_BIT_MASK + 1).is_err());
        assert!(FIPSCode::encode_tract(TWENTY_BIT_MASK).is_ok());
        assert!(FIPSCode::encode_tract(TWENTY_BIT_MASK + 1).is_err());
        assert!(FIPSCode::encode_category(FOUR_BIT_MASK).is_ok());
        assert!(FIPSCode::encode_category(FOUR_BIT_MASK + 1).is_err());
        assert!(FIPSCode::encode_id(FOURTEEN_BIT_MASK).is_ok());
        assert!(FIPSCode::encode_id(FOURTEEN_BIT_MASK + 1).is_err());
        assert!(FIPSCode::encode_data(NINE_BIT_MASK).is_ok());
        assert!(FIPSCode::encode_data(NINE_BIT_MASK + 1).is_err());
        // Constructors
        assert!(FIPSCode::with_state_code(1).is_ok());
        assert!(FIPSCode::with_state_code(0).is_err());
        assert!(FIPSCode::with_state_code(SEVEN_BIT_MASK + 1).is_err());
        assert!(FIPSCode::with_county(1, 0).is_ok());
        assert!(FIPSCode::with_county(1, TEN_BIT_MASK + 1).is_err());
        assert!(FIPSCode::with_tract(1, 0, 0).is_ok());
        assert!(FIPSCode::with_tract(1, 0, TWENTY_BIT_MASK + 1).is_err());
        assert!(FIPSCode::with_category(1, 0, 0, 0).is_ok());
        assert!(FIPSCode::with_category(1, 0, 0, FOUR_BIT_MASK + 1).is_err());
    }

    #[test]
    fn fields_round_trip() {
        let fips_code = FIPSCode::new(
            USState::TX.into(),
            123,
            990101,
            SettingCategory::Home.into(),
            14938,
            123,
        )
        .unwrap();
        assert_eq!(fips_code.state().unwrap(), USState::TX);
        assert_eq!(fips_code.county_code(), 123);
        assert_eq!(fips_code.census_tract_code(), 990101);
        assert_eq!(fips_code.category_code(), SettingCategory::Home.into());
        assert_eq!(fips_code.id(), 14938);
        assert_eq!(fips_code.data(), 123);
    }

    #[test]
    fn expanded_round_trip() {
        let fips_code = FIPSCode::new(
            USState::TX.into(),
            123,
            990101,
            SettingCategory::Home.into(),
            14938,
            0x01ff,
        )
        .unwrap();
        let expanded = ExpandedFIPSCode::from_fips_code(fips_code);
        let result = expanded.to_fips_code().unwrap();
        assert_eq!(result, fips_code);
    }

    #[test]
    fn test_compare_non_data() {
        let fips_code_a = FIPSCode::new(
            USState::TX.into(),
            123,
            990101,
            SettingCategory::Home.into(),
            14938,
            0x01ff,
        )
        .unwrap();
        let fips_code_b = FIPSCode::new(
            USState::TX.into(),
            123,
            990101,
            SettingCategory::Home.into(),
            14938,
            0x00ff,
        )
        .unwrap();

        assert_eq!(fips_code_a.compare_non_data(fips_code_b), Ordering::Equal);
        assert_eq!(fips_code_a.cmp(&fips_code_b), Ordering::Greater);
    }

    #[test]
    fn test_set_id() {
        // Exercises case that triggered a bug that causes a panic.
        let fips_code = FIPSCode::with_category(
            USState::AK.into(),
            0,
            0,
            SettingCategory::CensusTract.into(),
        )
        .unwrap();

        let other_fips_code = fips_code.set_id(0).unwrap();
        assert_eq!(fips_code, other_fips_code);
    }
}
