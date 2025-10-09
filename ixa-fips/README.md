# ixa-fips — FIPS geographic codes & utilities

**_Represent, parse, and manipulate hierarchical FIPS region codes in just 64
bits._**

[Federal Information Processing Series (FIPS)](https://www.census.gov/library/reference/code-lists/ansi.html)
is a standardized set of geographic codes useful for identifying places and
regions like counties and voting districts. This library provides an efficient
representation of a subset of these codes, in particular hierarchical region
codes suitable for identifying schools, workplaces, and homes. The primary data
type `FIPSCode` has explicit fields for state, county, and census tract, and has
additional fields convenient for user-defined specificity and even additional
arbitrary data.

## Why this crate?

- **Compact & cache-friendly.** A complete state → county → census-tract
  hierarchy (plus category, id, and extra data) is packed into a single `u64`,
  minimizing memory use and hash-map overhead.
- **Zero-copy parsing.** Fast lexical routines turn text into `FIPSCode` values
  without intermediate heap allocations.

## Standard alignment

This crate is designed to be usable with any revision of the standard. The
standard-dependent properties of this library are:

- The minimal subset of the U.S. state codes that includes only proper states
  and the District of Columbia. The codes for this subset have been stable for
  every revision. This is only relevant to the `USState` enum the use of which
  is optional.
- The number of digits that represent the U.S. state, county, and census tract
  codes (2, 3, and 6 respectively) in the parsing routines. This is a hard
  dependency for the ASPR library but only affects this library in that it
  determines maximum values for these fields.

## Core primitives

| Type / Module | Purpose                                                                                |
| ------------- | -------------------------------------------------------------------------------------- |
| `FIPSCode`    | 64-bit value encoding state + county + tract + category + id _(10 spare bits for you)_ |
| `parser`      | Zero-allocation conversions <br/>`&str` ⇆ `FIPSCode` / fragments                       |
| `USState`     | Exhaustive enum of valid state codes\* (fits in the 6 bits allocated by `FIPSCode`)    |
| `FIPSError`   | Represents value out of range errors.                                                  |

\* This is a minimal subset of FIPS state and state equivalent codes which have
been stable for every FIPS standard revision so far. See the
[2020 FIPS Standard here](https://www.census.gov/library/reference/code-lists/ansi.html#states).

## Quick tour

### 1. Building a code "by hand"

```rust
use ixa_fips::{FIPSCode, USState};

let code  = FIPSCode::with_tract(
    USState::TX,
    201,  // county
    1234, // census tract
);

// Pattern-match the components later:
assert_eq!(code.state(), USState::TX);
```

### **2. Parsing from text**

Example parsing a complete FIPS code (state + county + tract):

```rust
// Example: "01001020100" = Alabama (01), Autauga County (001), Tract 020100
let input = "01001020100";

// First parse the state
let (rest, state) = parse_state_code(input).unwrap();
assert_eq!(rest, "001020100");

// Then parse the county
let (rest, county) = parse_county_code(rest).unwrap();
assert_eq!(rest, "020100");

// Finally parse the tract
let (rest, tract) = parse_tract_code(rest).unwrap();
assert_eq!(rest, "");

// Verify the parsed values (assuming USState enum implementation)
assert_eq!(state, USState::AL);
assert_eq!(county, 1);
assert_eq!(tract, 20100);
```
