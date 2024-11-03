use crate::context::Context;
use crate::error::IxaError;
use csv::Writer;
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{create_dir_all, File};
use std::path::Path;

pub trait Report: 'static {
    // Returns report type
    fn type_id(&self) -> TypeId;
    // Serializes the data with the correct writer
    fn serialize(&self, writer: &mut Writer<File>);
}

/// Use this macro to define a unique report type
#[macro_export]
macro_rules! create_report_trait {
    ($name:ident) => {
        impl Report for $name {
            fn type_id(&self) -> std::any::TypeId {
                std::any::TypeId::of::<$name>()
            }

            fn serialize(&self, writer: &mut csv::Writer<std::fs::File>) {
                writer.serialize(self).unwrap();
            }
        }
    };
}

struct ReportData {
    file_writers: RefCell<HashMap<TypeId, Writer<File>>>,
}

// Registers a data container that stores
// * file_writers: Maps report type to file writer
// * config: Contains all the customizable filename options that the user supplies
crate::context::define_data_plugin!(
    ReportPlugin,
    ReportData,
    ReportData {
        file_writers: RefCell::new(HashMap::new()),
    }
);

// Checks that the path is valid. Creates the file and all parent directories if
// they do not exist. Returns the file if successful. Called by `add_report`
fn generate_validate_filepath(path_name: &str) -> Result<File, IxaError> {
    let path = Path::new(path_name);
    match path.extension().and_then(OsStr::to_str) {
        Some("csv") => {
            create_dir_all(path.parent().expect("Either root or empty path provided"))?;
            let file = File::create(path)?;
            Ok(file)
        }
        _ => Err(IxaError::ReportError(
            "Report output files must be CSVs at this time".to_string(),
        )),
    }
}

pub trait ContextReportExt {
    /// Call `add_report` with each report type, passing the name of the report type.
    /// Takes the complete path to which the output the report as argument.
    /// Returns `Result<(), IxaError>` indicating success.
    ///
    /// # Errors
    ///
    /// Returns an `IxaError` detailing what may have gone wrong
    fn add_report<T: Report + 'static>(&mut self, filepath: &str) -> Result<(), IxaError>;

    /// Write a new row with columns following items in the report struct
    /// to the report file associated with the report type struct.
    fn send_report<T: Report>(&self, report: T);
}

impl ContextReportExt for Context {
    fn add_report<T: Report + 'static>(&mut self, filepath: &str) -> Result<(), IxaError> {
        let file = generate_validate_filepath(filepath)?;

