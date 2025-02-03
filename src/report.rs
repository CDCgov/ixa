use crate::context::Context;
use crate::error::IxaError;
use crate::people::ContextPeopleExt;
use crate::Tabulator;
use crate::{error, trace};
use csv::Writer;
use std::any::TypeId;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::path::PathBuf;

// * file_prefix: precedes the report name in the filename. An example of a
// potential prefix might be scenario or simulation name
// * directory: location that the CSVs are written to. An example of this might
// be /data/
// * overwrite: if true, will overwrite existing files in the same location
pub struct ConfigReportOptions {
    pub file_prefix: String,
    pub output_dir: PathBuf,
    pub overwrite: bool,
}

impl ConfigReportOptions {
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn new() -> Self {
        trace!("new ConfigReportOptions");
        // Sets the defaults
        ConfigReportOptions {
            file_prefix: String::new(),
            output_dir: env::current_dir().unwrap(),
            overwrite: false,
        }
    }
    /// Sets the file prefix option (e.g., "report_")
    pub fn file_prefix(&mut self, file_prefix: String) -> &mut ConfigReportOptions {
        trace!("setting report prefix to {}", file_prefix);
        self.file_prefix = file_prefix;
        self
    }
    /// Sets the directory where reports will be output
    pub fn directory(&mut self, directory: PathBuf) -> &mut ConfigReportOptions {
        trace!("setting report directory to {:?}", directory);
        self.output_dir = directory;
        self
    }
    /// Sets whether to overwrite existing reports of the same name if they exist
    pub fn overwrite(&mut self, overwrite: bool) -> &mut ConfigReportOptions {
        trace!("setting report overwrite {}", overwrite);
        self.overwrite = overwrite;
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
        let prefix = &data_container.config.file_prefix;
        let directory = &data_container.config.output_dir;
        let short_name = short_name.to_string();
        let basename = format!("{prefix}{short_name}");
        directory.join(basename).with_extension("csv")
    }
}

pub trait ContextReportExt {
    /// Add a report file keyed by a `TypeId`.
    /// The `short_name` is used for file naming to distinguish what data each
    /// output file points to.
    /// # Errors
    /// If the file already exists and `overwrite` is set to false, raises an error and info message.
    /// If the file cannot be created, raises an error.
    fn add_report_by_type_id(&mut self, type_id: TypeId, short_name: &str) -> Result<(), IxaError>;

    /// Call `add_report` with each report type, passing the name of the report type.
    /// The `short_name` is used for file naming to distinguish what data each
    /// output file points to.
    /// # Errors
    /// If the file already exists and `overwrite` is set to false, raises an error and info message.
    /// If the file cannot be created, raises an error.
    fn add_report<T: Report + 'static>(&mut self, short_name: &str) -> Result<(), IxaError>;

    /// Adds a periodic report at the end of period `period` which summarizes the
    /// number of people in each combination of properties in `tabulator`.
    /// # Errors
    /// If the file already exists and `overwrite` is set to false, raises an error and info message.
    /// If the file cannot be created, returns [`IxaError`]
    fn add_periodic_report<T: Tabulator + Clone + 'static>(
        &mut self,
        short_name: &str,
        period: f64,
        tabulator: T,
    ) -> Result<(), IxaError>;
    fn get_writer(&self, type_id: TypeId) -> RefMut<Writer<File>>;
    fn send_report<T: Report>(&self, report: T);
    fn report_options(&mut self) -> &mut ConfigReportOptions;
}

