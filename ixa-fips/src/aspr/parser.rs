//! These high-level functions parse the concatenated FIPS code and ids.

use crate::{
    aspr::SettingCategory,
    fips_code::FIPSCode,
    parser::{parse_decimal_digits_to_bits, parse_state_code, FIPSParseResult, FIPSParserError},
    states::USState,
    CountyCode, IdCode, TractCode,
};

/// Parses the input as a FIPS code for a home id. Returns `(FIPSCode, rest)`,
/// where `rest` is the remaining input after the FIPS code.
pub fn parse_fips_home_id(input: &str) -> FIPSParseResult<FIPSCode> {
    let (rest, state): (&str, USState) = parse_state_code(input)?;
    let (rest, county): (&str, CountyCode) = parse_county_code(rest)?;
    let (rest, tract): (&str, TractCode) = parse_tract_code(rest)?;
    let (rest, home_id): (&str, IdCode) = parse_home_id(rest)?;

    let fips_code = FIPSCode::new(state, county, tract, SettingCategory::Home, home_id, 0);
    Ok((rest, fips_code))
}

/// Parses the input as a FIPS code for a school id. Returns `(FIPSCode, rest)`,
/// where `rest` is the remaining input after the FIPS code.
pub fn parse_fips_school_id(input: &str) -> FIPSParseResult<FIPSCode> {
    let (rest, state): (&str, USState) = parse_state_code(input)?;
    let (rest, county): (&str, CountyCode) = parse_county_code(rest)?;

    if rest.starts_with("x") {
        // Private school id
        let (rest, school_id): (&str, IdCode) = parse_private_school_id(rest)?;
        let fips_code = FIPSCode::new(
            state,
            county,
            0,
            SettingCategory::PrivateSchool,
            school_id,
            0,
        );
        Ok((rest, fips_code))
    } else {
        // Public school
        // Public schools also have a tract code.
        let (rest, tract): (&str, TractCode) = parse_tract_code(rest)?;
        let (rest, school_id): (&str, IdCode) = parse_public_school_id(rest)?;
        let fips_code = FIPSCode::new(
            state,
            county,
            tract,
            SettingCategory::PublicSchool,
            school_id,
            0,
        );
        Ok((rest, fips_code))
    }
}

/// Parses the input as a FIPS code for a workplace id. Returns `(FIPSCode, rest)`,
/// where `rest` is the remaining input after the FIPS code.
pub fn parse_fips_workplace_id(input: &str) -> FIPSParseResult<FIPSCode> {
    let (rest, state): (&str, USState) = parse_state_code(input)?;
    let (rest, county): (&str, CountyCode) = parse_county_code(rest)?;
    let (rest, tract): (&str, TractCode) = parse_tract_code(rest)?;
    let (rest, workplace_id): (&str, IdCode) = parse_workplace_id(rest)?;

    let fips_code = FIPSCode::new(
        state,
        county,
        tract,
        SettingCategory::Workplace,
        workplace_id,
        0,
    );
    Ok((rest, fips_code))
}

/// Parses the first three digits of `input` as a county
/// code. Enforces the requirement that the value is representable using 10
/// bits (which is tautologically always true).
pub fn parse_county_code(input: &str) -> FIPSParseResult<CountyCode> {
    parse_decimal_digits_to_bits(3, 10, input).map(|(rest, value)| (rest, value as CountyCode))
}

/// Parses the first six digits of `input` as a tract
/// code. Enforces the requirement that the value is representable using 20
/// bits (which is tautologically always true).
pub fn parse_tract_code(input: &str) -> FIPSParseResult<TractCode> {
    parse_decimal_digits_to_bits(6, 20, input).map(|(rest, value)| (rest, value as TractCode))
}

/// Parses the first four digits of `input` as a (monotonically increasing) id
/// number. Enforces the requirement that the value is representable using 14
/// bits (which is tautologically always true).
pub fn parse_home_id(input: &str) -> FIPSParseResult<IdCode> {
    parse_decimal_digits_to_bits(4, 14, input).map(|(rest, value)| (rest, value as IdCode))
}

/// Parses the first four digits of `input` as a (monotonically increasing)
/// id number after stripping `"xprvx"`, if it exists. Enforces the
/// requirement that the value is representable using 11 bits.
pub fn parse_private_school_id(input: &str) -> FIPSParseResult<IdCode> {
    let input = input.strip_prefix("xprvx").unwrap_or(input);
    parse_decimal_digits_to_bits(4, 11, input).map(|(rest, value)| (rest, value as IdCode))
}

/// Parses the first three digits of `input` as a (monotonically increasing)
/// id number. Enforces the requirement that the value is representable using
/// 10 bits (a tautology in this case).
pub fn parse_public_school_id(input: &str) -> FIPSParseResult<IdCode> {
    parse_decimal_digits_to_bits(3, 10, input).map(|(rest, value)| (rest, value as IdCode))
}

