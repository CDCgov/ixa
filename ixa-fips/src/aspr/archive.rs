/*!
This module is enabled with the `aspr_archive` feature and provides facilities to read ASPR synthetic population data.

Set and get the ASPR data path with the `set_aspr_data_path` and `get_aspr_data_path` functions:

```rust
# use ixa_fips::aspr::archive::{get_aspr_data_path, set_aspr_data_path};
# use std::path::PathBuf;
let current_path = get_aspr_data_path();
println!("The current ASPR data path: {:?}", current_path);

set_aspr_data_path(PathBuf::from("../CDC/data/ASPR_Synthetic_Population.zip"));
let new_path = get_aspr_data_path();
println!("The new ASPR data path: {:?}", new_path);
```

You can set the ASPR data path to a zip archive or to a directory. You can refer to subdirectories of the data path
with the `ALL_STATES_DIR`, `CBSA_ALL_DIR`, `CBSA_ONLY_RESIDENTS_DIR`, `NON_CBSA_RESIDENTS_DIR`, and `MULTI_STATE_DIR`
constants for convenience. (These are `&str`s.)

You can iterate over the records in a CSV file under the ASPR data path with the `ASPRRecordIterator` struct. This
struct transparently handles the case that the ASPR data path is a zip archive or a directory for you. Just provide the
path to the CSV file relative to the ASPR data path:

```ignore
# use ixa_fips::aspr::archive::{CBSA_ALL_DIR, ASPRRecordIterator};
# use std::path::PathBuf;
let subdirectory = PathBuf::from(CBSA_ALL_DIR).join("AK/Ketchikan AK.csv");
let records = ASPRRecordIterator::from_path(subdirectory);
// Do something with the records...
```

The `ASPRRecordIterator::state_population()` function is a convenience function that returns an iterator over the
records in `${ASPR_DATA_PATH}/${ALL_STATES_DIR}/${state}.csv`.

```ignore
# use ixa_fips::aspr::archive::{ASPRRecordIterator};
# use ixa_fips::USState;
let records = ASPRRecordIterator::state_population(USState::AK);
// Do something with the records...
```

You can get a list of CSV files in a given subdirectory of the ASPR data path with the `iter_csv_files` function. This
is useful for chaining record iterators using the `from_file_iterator` constructor method:

```ignore
# use ixa_fips::aspr::archive::{iter_csv_files, ALL_STATES_DIR, ASPRRecordIterator};
let records = ASPRRecordIterator::from_file_iterator(iter_csv_files(ALL_STATES_DIR).unwrap());
// Do something with the records...
```

*/

use crate::{
    aspr::{
        errors::ASPRError,
        parser::{parse_fips_home_id, parse_fips_school_id, parse_fips_workplace_id},
        ASPRPersonRecord,
    },
    states::USState,
};
use once_cell::sync::Lazy;
use ouroboros::self_referencing;
use std::{
    fs::File,
    io::Lines,
    io::{BufRead, BufReader},
    path::PathBuf,
    sync::RwLock,
};
use zip::{read::ZipFile, ZipArchive};

// Directory structure of the ASPR data
pub const ALL_STATES_DIR: &str = "all_states";
pub const CBSA_ALL_DIR: &str = "cbsa_all_work_school_household";
pub const CBSA_ONLY_RESIDENTS_DIR: &str = "cbsa_only_residents";
// Either of the next two can be affixed to either of the two above directories
pub const NON_CBSA_RESIDENTS_DIR: &str = "non_CBSA_residents";
pub const MULTI_STATE_DIR: &str = "Multi-state";

// Path to the ASPR data directory
const DEFAULT_ASPR_DATA_PATH: &str = "../CDC/data/ASPR_Synthetic_Population";
// ToDo: Get the ASPR data path from an environment variable.
static ASPR_DATA_PATH: Lazy<RwLock<PathBuf>> =
    Lazy::new(|| RwLock::new(PathBuf::from(DEFAULT_ASPR_DATA_PATH)));

/// Setter for the ASPR data directory path.
pub fn set_aspr_data_path(path: PathBuf) {
    *ASPR_DATA_PATH.write().unwrap() = path;
}

/// Getter for the ASPR data directory path.
pub fn get_aspr_data_path() -> PathBuf {
    ASPR_DATA_PATH.read().unwrap().clone()
}