impl ContextReportExt for Context {
    fn add_report_by_type_id(&mut self, type_id: TypeId, short_name: &str) -> Result<(), IxaError> {
        trace!("adding report {} by type_id {:?}", short_name, type_id);
        let path = self.generate_filename(short_name);

        let data_container = self.get_data_container_mut(ReportPlugin);

        let file_creation_result = File::create_new(&path);
        let created_file = match file_creation_result {
            Ok(file) => file,
            Err(e) => match e.kind() {
                std::io::ErrorKind::AlreadyExists => {
                    if data_container.config.overwrite {
                        File::create(&path)?
                    } else {
                        error!("File already exists: {}. Please set `overwrite` to true in the file configuration and rerun.", path.display());
                        return Err(IxaError::IoError(e));
                    }
                }
                _ => {
                    return Err(IxaError::IoError(e));
                }
            },
        };
        let writer = Writer::from_writer(created_file);
        let mut file_writer = data_container.file_writers.borrow_mut();
        file_writer.insert(type_id, writer);
        Ok(())
    }
    fn add_report<T: Report + 'static>(&mut self, short_name: &str) -> Result<(), IxaError> {
        trace!("Adding report {}", short_name);
        self.add_report_by_type_id(TypeId::of::<T>(), short_name)
    }
    fn add_periodic_report<T: Tabulator + Clone + 'static>(
        &mut self,
        short_name: &str,
        period: f64,
        tabulator: T,
    ) -> Result<(), IxaError> {
        trace!("Adding periodic report {}", short_name);

        self.add_report_by_type_id(TypeId::of::<T>(), short_name)?;

        {
            // Write the header
            let mut writer = self.get_writer(TypeId::of::<T>());
            let columns = tabulator.get_columns();
            let mut header = vec!["t".to_string()];
            header.extend(columns);
            header.push("count".to_string());
            writer
                .write_record(&header)
                .expect("Failed to write header");
        }

        self.add_periodic_plan_with_phase(
            period,
            move |context: &mut Context| {
                context.tabulate_person_properties(&tabulator, move |context, values, count| {
                    let mut writer = context.get_writer(TypeId::of::<T>());
                    let mut row = vec![context.get_current_time().to_string()];
                    row.extend(values.to_owned());
                    row.push(count.to_string());

                    writer.write_record(&row).expect("Failed to write row");
                });
            },
            crate::context::ExecutionPhase::Last,
        );

        Ok(())
    }

    fn get_writer(&self, type_id: TypeId) -> RefMut<Writer<File>> {
        // No data container will exist if no reports have been added
        let data_container = self
            .get_data_container(ReportPlugin)
            .expect("No writer found for the report type");
        let writers = data_container.file_writers.try_borrow_mut().unwrap();
        RefMut::map(writers, |writers| {
            writers
                .get_mut(&type_id)
                .expect("No writer found for the report type")
        })
    }

    /// Write a new row to the appropriate report file
    fn send_report<T: Report>(&self, report: T) {
        let writer = &mut self.get_writer(report.type_id());
        report.serialize(writer);
    }

    /// Returns a `ConfigReportOptions` object which has setter methods for report configuration
    fn report_options(&mut self) -> &mut ConfigReportOptions {
        let data_container = self.get_data_container_mut(ReportPlugin);
        &mut data_container.config
    }
}

#[cfg(test)]
mod test {
    use crate::define_person_property_with_default;

    use super::*;
    use core::convert::TryInto;
    use serde_derive::{Deserialize, Serialize};
    use std::thread;
    use tempfile::tempdir;

    define_person_property_with_default!(IsRunner, bool, false);

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
        context.add_report::<SampleReport>("sample_report").unwrap();
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
        context.add_report::<SampleReport>("sample_report").unwrap();
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

    struct PathBufWithDrop {
        file: PathBuf,
    }

    impl Drop for PathBufWithDrop {
        fn drop(&mut self) {
            std::fs::remove_file(&self.file).unwrap();
        }
    }

