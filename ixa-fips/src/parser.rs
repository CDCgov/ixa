/*!

Simple parsing utilities for parsing text representations of FIPS codes and variations thereupon.

The concatenative structure of hierarchical FIPS geographic region codes, the (decimal)
digit count of code fragments, and the bit count (defined by this implementation) allowed
for each code fragment, are described in detail in the library level documentation

*/

use std::fmt::{Debug, Display};

use crate::states::USState;

/// The FIPS parser error type.
/// The assumption is that the parsing context is so small that it isn't necessary to track source location information.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum FIPSParserError {
    InvalidDigit { found: char },
    InvalidLength { expected: u32, found: u32 },
    ValueExceedsCapacity { value: u64, capacity: u64 },
}

impl Display for FIPSParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FIPSParserError::InvalidDigit { found } => write!(f, "Invalid digit: {}", found),
            FIPSParserError::InvalidLength { expected, found } => {
                write!(f, "Expected {} characters, found {}", expected, found)
            }
            FIPSParserError::ValueExceedsCapacity { value, capacity } => {
                write!(f, "Value {} exceeds max capacity {}", value, capacity)
            }
        }
    }
}

impl Debug for FIPSParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self, f)
    }
}

impl std::error::Error for FIPSParserError {}

/// Similar to how Nom structures its results. We have:
///   `I`: The input type, i.e. `&str`
///   `O`: The output type, i.e. `u32`
///   `E`: The error type returns a tuple of the original input and the error.
/// A successful result consists of the remaining unparsed input and the parsed value.
pub type IResult<I, O, E = (I, FIPSParserError)> = Result<(I, O), E>;
pub type FIPSParseResult<'a, T> = IResult<&'a str, T>;

/// A function that parses a specified number of decimal digits, enforcing the
/// constraint that the parsed value of those digits be representable by the
/// specified number of binary bits. Upon success, returns the remainder of the
/// input after consuming the parsed digits together with the value of the
/// parsed digits. If there is an error, the original input is returned along
/// with the `FIPSParserError` variant describing the error.
///
/// This function assumes ASCII decimal digits. (The rest of the string can by any  valid UTF-8.)
pub(crate) fn parse_decimal_digits_to_bits(
    digit_count: u32,
    bit_count: u8,
    input: &str,
) -> IResult<&str, u64> {
    let maximum_allowed_value = (1u64 << bit_count) - 1;
    let mut input_bytes = input.as_bytes().iter();
    let mut computed_value: u64 = 0;

    for idx in 0..digit_count {
        match input_bytes.next() {
            Some(c) => {
                if c.is_ascii_digit() {
                    computed_value = 10 * computed_value + (c - b'0') as u64;
                } else {
                    return Err((
                        input,
                        FIPSParserError::InvalidDigit {
                            // The UTF-8 encoded character at `idx` might not be represented as a single byte.
                            // However, as we assume ASCII decimal digits, we are gauranteed that the first
                            // `idx-1` bytes represent `idx-1` characters.
                            found: input.chars().nth(idx as usize).unwrap(),
                        },
                    ));
                }
            }

            None => {
                // Ran out of digits before we were done parsing.
                return Err((
                    input,
                    FIPSParserError::InvalidLength {
                        expected: digit_count,
                        found: idx,
                    },
                ));
            }
        } // end match next byte
    } // end for idx

    // Enforce the bit count constraint.
    if computed_value > maximum_allowed_value {
        return Err((
            input,
            FIPSParserError::ValueExceedsCapacity {
                value: computed_value,
                capacity: maximum_allowed_value,
            },
        ));
    }

    let remaining = &input[digit_count as usize..];
    Ok((remaining, computed_value))
}

/// Parses the first two decimal digits of `input` into a `USState` enum variant.
///
/// This method is only intended to parse states for which `state.is_state()` is
/// true. In particular, it enforces the constraint that the decimal value be
/// representable by 6 binary bits (so values <= 63).
pub fn parse_state_code(input: &str) -> FIPSParseResult<USState> {
    parse_decimal_digits_to_bits(2, 6, input).map(|(rest, value)| {
        // The `parse_decimal_digits_to_bits` function guarantees `value` fits in 6
        // bits, so this unwrap always succeeds.
        let state = unsafe { USState::decode(value as u8).unwrap_unchecked() };
        (rest, state)
    })
}

/// Parses the first three digits of `input` as a FIPS county code. Enforces the
/// requirement that the value fit into 10 bits (a tautology in this case).
pub fn parse_county_code(input: &str) -> FIPSParseResult<u16> {
    parse_decimal_digits_to_bits(3, 10, input).map(|(rest, value)| {
        // The `parse_decimal_digits_to_bits` function guarantees `value` fits in 10 bits.
        (rest, value as u16)
    })
}

/// Parses the first six digits of `input` as a FIPS census tract code. Enforces the
/// requirement that the value fit into 20 bits (a tautology in this case).
pub fn parse_tract_code(input: &str) -> FIPSParseResult<u32> {
    parse_decimal_digits_to_bits(6, 20, input).map(|(rest, value)| {
        // The `parse_decimal_digits_to_bits` function guarantees `value` fits in 20 bits.
        (rest, value as u32)
    })
}

