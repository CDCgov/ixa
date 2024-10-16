use crate::context::Context;
use csv::Writer;
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::path::PathBuf;

// * file_prefix: precedes the report name in the filename. An example of a
// potential prefix might be scenario or simulation name
// * directory: location that the CSVs are written to. An example of this might
// be /data/
pub struct ConfigReportOptions {
    pub file_prefix: String,
    pub directory: PathBuf,
}

impl ConfigReportOptions {
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn new() -> Self {
        // Sets the defaults
        ConfigReportOptions {
            file_prefix: String::new(),
            directory: env::current_dir().unwrap(),
        }
    }
    /// Sets the file prefix option (e.g., "report_")
    pub fn file_prefix(&mut self, file_prefix: String) -> &mut ConfigReportOptions {
        self.file_prefix = file_prefix;
        self
    }
    /// Sets the directory where reports will be output
    pub fn directory(&mut self, directory: PathBuf) -> &mut ConfigReportOptions {
        self.directory = directory;
        self
    }
}

impl Default for ConfigReportOptions {
    fn default() -> Self {
        Self::new()
    }
}

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
    config: ConfigReportOptions,
}

// Registers a data container that stores
// * file_writers: Maps report type to file writer
// * config: Contains all the customizable filename options that the user supplies
crate::context::define_data_plugin!(
    ReportPlugin,
    ReportData,
    ReportData {
        file_writers: RefCell::new(HashMap::new()),
        config: ConfigReportOptions::new(),
    }
);

impl Context {
    // Builds the filename. Called by `add_report`, `short_name` refers to the
    // report type. The three main components are `prefix`, `directory`, and
    // `short_name`.
    fn generate_filename(&mut self, short_name: &str) -> PathBuf {
        let data_container = self.get_data_container_mut(ReportPlugin);
        let prefix = data_container.config.file_prefix.clone();
        let directory = data_container.config.directory.clone();
        let short_name = short_name.to_string();
        let basename = format!("{prefix}{short_name}");
        directory.join(basename).with_extension("csv")
    }
}

pub trait ContextReportExt {
    fn add_report<T: Report + 'static>(&mut self, short_name: &str);
    fn send_report<T: Report>(&self, report: T);
    fn report_options(&mut self) -> &mut ConfigReportOptions;
}

impl ContextReportExt for Context {
    /// Call `add_report` with each report type, passing the name of the report type.
    /// The `short_name` is used for file naming to distinguish what data each
    /// output file points to.
    fn add_report<T: Report + 'static>(&mut self, short_name: &str) {
        let path = self.generate_filename(short_name);

        let data_container = self.get_data_container_mut(ReportPlugin);

        let file = File::create(path).expect("Couldn't create file");
        let writer = Writer::from_writer(file);
        let mut file_writer = data_container.file_writers.borrow_mut();
        file_writer.insert(TypeId::of::<T>(), writer);
    }

    /// Write a new row to the appropriate report file
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

    /// Returns a `ConfigReportOptions` object which has setter methods for report configuration
    fn report_options(&mut self) -> &mut ConfigReportOptions {
        let data_container = self.get_data_container_mut(ReportPlugin);
        &mut data_container.config
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
        let path = PathBuf::from(&temp_dir.path());
        let config = context.report_options();
        config
            .file_prefix("prefix1_".to_string())
            .directory(path.clone());
        context.add_report::<SampleReport>("sample_report");
        let report = SampleReport {
            id: 1,
            value: "Test Value".to_string(),
        };

        context.send_report(report);

        let file_path = path.join("prefix1_sample_report.csv");
        assert!(file_path.exists(), "CSV file should exist");

        let mut reader = csv::Reader::from_path(file_path).unwrap();
        for result in reader.deserialize() {
            let record: SampleReport = result.unwrap();
            assert_eq!(record.id, 1);
            assert_eq!(record.value, "Test Value");
        }
    }

    #[test]
    fn add_report_empty_prefix() {
        let mut context = Context::new();
        let temp_dir = tempdir().unwrap();
        let path = PathBuf::from(&temp_dir.path());
        let config = context.report_options();
        config.directory(path.clone());
        context.add_report::<SampleReport>("sample_report");
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
    fn add_report_no_dir() {
        let mut context = Context::new();
        let temp_dir = tempdir().unwrap();
        let path = PathBuf::from(&temp_dir.path());
        let config = context.report_options();
        config
            .file_prefix("test_prefix_".to_string())
            .directory(path.clone());
        context.add_report::<SampleReport>("sample_report");
        let report = SampleReport {
            id: 1,
            value: "Test Value".to_string(),
        };

        context.send_report(report);

        let file_path = path.join("test_prefix_sample_report.csv");
        assert!(file_path.exists(), "CSV file should exist");

        let mut reader = csv::Reader::from_path(file_path).unwrap();
        for result in reader.deserialize() {
            let record: SampleReport = result.unwrap();
            assert_eq!(record.id, 1);
            assert_eq!(record.value, "Test Value");
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
        let path = PathBuf::from(&temp_dir.path());
        let config = context.report_options();
        config
            .file_prefix("mult_report_".to_string())
            .directory(path.clone());
        context.add_report::<SampleReport>("sample_report");
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
        let base_path = temp_dir.path().to_path_buf();

        for i in 0..num_threads {
            let path = base_path.clone();
            let handle = thread::spawn(move || {
                let mut context = Context::new();
                let config = context.report_options();
                config.file_prefix(i.to_string()).directory(path.clone());
                context.add_report::<SampleReport>("sample_report");

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
