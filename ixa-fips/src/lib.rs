//! # FIPS Geographic Region Code Library
//!
//! FIPS geographic region codes are used to represent hierarchical geographic regions from the state level down to the
//! "block" level. They are augmented in some synthetic population datasets with additional ID numbers for households,
//! workplaces, and schools. This library provides types to represent FIPS geographic region codes (and "code fragments"),
//! efficient representations, and utilities to convert to and from textual representations ([`crate::parser`]).
//!
//! The [`crate::aspr`] module provides types for representing records from the ASPR synthetic population dataset, and the
//! [`crate::aspr::parser`] submodule provides parsers for textual representations of ASPR records.
//!
//! The `aspr_archive` feature (enabled by default) enables the [`crate::aspr::archive`] module, which provides a reader
//! for ASPR synthetic population data files, including files that are within a zip archive.

#![allow(dead_code)]
// Positive instances of the following lints have been audited.
#![allow(clippy::inline_always)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_lossless)]

pub mod fips_code;
pub mod parser;
pub mod states;

pub use fips_code::{ExpandedFIPSCode, FIPSCode};
pub use states::USState;

// Convenience constants
const FOUR_BIT_MASK: u8 = 15; // 2^4-1
const SIX_BIT_MASK: u8 = 63; // 2^6-1
const TEN_BIT_MASK: u16 = 1023; // 2^10-1
const FOURTEEN_BIT_MASK: u16 = 16383; // 2^14-1
const TWENTY_BIT_MASK: u32 = 1048575; // 2^20-1

// Offsets of the bit fields in the encoded FIPS code
const STATE_OFFSET: usize = 58;
const COUNTY_OFFSET: usize = 48;
const TRACT_OFFSET: usize = 28;
const CATEGORY_OFFSET: usize = 24;
const ID_OFFSET: usize = 10;
// const DATA_OFFSET: usize = 0;

// Numeric types used for code fragments. By convention, zero values are reserved for "no data."

/// The numeric type used for the state code fragment; `u8`
pub type StateCode = u8;
/// The numeric type used for the county code fragment; `u16`
pub type CountyCode = u16;
/// The numeric type used for the tract code fragment; `u32`
pub type TractCode = u32;
/// The numeric type used for the setting category code fragment; `u8`
pub type SettingCategoryCode = u8;
/// The numeric type used for the id code fragment; `u16`
pub type IdCode = u16;
/// The numeric type used for the data code fragment; `u16`
pub type DataCode = u16;