    #[test]
    fn add_report_no_dir() {
        let mut context = Context::new();
        let config = context.report_options();
        config.file_prefix("test_prefix_".to_string());
        context.add_report::<SampleReport>("sample_report").unwrap();
        let report = SampleReport {
            id: 1,
            value: "Test Value".to_string(),
        };

        context.send_report(report);

        let path = env::current_dir().unwrap();
        let file_path = PathBufWithDrop {
            file: path.join("test_prefix_sample_report.csv"),
        };
        assert!(file_path.file.exists(), "CSV file should exist");

        let mut reader = csv::Reader::from_path(&file_path.file).unwrap();
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
        let temp_dir = tempdir().unwrap();
        let path = PathBuf::from(&temp_dir.path());
        // We need the writer to go out of scope so the file is flushed
        {
            let mut context = Context::new();
            let config = context.report_options();
            config
                .file_prefix("mult_report_".to_string())
                .directory(path.clone());
            context.add_report::<SampleReport>("sample_report").unwrap();
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
        }

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
                config.file_prefix(i.to_string()).directory(path);
                context.add_report::<SampleReport>("sample_report").unwrap();

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

    #[test]
    fn dont_overwrite_report() {
        let mut context1 = Context::new();
        let temp_dir = tempdir().unwrap();
        let path = PathBuf::from(&temp_dir.path());
        let config = context1.report_options();
        config
            .file_prefix("prefix1_".to_string())
            .directory(path.clone());
        context1
            .add_report::<SampleReport>("sample_report")
            .unwrap();
        let report = SampleReport {
            id: 1,
            value: "Test Value".to_string(),
        };

        context1.send_report(report);

        let file_path = path.join("prefix1_sample_report.csv");
        assert!(file_path.exists(), "CSV file should exist");

        let mut context2 = Context::new();
        let config = context2.report_options();
        config.file_prefix("prefix1_".to_string()).directory(path);
        let result = context2.add_report::<SampleReport>("sample_report");
        assert!(result.is_err());
        let error = result.err().unwrap();
        match error {
            IxaError::IoError(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists);
            }
            _ => {
                panic!("Unexpected error type");
            }
        }
    }

    #[test]
    fn overwrite_report() {
        let mut context1 = Context::new();
        let temp_dir = tempdir().unwrap();
        let path = PathBuf::from(&temp_dir.path());
        let config = context1.report_options();
        config
            .file_prefix("prefix1_".to_string())
            .directory(path.clone());
        context1
            .add_report::<SampleReport>("sample_report")
            .unwrap();
        let report = SampleReport {
            id: 1,
            value: "Test Value".to_string(),
        };

        context1.send_report(report);

        let file_path = path.join("prefix1_sample_report.csv");
        assert!(file_path.exists(), "CSV file should exist");

        let mut context2 = Context::new();
        let config = context2.report_options();
        config
            .file_prefix("prefix1_".to_string())
            .directory(path)
            .overwrite(true);
        let result = context2.add_report::<SampleReport>("sample_report");
        assert!(result.is_ok());
        let file = File::open(file_path).unwrap();
        let reader = csv::Reader::from_reader(file);
        let records = reader.into_records();
        assert_eq!(records.count(), 0);
    }

    #[test]
    fn add_periodic_report() {
        let temp_dir = tempdir().unwrap();
        let path = PathBuf::from(&temp_dir.path());
        // We need the writer to go out of scope so the file is flushed
        {
            let mut context = Context::new();
            let config = context.report_options();
            config
                .file_prefix("test_".to_string())
                .directory(path.clone());
            let _ = context.add_periodic_report("periodic", 1.2, (IsRunner,));
            let person = context.add_person(()).unwrap();
            context.add_person(()).unwrap();

            context.add_plan(1.2, move |context: &mut Context| {
                context.set_person_property(person, IsRunner, true);
            });

            context.execute();
        }

        let file_path = path.join("test_periodic.csv");
        assert!(file_path.exists(), "CSV file should exist");

        let mut reader = csv::Reader::from_path(file_path).unwrap();

        assert_eq!(reader.headers().unwrap(), vec!["t", "IsRunner", "count"]);

        let mut actual: Vec<Vec<String>> = reader
            .records()
            .map(|result| result.unwrap().iter().map(String::from).collect())
            .collect();
        let mut expected = vec![
            vec!["0", "false", "2"],
            vec!["1.2", "false", "1"],
            vec!["1.2", "true", "1"],
        ];

        actual.sort();
        expected.sort();

        assert_eq!(actual, expected, "CSV file should contain the correct data");
    }
}