// region ZipLineIterator

/// Iterator over lines in a particular ASPR data file within a zip archive.
#[self_referencing]
struct ZipLineIterator {
    _archive: ZipArchive<BufReader<File>>,

    // This option is always `Some` after successful construction.
    #[borrows(mut _archive)]
    #[covariant]
    line_iter: Option<Lines<BufReader<ZipFile<'this, BufReader<File>>>>>,
}

impl ZipLineIterator {
    /// Constructs a ZipLineIterator over the lines of the file `path` zipped inside the archive at `archive_path`.
    pub fn from_path(archive_path: PathBuf, path: PathBuf) -> Result<Self, ASPRError> {
        // Open the file with a buffer. These values are consumed.
        let file = File::open(archive_path).map_err(ASPRError::Io)?;
        let reader = BufReader::new(file);
        // Capturing an error during construction is a little awkward.
        let mut maybe_error: Option<ASPRError> = None;

        let zip_line_iter = ZipLineIteratorBuilder {
            _archive: ZipArchive::new(reader).map_err(ASPRError::ZipError)?,

            line_iter_builder: |archive: &mut ZipArchive<BufReader<File>>| match archive
                .by_name(path.to_str().unwrap())
            {
                Ok(zipped_file) => {
                    let buf_zipped_file = BufReader::new(zipped_file);
                    Some(buf_zipped_file.lines())
                }
                Err(e) => {
                    maybe_error = Some(ASPRError::ZipError(e));
                    None
                }
            },
        }
        .build();

        if let Some(e) = maybe_error {
            Err(e)
        } else {
            Ok(zip_line_iter)
        }
    }
}

impl Iterator for ZipLineIterator {
    type Item = Result<String, ASPRError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.with_line_iter_mut(|line_iter| {
            line_iter
                .as_mut()
                .unwrap() // always safe
                .next()
                .map(|r| r.map_err(ASPRError::Io))
        })
    }
}
// endregion ZipLineIterator

/// Interface abstracting over the different ways to iterate over lines in an ASPR data file.
enum LineIterator {
    File(Lines<BufReader<File>>),
    Zip(ZipLineIterator),
}

impl LineIterator {
    pub fn from_path(file_path: PathBuf) -> Result<Self, ASPRError> {
        let path = get_aspr_data_path();

        // The dance to check if the path is a zip archive is ridiculous.
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.eq_ignore_ascii_case("zip"))
            .unwrap_or(false)
        {
            // The path is a zip archive.
            Ok(LineIterator::Zip(ZipLineIterator::from_path(
                path, file_path,
            )?))
        } else {
            // The path is a directory.
            let file = File::open(path.join(file_path)).map_err(ASPRError::Io)?;
            let reader = BufReader::new(file);
            Ok(LineIterator::File(reader.lines()))
        }
    }
}

impl Iterator for LineIterator {
    type Item = Result<String, ASPRError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            LineIterator::File(iter) => iter.next().map(|r| r.map_err(ASPRError::Io)),

            LineIterator::Zip(iter) => iter.next(),
        }
    }
}

/// Returns an iterator over all the data files in the given subdirectory of the ASPR data path. The ASPR data path can
/// be a zip archive or a directory. The paths returned are relative to the ASPR data path.
pub fn iter_csv_files(
    subdirectory: &'static str,
) -> Result<std::vec::IntoIter<PathBuf>, ASPRError> {
    let mut path = get_aspr_data_path();

    // The dance to check if the path is a zip archive is ridiculous.
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
    {
        // Iterator through files within the zip archive.
        let file = File::open(path).map_err(ASPRError::Io)?;
        let reader = BufReader::new(file);
        let archive = ZipArchive::new(reader).map_err(ASPRError::ZipError)?;

        let file_names: Vec<PathBuf> = archive
            .file_names()
            .filter_map(|s| {
                if s.starts_with(subdirectory) {
                    Some(PathBuf::from(s))
                } else {
                    None
                }
            })
            .collect();

        Ok(file_names.into_iter())
    } else {
        // Iterator through files in the directory.
        path.push(subdirectory);
        let mut files = vec![];
        let entries = path.read_dir().map_err(ASPRError::Io)?;

        for entry in entries {
            // We don't use `filter_map` so we can return an error here.
            let entry = entry.map_err(ASPRError::Io)?;
            if entry.path().is_file() {
                files.push(entry.path());
            }
        }

        Ok(files.into_iter())
    }
}