/// Parses the first five digits of `input` as a (monotonically increasing)
/// id number. Enforces the requirement that the value is representable using
/// 14 bits.
pub fn parse_workplace_id(input: &str) -> FIPSParseResult<IdCode> {
    parse_decimal_digits_to_bits(5, 14, input).map(|(rest, value)| (rest, value as IdCode))
}

/// Parses the next sequence of decimal digits in `input` without respect to
/// its length of how many bits are required to represent it (thought it must
/// implicitly be at most 64).
pub fn parse_integer(input: &str) -> FIPSParseResult<u64> {
    // Find the first non-digit character
    let digit_end = input
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(input.len());

    // If there are no digits at the start, return None
    if digit_end == 0 {
        return Err((
            input,
            FIPSParserError::InvalidLength {
                expected: 1,
                found: 0,
            },
        ));
    }

    // Parse the digit substring
    let value = match input[..digit_end].parse::<u64>() {
        Ok(v) => v,
        Err(parse_int_error) => match parse_int_error.kind() {
            std::num::IntErrorKind::Empty => {
                return Err((
                    input,
                    FIPSParserError::InvalidLength {
                        expected: 1,
                        found: 0,
                    },
                ));
            }

            std::num::IntErrorKind::InvalidDigit => {
                return Err((
                    input,
                    FIPSParserError::InvalidDigit {
                        found: input.chars().next().unwrap(),
                    },
                ));
            }

            _ => {
                return Err((
                    input,
                    FIPSParserError::ValueExceedsCapacity {
                        value: u64::MAX,
                        capacity: u64::MAX,
                    },
                ));
            }
        },
    };

    Ok((&input[digit_end..], value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StateCode;

    #[test]
    fn test_parse_home_id() {
        // Basic successful parsing
        assert_eq!(parse_home_id("1234rest"), Ok(("rest", 1234)));
        assert_eq!(parse_home_id("0001xyz"), Ok(("xyz", 1)));
        assert_eq!(parse_home_id("9999"), Ok(("", 9999)));

        // Maximum allowed value (14 bits max = 16383)
        assert_eq!(parse_home_id("16383abc"), Ok(("3abc", 1638)));

        // Edge cases
        assert_eq!(parse_home_id("0000test"), Ok(("test", 0)));

        // Error cases
        assert!(parse_home_id("").is_err()); // Empty string
        assert!(parse_home_id("abc").is_err()); // No digits
        assert!(parse_home_id("12").is_err()); // Too few digits
        assert!(parse_home_id("123").is_err()); // Too few digits
    }

    #[test]
    fn test_parse_private_school_id() {
        // Basic successful parsing
        assert_eq!(parse_private_school_id("1234rest"), Ok(("rest", 1234)));
        assert_eq!(parse_private_school_id("0001xyz"), Ok(("xyz", 1)));

        // With 'xprvx' prefix
        assert_eq!(parse_private_school_id("xprvx1234rest"), Ok(("rest", 1234)));
        assert_eq!(parse_private_school_id("xprvx0001xyz"), Ok(("xyz", 1)));

        // Maximum allowed value (11 bits max = 2047)
        assert_eq!(parse_private_school_id("2047"), Ok(("", 2047)));
        assert_eq!(parse_private_school_id("xprvx2047"), Ok(("", 2047)));

        // Edge cases
        assert_eq!(parse_private_school_id("0000test"), Ok(("test", 0)));
        assert_eq!(parse_private_school_id("xprvx0000test"), Ok(("test", 0)));

        // Error cases
        assert!(parse_private_school_id("").is_err()); // Empty string
        assert!(parse_private_school_id("xprvx").is_err()); // Empty after prefix
        assert!(parse_private_school_id("xprvxabc").is_err()); // No digits after prefix
        assert!(parse_private_school_id("2048").is_err()); // Exceeds 11 bits
        assert!(parse_private_school_id("xprvx2048").is_err()); // Exceeds 11 bits
    }

    #[test]
    fn test_parse_public_school_id() {
        // Basic successful parsing
        assert_eq!(parse_public_school_id("123rest"), Ok(("rest", 123)));
        assert_eq!(parse_public_school_id("001xyz"), Ok(("xyz", 1)));
        assert_eq!(parse_public_school_id("999"), Ok(("", 999)));

        // Maximum allowed value (10 bits max = 1023)
        assert_eq!(parse_public_school_id("1023abc"), Ok(("3abc", 102)));

        // Edge cases
        assert_eq!(parse_public_school_id("000test"), Ok(("test", 0)));

        // Error cases
        assert!(parse_public_school_id("").is_err()); // Empty string
        assert!(parse_public_school_id("abc").is_err()); // No digits
        assert!(parse_public_school_id("12").is_err()); // Too few digits
    }

    #[test]
    fn test_parse_workplace_id() {
        // Basic successful parsing
        assert_eq!(parse_workplace_id("12345rest"), Ok(("rest", 12345)));
        assert_eq!(parse_workplace_id("00001xyz"), Ok(("xyz", 1)));
        assert_eq!(parse_workplace_id("10383"), Ok(("", 10383)));

        // Maximum allowed value (14 bits max = 16383)
        assert_eq!(parse_workplace_id("16383abc"), Ok(("abc", 16383)));

        // Edge cases
        assert_eq!(parse_workplace_id("00000test"), Ok(("test", 0)));

        // Error cases
        assert!(parse_workplace_id("").is_err()); // Empty string
        assert!(parse_workplace_id("abc").is_err()); // No digits
        assert!(parse_workplace_id("1234").is_err()); // Too few digits
        assert!(parse_workplace_id("16384").is_err()); // Exceeds 14 bits
    }

    #[test]
    fn test_fips_home_id() {
        let home_id = "110010109000024";
        let state_code: StateCode = 11;
        let county_code: CountyCode = 1;
        let tract_code: TractCode = 10900;
        let home_id_code = 24;

        let (_, parsed_home_id) = parse_fips_home_id(home_id).unwrap();

        assert_eq!(parsed_home_id.state_code(), state_code);
        assert_eq!(parsed_home_id.county_code(), county_code);
        assert_eq!(parsed_home_id.census_tract_code(), tract_code);
        assert_eq!(parsed_home_id.id(), home_id_code);
    }

    #[test]
    fn test_fips_work_id() {
        let workplace_id = "1100100620201546";
        let state_code: StateCode = 11;
        let county_code: CountyCode = 1;
        let tract_code: TractCode = 6202;
        let workplace_id_code = 1546;

        let (_, parsed_workplace_id) = parse_fips_workplace_id(workplace_id).unwrap();

        assert_eq!(parsed_workplace_id.state_code(), state_code);
        assert_eq!(parsed_workplace_id.county_code(), county_code);
        assert_eq!(parsed_workplace_id.census_tract_code(), tract_code);
        assert_eq!(parsed_workplace_id.id(), workplace_id_code);
    }

    #[test]
    fn test_fips_public_school_id() {
        let public_school_id = "11001009810157";
        let state_code: StateCode = 11;
        let county_code: CountyCode = 1;
        let tract_code: TractCode = 9810;
        let public_school_id_code = 157;

        let (_, parsed_public_school_id) = parse_fips_school_id(public_school_id).unwrap();

        assert_eq!(parsed_public_school_id.state_code(), state_code);
        assert_eq!(parsed_public_school_id.county_code(), county_code);
        assert_eq!(parsed_public_school_id.census_tract_code(), tract_code);
        assert_eq!(parsed_public_school_id.id(), public_school_id_code);
    }

    #[test]
    fn test_fips_private_school_id() {
        let private_school_id = "24031xprvx0150";
        let state_code: StateCode = 24;
        let county_code: CountyCode = 31;
        let tract_code: TractCode = 0;
        let private_school_id_code = 150;

        let (_, parsed_private_school_id) = parse_fips_school_id(private_school_id).unwrap();

        assert_eq!(parsed_private_school_id.state_code(), state_code);
        assert_eq!(parsed_private_school_id.county_code(), county_code);
        assert_eq!(parsed_private_school_id.census_tract_code(), tract_code);
        assert_eq!(parsed_private_school_id.id(), private_school_id_code);
    }

    #[test]
    fn test_parse_integer() {
        // Basic successful parsing
        assert_eq!(parse_integer("123rest"), Ok(("rest", 123)));
        assert_eq!(parse_integer("0xyz"), Ok(("xyz", 0)));
        assert_eq!(parse_integer("9876543210"), Ok(("", 9876543210)));

        // Single digit
        assert_eq!(parse_integer("5abc"), Ok(("abc", 5)));

        // Long number
        assert_eq!(
            parse_integer("18446744073709551615end"),
            Ok(("end", 18446744073709551615))
        ); // u64 max

        // Error cases
        assert!(parse_integer("").is_err()); // Empty string
        assert!(parse_integer("abc").is_err()); // No digits
    }

    // Additional combined tests
    #[test]
    fn test_combined_scenarios() {
        // Test with leading zeros
        assert_eq!(parse_home_id("0123"), Ok(("", 123)));
        assert_eq!(parse_private_school_id("xprvx0042"), Ok(("", 42)));

        // Test with excess digits
        assert_eq!(parse_public_school_id("12345"), Ok(("45", 123)));

        // Test with value equal to max
        assert_eq!(parse_workplace_id("16383@#$%"), Ok(("@#$%", 16383)));

        // Test with value exceeding max
        assert_eq!(
            parse_workplace_id("19876@#$%"),
            Err((
                "19876@#$%",
                FIPSParserError::ValueExceedsCapacity {
                    value: 19876,
                    capacity: 16383
                }
            ))
        );

        // Test with special characters after digits
        assert_eq!(parse_workplace_id("16380@#$%"), Ok(("@#$%", 16380)));
    }
}
