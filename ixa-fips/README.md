# fips — High-performance FIPS geographic codes & ASPR dataset utilities

***Represent, parse, and manipulate hierarchical FIPS region codes in just 64 bits ― with first-class support for the
ASPR synthetic-population files.***

## Why this crate?

* **Compact & cache-friendly.** A complete state → county → census-tract hierarchy (plus category, id, and extra data)
  is packed into a single `u64`, minimizing memory use and hash-map overhead.
* **Zero-copy parsing.** Fast lexical routines turn text into `FIPSCode` values without intermediate heap allocations.
* **Batteries included for the ASPR dataset.** Transparent iterators load ASPR CSVs whether they live in a directory
  tree *or* a ZIP archive, so large synthetic populations stream in record-by-record.
* **Ergonomic, strongly-typed API.** Rich enums (`USState`, `SettingCategory`, …) prevent illegal states at compile time
  and make downstream code self-documenting.

## Core primitives

| Type / Module   | Purpose                                                                                                     |
|-----------------|-------------------------------------------------------------------------------------------------------------|
| `FIPSCode`      | 64-bit value encoding state + county + tract + category + id *(10 spare bits for you)*                      |
| `parser`        | Zero-allocation conversions <br/>`&str` ⇆ `FIPSCode` / fragments                                            |
| `USState`       | Exhaustive enum of valid state codes (fits in the 6 bits allocated by `FIPSCode`)                           |
| `aspr`          | Helpers for the **ASPR synthetic-population** files <br/>`ASPRPersonRecord`, parsers                        |
| `aspr::archive` | (With feature `aspr_archive`) <br/>Reads ASPR CSVs inside a directory *or* a ZIP without changing your code |

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

(The grammar understands plain hierarchical strings as well as extended from the ASPR synthetic population dataset.)

### 3. Streaming a synthetic population

```rust
use ixa_fips::aspr::archive::{ASPRRecordIterator, set_aspr_data_path};
use ixa_fips::USState;

set_aspr_data_path(PathBuf::from("../CDC/data/ASPR_Synthetic_Population.zip"));

for person in ASPRRecordIterator::state_population(USState::AK) {
    if let Ok(record) = person {
        // record.age, record.home_id, record.school_id, record.work_id …
        println ! ("{}", record);
    }
}
```

The iterator automatically detects whether the `ASPR_DATA_PATH` you configured is a directory tree or a zipped archive.

## Feature flags

| **Feature**    | **Default?** | **Adds…**                                            |
|----------------|--------------|------------------------------------------------------|
| `aspr_archive` | Yes          | `aspr::archive` — seamless ZIP/dir record iterators. |
