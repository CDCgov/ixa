/*!

This module provides routines to make it easier to work with the ASPR synthetic population dataset. It provides basic
parsing functionality for parsing the codes found in the dataset.  The `archive` submodule, enabled with the
"aspr_archive" feature, additionally allows for reading ASPR data from CSV files in the dataset, including files within
zipped archives.

This dataset encodes `homeId`, `schoolId`, and `workplaceId` using a FIPS geographic region code prefix. In particular,
each row is a single entry for each person with:

1. **Age** as an integer by single year
2. **Home ID** as a 15-character string:
    - 11-digit tract + 4-digit within-tract sequential id
3. **School ID** as a 14-character string:
    - Public: 11-digit tract + 3-digit within-tract sequential id
    - Private: 5-digit county + “xprvx” + 4-digit within-county sequential id
4. **Work ID** as a 16-character string:
    - 11-digit tract + 5-digit within-tract sequential id

*/

use crate::fips_code::FIPSCode;
use std::fmt::{Display, Write};

// Re-exported publicly in `parser.rs`.
#[cfg(feature = "aspr_archive")]
pub mod archive;
pub mod errors;
pub mod parser;

/// A record representing a person in the ASPR synthetic population dataset
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default, Debug)]
pub struct ASPRPersonRecord {
    pub age: u8,
    pub home_id: Option<FIPSCode>,
    pub school_id: Option<FIPSCode>,
    pub work_id: Option<FIPSCode>,
}

impl Display for ASPRPersonRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Age: {}", self.age)?;

        if let Some(home) = &self.home_id {
            write!(f, ", Home: ({})", home)?;
        }
        if let Some(school) = &self.school_id {
            write!(f, ", School: ({})", school)?;
        }
        if let Some(work) = &self.work_id {
            write!(f, ", Work: ({})", work)?;
        }

        Ok(())
    }
}

/// A `SettingCategory` is not a FIPS code but is implicit in the ASPR synthetic population dataset
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default, Debug)]
#[repr(u8)]
pub enum SettingCategory {
    // We expect applications that do not use `SettingCategory` to have this field zeroed out.
    #[default]
    Unspecified = 0,
    Home,
    Workplace,
    PublicSchool,
    PrivateSchool,
    CensusTract,
}

impl SettingCategory {
    /// Decode a numeric value to a `SettingCategory`
    #[inline(always)]
    pub fn decode(value: u8) -> Option<Self> {
        // ToDo: This isn't great, as we need to keep this limit updated with the number of variants in `SettingCategory`.
        if value <= 5 {
            Some(unsafe { std::mem::transmute(value) })
        } else {
            None
        }
    }

    /// Encode a `SettingCategory` as a `u8`
    #[inline(always)]
    pub fn encode(self) -> u8 {
        self as u8
    }
}

impl Display for SettingCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettingCategory::Unspecified => write!(f, "Unspecified"),
            SettingCategory::Home => write!(f, "Home"),
            SettingCategory::Workplace => write!(f, "Workplace"),
            SettingCategory::PublicSchool => write!(f, "Public School"),
            SettingCategory::PrivateSchool => write!(f, "Private School"),
            SettingCategory::CensusTract => write!(f, "Census Tract"),
        }
    }
}

/// This formats the FIPS code as a string according to the ASPR format, which augments FIPS region codes with setting
/// IDs. The category code and "data" field are not represented in this format. However, this function should round-trip
/// for IDs from the ASPR synthetic population dataset.
fn format_as_fips_code<W: Write>(fips_code: FIPSCode, f: &mut W) -> std::fmt::Result {
    write!(f, "{:02}", fips_code.state_code())?;
    write!(f, "{:03}", fips_code.county_code())?;

    match fips_code.category() {
        SettingCategory::Home => {
            // 11-digit tract + 4-digit within-tract sequential id
            write!(f, "{:06}", fips_code.census_tract_code())?;
            write!(f, "{:04}", fips_code.id())
        }

        SettingCategory::Workplace => {
            // 11-digit tract + 5-digit within-tract sequential id
            write!(f, "{:06}", fips_code.census_tract_code())?;
            write!(f, "{:05}", fips_code.id())
        }

        SettingCategory::PublicSchool => {
            // 11-digit tract + 3-digit within-tract sequential id
            write!(f, "{:06}", fips_code.census_tract_code())?;
            write!(f, "{:03}", fips_code.id())
        }

        SettingCategory::PrivateSchool => {
            // 5-digit county + “xprvx” + 4-digit within-county sequential id
            write!(f, "xprvx")?;
            write!(f, "{:04}", fips_code.id())
        }

        // ToDo: Give a reasonable representation for these categories.
        SettingCategory::Unspecified | SettingCategory::CensusTract => Err(std::fmt::Error),
    }
    // The category code and "data" field are not represented in this format.
    // write!(f, "{:01}", fips_code.category_code())?;
    // write!(f, "{:03}", fips_code.data())?;
}

fn format_as_fips_code_string(fips_code: FIPSCode) -> String {
    let mut buf = String::new();
    format_as_fips_code(fips_code, &mut buf).unwrap();
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aspr::parser::{parse_fips_home_id, parse_fips_school_id, parse_fips_workplace_id};

    #[test]
    fn text_round_trip_formatting() {
        let home_id = "110010109000024";
        let workplace_id = "1100100620201546";
        let public_school_id = "11001009810157";
        let private_school_id = "24031xprvx0085";

        let (_, parsed_home_id) = parse_fips_home_id(home_id).unwrap();
        let (_, parsed_workplace_id) = parse_fips_workplace_id(workplace_id).unwrap();
        let (_, parsed_public_school_id) = parse_fips_school_id(public_school_id).unwrap();
        let (_, parsed_private_school_id) = parse_fips_school_id(private_school_id).unwrap();

        assert_eq!(home_id, format_as_fips_code_string(parsed_home_id));
        assert_eq!(
            workplace_id,
            format_as_fips_code_string(parsed_workplace_id)
        );
        assert_eq!(
            public_school_id,
            format_as_fips_code_string(parsed_public_school_id)
        );
        assert_eq!(
            private_school_id,
            format_as_fips_code_string(parsed_private_school_id)
        );
    }
}