/// Iterator over ASPR records in a particular ASPR data file.
pub struct ASPRRecordIterator {
    line_iter: LineIterator,
}

impl ASPRRecordIterator {
    /// Returns an iterator over the records in `${ASPR_DATA_PATH}/${ALL_STATES_DIR}/${state}.csv`
    pub fn state_population(state: USState) -> Result<Self, ASPRError> {
        let file_name = format!("{}.csv", state.to_string().to_lowercase());
        let mut path = PathBuf::from(ALL_STATES_DIR);
        path.push(file_name);

        Self::from_path(path)
    }

    /// Returns an iterator over the records in `file_path`. This function is intended to be used with the
    /// `iter_csv_files` function.
    pub fn from_path(file_path: PathBuf) -> Result<Self, ASPRError> {
        // let file          = File::open(path.clone()).map_err(ASPRError::Io)?;
        let mut line_iter = LineIterator::from_path(file_path.clone())?;

        // Skip the header row
        if line_iter.next().is_none() {
            // If there is no header row, something is wrong, so return an error.
            return Err(ASPRError::EmptyFile(file_path));
        }

        Ok(Self { line_iter })
    }

    /// Returns an iterator over all the rows of all the files in the iterator. This function is intended to be used with
    /// the `iter_csv_files` function:
    ///
    /// ```ignore
    /// # use ixa_fips::aspr::archive::{iter_csv_files, ALL_STATES_DIR, ASPRRecordIterator};
    /// let records = ASPRRecordIterator::from_file_iterator(iter_csv_files(ALL_STATES_DIR).unwrap());
    /// ```
    pub fn from_file_iterator(
        files: impl Iterator<Item = PathBuf>,
    ) -> impl Iterator<Item = ASPRPersonRecord> {
        // Try to open each file, drop it if Err(_)
        files
            .filter_map(|path| ASPRRecordIterator::from_path(path).ok())
            // Each successful iterator yields records; flatten them all.
            .flatten()
    }
}

impl Iterator for ASPRRecordIterator {
    type Item = ASPRPersonRecord;

    /// Returns the next record in the ASPR data file. This function returns `None` on malformed data. We assume
    /// that the prepared data is well-formed.
    fn next(&mut self) -> Option<Self::Item> {
        let line = (self.line_iter.next()?).ok()?;
        let mut part_iter = line.split(',');

        let age = part_iter.next()?.parse::<u8>().unwrap();

        let home_id_str = part_iter.next()?.trim();
        let home_id = parse_fips_home_id(home_id_str).ok().map(|(_, id)| id);

        let school_id_str = part_iter.next()?.trim();
        let school_id = parse_fips_school_id(school_id_str).ok().map(|(_, id)| id);

        let work_id_str = part_iter.next()?.trim();
        let work_id = parse_fips_workplace_id(work_id_str).ok().map(|(_, id)| id);

        Some(ASPRPersonRecord {
            age,
            home_id,
            school_id,
            work_id,
        })
    }
}

#[cfg(all(feature = "aspr_tests", test))]
mod tests {
    //! These tests assume the existence of data in the default ASPR data path AND the existence of the zip archive
    //! in the default ASPR data path.

    use super::*;

    // Enforce serial execution of tests. Since the "zip" tests change the ASPR data path, we also need to set the
    // ASPR data path to the default value before running the tests.
    static TEST_MUTEX: Lazy<std::sync::Mutex<()>> = Lazy::new(|| std::sync::Mutex::new(()));

    #[test]
    fn test_record_iterator_state_population() {
        let _guard = TEST_MUTEX.lock();
        set_aspr_data_path(PathBuf::from(DEFAULT_ASPR_DATA_PATH));

        let records = ASPRRecordIterator::state_population(USState::WY).unwrap();
        // We count the lines in the file excluding the header:
        //     583,201 - 1 = 583,200
        assert_eq!(records.count(), 583200);
    }

