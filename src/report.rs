use crate::context::Context;
use crate::people::PeoplePlugin;
use csv::Writer;
use serde::Deserialize;
use serde::Serialize;
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs;
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
    /// # Panics
    /// if the directory in which the report is to be stored does not exist and cannot be created
    pub fn directory(&mut self, directory: PathBuf) -> &mut ConfigReportOptions {
        // if the directory does not exist, create it
        fs::create_dir_all(directory.clone()).expect("Failed to create directory");
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

#[allow(clippy::module_name_repetitions)]
pub struct ReportData {
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

#[derive(Serialize, Deserialize)]
struct PeriodicReportItem {
    time: f64,
    property_type: String,
    property_value: String,
    count: usize,
}

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
    fn count_person_properties_and_report(&mut self, report_period: f64) {
        // only want to schedule the person properties report
        // to go through people properties again
        // if there are plans out on the queue/there is still work
        // for the simulation to do
        // note that this may run an extra report at the end
        // while plans with no data at the end of the queue are removed
        // the plan_queue will not yet register as empty
        // this function only checks for plans because (a) callbacks
        // are always run first and (b) we hope to remove callbacks in
        // the future anyways

        // however, does this plan need to be scheduled so that it is the _last_ plan
        // occuring at the decided upon time?
        // that way it is unequivically reporting out on the state of the world
        // at the _end_ of the reported time?
        if self.more_plans() {
            self.add_plan(self.get_current_time() + report_period, move |context| {
                context.count_person_properties_and_report(report_period);
            });
        }
        let people_data = self.get_data_container(PeoplePlugin)
        .expect("PeoplePlugin is not initialized; make sure you add a person before reporting on their properties.");
        let include_in_report = &people_data.include_in_periodic_report;
        // iterate through the various properties that are in the report
        // and call their tabulate method to get the counts of each property value
        for property in include_in_report.values() {
            // use a trait function to get the tabulation of values for the property
            // in essence, this function grabs the property vector for this particular property
            // and turns that into a vector of property values of the right type and then tabulates
            let property_values_tabulated =
                property.get_tabulation(people_data.properties_map.borrow());
            // iterate through the tabulated values and send a report item for each
            for property_value in property_values_tabulated.keys() {
                // send a generic report item for each property value
                self.send_report(PeriodicReportItem {
                    time: self.get_current_time(),
                    property_type: property.to_string(),
                    property_value: property_value.to_string(),
                    count: *property_values_tabulated.get(property_value).unwrap(),
                });
            }
        }
    }
}

pub trait ContextReportExt {
    fn add_report<T: Report + 'static>(&mut self, short_name: &str);
    fn send_report<T: Report>(&self, report: T);
    fn report_options(&mut self) -> &mut ConfigReportOptions;
    fn add_person_properties_report(&mut self, short_name: &str, report_period: f64);
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

    /// Adds a report that counts the number of people with each person property value
    fn add_person_properties_report(&mut self, short_name: &str, report_period: f64) {
        create_report_trait!(PeriodicReportItem);
        self.add_report::<PeriodicReportItem>(short_name);
        // need to think a little bit about the flow here
        // we could just call this function (no plan) as the last thing in the init sequence,
        // but then it would be reporting on the simulation at the end of init
        // and the beginning of simulation time
        // need to think a bit about flow here
        // i think adding it as a plan at time 0 guarantees that it reports out on
        // the initial state of the simulation in a way the user can reason about more
        // based on whether they call the add report function at the beginning or end
        // of init
        // presumably, the user knows the state of the world that they set up the init with,
        // but it's not obvious that the user knows the state of the world at the end of init
        self.add_plan(0.0, move |context| {
            context.count_person_properties_and_report(report_period);
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        define_person_property, define_person_property_with_default, people::ContextPeopleExt,
    };
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
    #[test]
    fn person_property_report_adds_plan() {
        let mut context = Context::new();
        let temp_dir = tempdir().unwrap();
        let path = PathBuf::from(&temp_dir.path());
        let config = context.report_options();
        config
            .file_prefix("person_property_report_adds_plan".to_string())
            .directory(path.clone());
        assert!(!context.more_plans());
        context.add_person_properties_report("", 1.0);
        assert!(context.more_plans());
        // check that we have implemented trait report for PeriodicReportItem
        let periodic_report_item = PeriodicReportItem {
            time: 0.0,
            property_type: "test".to_string(),
            property_value: "test".to_string(),
            count: 0,
        };
        assert_eq!(
            periodic_report_item.type_id(),
            TypeId::of::<PeriodicReportItem>()
        );
    }

    #[test]
    #[should_panic(
        expected = "PeoplePlugin is not initialized; make sure you add a person before reporting on their properties."
    )]
    fn person_properties_report_with_no_people() {
        let mut context = Context::new();
        let temp_dir = tempdir().unwrap();
        let path = PathBuf::from(&temp_dir.path());
        let config = context.report_options();
        config
            .file_prefix("person_properties_report_with_no_people".to_string())
            .directory(path.clone());
        assert!(!context.more_plans());
        context.add_person_properties_report("", 1.0);
        assert!(context.more_plans());
        context.execute();
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn person_property_report_self_schedules() {
        // checks whether the person properties report schedules itself
        // based on whether there are plans in the queue
        let mut context = Context::new();
        let temp_dir = tempdir().unwrap();
        let path = PathBuf::from(&temp_dir.path());
        let config = context.report_options();
        config
            .file_prefix("person_property_report_self_schedules".to_string())
            .directory(path.clone());
        define_person_property!(TestProperty, bool, true);
        context.add_person_properties_report("", 1.0);
        assert!(context.more_plans());
        // Add a person to the context
        let person = context.add_person();
        context.initialize_person_property(person, TestProperty, false);
        context.add_plan(1.0, move |context| {
            context.set_person_property(person, TestProperty, true);
        });
        context.add_plan(2.0, move |context| {
            context.set_person_property(person, TestProperty, true);
        });
        context.execute();
        assert_eq!(context.get_current_time(), 2.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn selected_properties_included() {
        // test that only the properties that are set to be included
        // in the report are included
        let mut context = Context::new();
        let temp_dir = tempdir().unwrap();
        let path = PathBuf::from(&temp_dir.path());
        let config = context.report_options();
        config
            .file_prefix("selected_properties_included".to_string())
            .directory(path.clone());
        context.add_person_properties_report("", 1.0);
        // Add a person with a property that should be included in the report
        define_person_property_with_default!(IncludedProperty, u8, true, 0);
        define_person_property_with_default!(ExcludedProperty, u8, false, 0);
        let person = context.add_person();
        context.set_person_property(person, IncludedProperty, 42);
        context.set_person_property(person, ExcludedProperty, 24);

        // Execute the context to generate the report
        context.execute();

        // Check that the report file exists and contains the expected data
        let file_path = path.join("selected_properties_included.csv");
        assert!(file_path.exists(), "CSV file should exist");

        let mut reader = csv::Reader::from_path(file_path).unwrap();
        let mut records = reader.deserialize::<PeriodicReportItem>();

        let item: PeriodicReportItem = records.next().expect("No record found").unwrap();
        assert_eq!(item.time, 0.0);
        assert_eq!(item.property_type, "IncludedProperty");
        assert_eq!(item.property_value, "42");
        assert_eq!(item.count, 1);
        assert!(records.next().is_none());
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn properties_only_included_after_init() {
        // up for debate whether this is actually good behavior or not
        let mut context = Context::new();
        let temp_dir = tempdir().unwrap();
        let path = PathBuf::from(&temp_dir.path());
        let config = context.report_options();
        config
            .file_prefix("properties_only_included_after_init".to_string())
            .directory(path.clone());
        context.add_person_properties_report("", 1.0);
        // Add a person with a property that should be included in the report
        define_person_property_with_default!(PropertyWithDefault, u8, true, 0);
        define_person_property!(PropertyInited, u8, true);
        let person = context.add_person();
        context.add_plan(1.0, move |context| {
            context.set_person_property(person, PropertyWithDefault, 42);
        });
        context.add_plan(2.0, move |context| {
            context.initialize_person_property(person, PropertyInited, 24);
        });
        // Execute the context to generate the report
        context.execute();
        assert_eq!(context.get_current_time(), 2.0);

        // Check that the report file exists and contains the expected data
        let file_path = path.join("properties_only_included_after_init.csv");
        assert!(file_path.exists(), "CSV file should exist");

        let mut reader = csv::Reader::from_path(file_path).unwrap();
        let mut records = reader.deserialize::<PeriodicReportItem>();

        let item: PeriodicReportItem = records.next().expect("No record found").unwrap();
        assert_eq!(item.time, 1.0);
        assert_eq!(item.property_type, "PropertyWithDefault");
        assert_eq!(item.property_value, "42");
        assert_eq!(item.count, 1);

        let item: PeriodicReportItem = records.next().expect("No record found").unwrap();
        assert_eq!(item.time, 2.0);
        // don't know which item in which order because .values() visits in arbitrary order
        assert_eq!(item.count, 1);

        // but there should be two properties reported on at time 2.0
        let item: PeriodicReportItem = records.next().expect("No record found").unwrap();
        assert_eq!(item.time, 2.0);
        assert_eq!(item.count, 1);

        // should be no more records left
        assert!(records.next().is_none());
    }
}