// #[allow(unused_imports)]
// pub use crate::aspr::parser::{
//   parse_home_id,
//   parse_public_school_id,
//   parse_private_school_id,
//   parse_workplace_id,
//   parse_integer
// };

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_decimal_digits_to_bits_valid_cases() {
        // Test with different digit and bit counts
        assert_eq!(
            parse_decimal_digits_to_bits(2, 6, "42rest"),
            Ok(("rest", 42))
        );
        assert_eq!(
            parse_decimal_digits_to_bits(3, 10, "123more"),
            Ok(("more", 123))
        );
        assert_eq!(parse_decimal_digits_to_bits(1, 4, "7end"), Ok(("end", 7)));
        assert_eq!(
            parse_decimal_digits_to_bits(6, 20, "123456extra"),
            Ok(("extra", 123456))
        );

        // Test maximum values for given bit constraints
        assert_eq!(
            parse_decimal_digits_to_bits(2, 6, "63text"),
            Ok(("text", 63))
        );
        assert_eq!(
            parse_decimal_digits_to_bits(3, 10, "999text"),
            Ok(("text", 999))
        );
        assert_eq!(
            parse_decimal_digits_to_bits(6, 20, "999999text"),
            Ok(("text", 999999))
        );
    }

    #[test]
    fn test_parse_decimal_digits_to_bits_invalid_cases() {
        // Too few digits in input
        assert!(parse_decimal_digits_to_bits(2, 6, "4").is_err());
        assert!(parse_decimal_digits_to_bits(3, 10, "12").is_err());

        // Non-digit characters
        assert!(parse_decimal_digits_to_bits(2, 6, "a4rest").is_err());
        assert!(parse_decimal_digits_to_bits(3, 10, "1x3more").is_err());

        // Value exceeds bit constraint
        assert!(parse_decimal_digits_to_bits(2, 6, "64text").is_err()); // 64 doesn't fit in 6 bits
        assert_eq!(
            parse_decimal_digits_to_bits(3, 8, "256text"),
            Err((
                "256text",
                FIPSParserError::ValueExceedsCapacity {
                    value: 256,
                    capacity: 255
                }
            ))
        ); // 256 doesn't fit in 8 bits
        assert_eq!(
            parse_decimal_digits_to_bits(7, 20, "1048576text"),
            Err((
                "1048576text",
                FIPSParserError::ValueExceedsCapacity {
                    value: 1048576,
                    capacity: 1048575
                }
            ))
        ); // 2^20 = 1048576
    }

    #[test]
    fn test_parse_state_code_valid_cases() {
        // Test with valid state codes (assuming implementation of USState enum)
        assert!(parse_state_code("01rest").is_ok()); // Alabama
        assert!(parse_state_code("06rest").is_ok()); // California
        assert!(parse_state_code("48rest").is_ok()); // Texas
        assert!(parse_state_code("36rest").is_ok()); // New York

        // Check that remainder is correctly returned
        let (remainder, _) = parse_state_code("42Pennsylvania").unwrap();
        assert_eq!(remainder, "Pennsylvania");
    }

    #[test]
    fn test_parse_state_code_invalid_cases() {
        // Value exceeds 6 bits
        assert!(parse_state_code("64rest").is_err());
        assert!(parse_state_code("99rest").is_err());

        // Non-digit characters
        assert!(parse_state_code("A1rest").is_err());

        // Too few digits
        assert!(parse_state_code("4").is_err());

        // Empty input
        assert!(parse_state_code("").is_err());
    }

    #[test]
    fn test_parse_county_code_valid_cases() {
        // Test with valid county codes
        assert_eq!(parse_county_code("001rest").unwrap().1, 1);
        assert_eq!(parse_county_code("123rest").unwrap().1, 123);
        assert_eq!(parse_county_code("999rest").unwrap().1, 999);

        // Check that remainder is correctly returned
        let (remainder, _) = parse_county_code("001CountyName").unwrap();
        assert_eq!(remainder, "CountyName");
    }

    #[test]
    fn test_parse_county_code_invalid_cases() {
        // Non-digit characters
        assert_eq!(
            parse_county_code("x01rest"),
            Err(("x01rest", FIPSParserError::InvalidDigit { found: 'x' }))
        );

        // Too few digits
        assert_eq!(
            parse_county_code("12"),
            Err((
                "12",
                FIPSParserError::InvalidLength {
                    expected: 3,
                    found: 2
                }
            ))
        );

        // Empty input
        assert_eq!(
            parse_county_code(""),
            Err((
                "",
                FIPSParserError::InvalidLength {
                    expected: 3,
                    found: 0
                }
            ))
        );
    }

    #[test]
    fn test_parse_tract_code_valid_cases() {
        // Test with valid tract codes
        assert_eq!(parse_tract_code("000001rest").unwrap().1, 1);
        assert_eq!(parse_tract_code("123456rest").unwrap().1, 123456);
        assert_eq!(parse_tract_code("999999rest").unwrap().1, 999999);

        // Check that remainder is correctly returned
        let (remainder, _) = parse_tract_code("123456TractInfo").unwrap();
        assert_eq!(remainder, "TractInfo");
    }

    #[test]
    fn test_parse_tract_code_invalid_cases() {
        // Non-digit characters
        assert!(parse_tract_code("12345xrest").is_err());

        // Too few digits
        assert!(parse_tract_code("12345").is_err());

        // Empty input
        assert!(parse_tract_code("").is_err());
    }

    #[test]
    fn test_integration_fips_parsing() {
        // Test parsing a complete FIPS code (state + county + tract)
        // Example: "01001020100" = Alabama (01), Autauga County (001), Tract 020100

        let input = "01001020100RestOfData";

        // First parse the state
        let (remainder1, state) = parse_state_code(input).unwrap();
        assert_eq!(remainder1, "001020100RestOfData");

        // Then parse the county
        let (remainder2, county) = parse_county_code(remainder1).unwrap();
        assert_eq!(remainder2, "020100RestOfData");

        // Finally parse the tract
        let (remainder3, tract) = parse_tract_code(remainder2).unwrap();
        assert_eq!(remainder3, "RestOfData");

        // Verify the parsed values (assuming USState enum implementation)
        assert_eq!(state, USState::AL);
        assert_eq!(county, 1);
        assert_eq!(tract, 20100);
    }
}