    #[test]
    fn test_record_iterator_from_path() {
        let _guard = TEST_MUTEX.lock();
        set_aspr_data_path(PathBuf::from(DEFAULT_ASPR_DATA_PATH));

        let path = PathBuf::from(CBSA_ALL_DIR).join("AK/Ketchikan AK.csv");
        let records = match ASPRRecordIterator::from_path(path) {
            Ok(records) => records,
            Err(e) => {
                // println!("{:?}", e);
                panic!("{:?}", e);
            }
        };
        // We count the lines in the file excluding the header:
        //     14,133 - 1 = 14,132
        assert_eq!(records.count(), 14132);
    }

    #[test]
    fn test_record_iterator_from_files() {
        let _guard = TEST_MUTEX.lock();
        set_aspr_data_path(PathBuf::from(DEFAULT_ASPR_DATA_PATH));

        let all_path = PathBuf::from(CBSA_ALL_DIR);
        let only_residents_path = PathBuf::from(CBSA_ONLY_RESIDENTS_DIR);
        let paths = vec![
            all_path.join("AK/Ketchikan AK.csv"),
            all_path.join("TX/Vernon TX.csv"),
            only_residents_path.join("AK/Ketchikan AK.csv"),
            only_residents_path.join("TX/Vernon TX.csv"),
        ]
        .into_iter();

        let records = ASPRRecordIterator::from_file_iterator(paths);

        // We sum the count of lines in each file excluding the header:
        //     14,133 + 16,606 + 13,746 + 12,973 - 4 = 57,454
        assert_eq!(records.count(), 57454);
    }

    #[test]
    fn test_state_row_iter() {
        let _guard = TEST_MUTEX.lock();
        set_aspr_data_path(PathBuf::from(DEFAULT_ASPR_DATA_PATH));

        let state = USState::AL;
        let state_records = ASPRRecordIterator::state_population(state).unwrap();

        for (idx, record) in state_records.enumerate() {
            if idx == 10 {
                break;
            }
            println!("{}", record);
        }
    }

    #[test]
    fn test_zip_record_iterator_state_population() {
        let _guard = TEST_MUTEX.lock();

        set_aspr_data_path(get_aspr_data_path().with_extension("zip"));

        let records = ASPRRecordIterator::state_population(USState::WY).unwrap();
        // We count the lines in the file excluding the header:
        //     583,201 - 1 = 583,200
        assert_eq!(records.count(), 583200);
    }

    #[test]
    fn test_zip_record_iterator_from_path() {
        let _guard = TEST_MUTEX.lock();

        set_aspr_data_path(get_aspr_data_path().with_extension("zip"));

        let path = PathBuf::from(CBSA_ALL_DIR).join("AK/Ketchikan AK.csv");
        let records = ASPRRecordIterator::from_path(path).unwrap();
        // We count the lines in the file excluding the header:
        //     14,133 - 1 = 14,132
        assert_eq!(records.count(), 14132);
    }

    #[test]
    fn test_zip_record_iterator_from_files() {
        let _guard = TEST_MUTEX.lock();

        set_aspr_data_path(get_aspr_data_path().with_extension("zip"));

        let all_path = PathBuf::from(CBSA_ALL_DIR);
        let only_residents_path = PathBuf::from(CBSA_ONLY_RESIDENTS_DIR);
        let paths = vec![
            all_path.join("AK/Ketchikan AK.csv"),
            all_path.join("TX/Vernon TX.csv"),
            only_residents_path.join("AK/Ketchikan AK.csv"),
            only_residents_path.join("TX/Vernon TX.csv"),
        ]
        .into_iter();

        let records = ASPRRecordIterator::from_file_iterator(paths);

        // We sum the count of lines in each file excluding the header:
        //     14,133 + 16,606 + 13,746 + 12,973 - 4 = 57,454
        assert_eq!(records.count(), 57454);
    }

    #[test]
    fn test_zip_state_row_iter() {
        let _guard = TEST_MUTEX.lock();

        set_aspr_data_path(get_aspr_data_path().with_extension("zip"));

        let state = USState::AL;
        let state_records = ASPRRecordIterator::state_population(state).unwrap();

        for (idx, record) in state_records.enumerate() {
            if idx == 10 {
                break;
            }
            println!("{}", record);
        }
    }
}