        let data_container = self.get_data_container_mut(ReportPlugin);
        let writer = Writer::from_writer(file);
        let mut file_writer = data_container.file_writers.borrow_mut();
        file_writer.insert(TypeId::of::<T>(), writer);
        Ok(())
    }

    fn send_report<T: Report>(&self, report: T) {
        // No data container will exist if no reports have been added
        let data_container = self
            .get_data_container(ReportPlugin)
            .expect("No writer found for the report type");
        let mut writer_cell = data_container.file_writers.try_borrow_mut().unwrap();
        let writer = writer_cell
            .get_mut(&report.type_id())
            .expect("No writer found for the report type");
        report.serialize(writer);
        writer.flush().expect("Failed to flush writer");
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use core::convert::TryInto;
    use serde_derive::{Deserialize, Serialize};
    use std::thread;
    use tempfile::tempdir;

    #[derive(Serialize, Deserialize)]
    struct SampleReport {
        id: u32,
        value: String,
    }

    create_report_trait!(SampleReport);

    #[test]
    fn add_and_send_report() {
        let mut context = Context::new();
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path();
        context
            .add_report::<SampleReport>(path.join("sample_report.csv").to_str().unwrap())
            .unwrap();
        let report = SampleReport {
            id: 1,
            value: "Test Value".to_string(),
        };

        context.send_report(report);

        let file_path = path.join("sample_report.csv");
        assert!(file_path.exists(), "CSV file should exist");

        let mut reader = csv::Reader::from_path(file_path).unwrap();
        for result in reader.deserialize() {
            let record: SampleReport = result.unwrap();
            assert_eq!(record.id, 1);
            assert_eq!(record.value, "Test Value");
        }
    }

    #[test]
    #[should_panic(expected = "Permission denied (os error 13)")]
    fn directory_creation_error() {
        // way to make this path in a way that could be tested on Windows too?
        let res = generate_validate_filepath("/ixa-temporary-files/sample_report.csv");
        match res {
            Ok(_) => {
                panic!("File should not have been created")
            }
            Err(ixa_error) => match ixa_error {
                IxaError::IoError(error_message) => panic!("{}", error_message),
                _ => panic!("Unexpected error"),
            },
        }
    }

    #[test]
    fn directory_creation_writing_works() {
        let mut context = Context::new();
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path();
        context
            .add_report::<SampleReport>(
                path.join("test-temp")
                    .join("sample_report.csv")
                    .to_str()
                    .unwrap(),
            )
            .unwrap();
        let report = SampleReport {
            id: 1,
            value: "Test Value".to_string(),
        };

        context.send_report(report);

        let file_path = path.join("test-temp").join("sample_report.csv");
        assert!(file_path.exists(), "CSV file should exist");

        let mut reader = csv::Reader::from_path(file_path).unwrap();
        for result in reader.deserialize() {
            let record: SampleReport = result.unwrap();
            assert_eq!(record.id, 1);
            assert_eq!(record.value, "Test Value");
        }
    }

    #[test]
    #[should_panic(expected = "Report output files must be CSVs at this time")]
    fn only_csvs_allowed() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path();
        let res = generate_validate_filepath(path.join("sample_report.tsv").to_str().unwrap());
        match res {
            Ok(_) => {
                panic!("Other file types beyond CSV are not allowed (yet)")
            }
            Err(ixa_error) => match ixa_error {
                IxaError::ReportError(error_message) => panic!("{}", error_message),
                _ => panic!("Unexpected error"),
            },
        }
    }

    #[test]
    #[should_panic(expected = "No writer found for the report type")]
    fn send_report_without_adding_report() {
        let context = Context::new();
        let report = SampleReport {
            id: 1,
            value: "Test Value".to_string(),
        };

        context.send_report(report);
    }

    #[test]
    fn multiple_reports_one_context() {
        let mut context = Context::new();
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path();
        context
            .add_report::<SampleReport>(
                path.join("mult_report_sample_report.csv").to_str().unwrap(),
            )
            .unwrap();
        let report1 = SampleReport {
            id: 1,
            value: "Value,1".to_string(),
        };
        let report2 = SampleReport {
            id: 2,
            value: "Value\n2".to_string(),
        };

        context.send_report(report1);
        context.send_report(report2);

        let file_path = path.join("mult_report_sample_report.csv");
        assert!(file_path.exists(), "CSV file should exist");

        let mut reader = csv::Reader::from_path(file_path).expect("Failed to open CSV file");
        let mut records = reader.deserialize::<SampleReport>();

        let item1: SampleReport = records
            .next()
            .expect("No record found")
            .expect("Failed to deserialize record");
        assert_eq!(item1.id, 1);
        assert_eq!(item1.value, "Value,1");

        let item2: SampleReport = records
            .next()
            .expect("No second record found")
            .expect("Failed to deserialize record");
        assert_eq!(item2.id, 2);
        assert_eq!(item2.value, "Value\n2");
    }

    #[test]
    fn multithreaded_report_generation_thread_local() {
        let num_threads = 10;
        let num_reports_per_thread = 5;

        let mut handles = vec![];
        let temp_dir = tempdir().unwrap();
        // needs to be owned in this test
        let base_path = temp_dir.path().to_path_buf();

        for i in 0..num_threads {
            let path = base_path.clone();
            let handle = thread::spawn(move || {
                let mut context = Context::new();
                context
                    .add_report::<SampleReport>(
                        path.join(format!("{i}sample_report.csv")).to_str().unwrap(),
                    )
                    .unwrap();

                for j in 0..num_reports_per_thread {
                    let report = SampleReport {
                        id: u32::try_from(i * num_reports_per_thread + j).unwrap(),
                        value: format!("Thread {i} Report {j}"),
                    };
                    context.send_report(report);
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Thread failed");
        }

        for i in 0..num_threads {
            let file_name = format!("{i}sample_report.csv");
            let file_path = base_path.join(file_name);
            assert!(file_path.exists(), "CSV file should exist");

            let mut reader = csv::Reader::from_path(file_path).expect("Failed to open CSV file");
            let records = reader.deserialize::<SampleReport>();

            for (j, record) in records.enumerate() {
                let record: SampleReport = record.expect("Failed to deserialize record");
                let id_expected = TryInto::<u32>::try_into(i * num_reports_per_thread + j).unwrap();
                assert_eq!(record.id, id_expected);
            }
        }
    }
}
