use std::path::Path;

use super::file::write_profiling_data_to_file;
use crate::{error, Context, ContextReportExt};

/// Trait extension for [`Context`] providing profiling capabilities.
pub trait ProfilingContextExt: ContextReportExt {
    /// Prints the execution statistics for this context to the console.
    ///
    /// If `include_profiling_data` is true, also prints the global profiling data
    /// (spans, counts, and computed statistics).
    fn print_execution_statistics(&mut self, include_profiling_data: bool) {
        let stats = self.get_execution_statistics();
        crate::execution_stats::print_execution_statistics(&stats);

        if include_profiling_data {
            super::print_profiling_data();
        }
    }

    /// Writes the execution statistics for the context and all profiling data
    /// to a JSON file.
    fn write_profiling_data(&mut self) {
        let (mut prefix, directory, overwrite) = {
            let report_options = self.report_options();
            (
                report_options.file_prefix.clone(),
                report_options.output_dir.clone(),
                report_options.overwrite,
            )
        };

        let execution_statistics = self.get_execution_statistics();
        // Default filename when not provided via parameters: write under report options
        // using the current file prefix.
        prefix.push_str("profiling.json");
        let profiling_data_path = directory.join(prefix);
        let profiling_data_path = Path::new(&profiling_data_path);

        if !overwrite && profiling_data_path.exists() {
            error!(
                "profiling output file already exists: {}",
                profiling_data_path.display()
            );
            return;
        }

        write_profiling_data_to_file(profiling_data_path, execution_statistics)
            .expect("could not write profiling data to file");
    }
}
impl ProfilingContextExt for Context {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use tempfile::tempdir;

    use super::ProfilingContextExt;
    use crate::context::Context;
    use crate::profiling::{add_computed_statistic, increment_named_count, open_span};
    use crate::report::ContextReportExt as _; // bring trait with methods like report_options into scope

    #[test]
    fn print_execution_statistics_without_profiling_data() {
        let mut context = Context::new();
        context.print_execution_statistics(false);
    }

    #[test]
    fn print_execution_statistics_with_profiling_data() {
        // Create some profiling activity so data exists
        increment_named_count("reporting_print_event");
        increment_named_count("reporting_print_event");
        {
            let _span = open_span("reporting_print_span");
            std::thread::sleep(Duration::from_millis(5));
        }
        add_computed_statistic::<usize>(
            "reporting_print_stat",
            "Count of reporting_print_event",
            Box::new(|data| data.get_named_count("reporting_print_event")),
            Box::new(|_v| {}),
        );

        let mut context = Context::new();
        context.print_execution_statistics(true);
    }

    #[test]
    fn write_profiling_data_creates_json() {
        let temp_dir = tempdir().unwrap();
        let out_dir = temp_dir.path().to_path_buf();

        // Prepare some profiling data
        increment_named_count("reporting_write_event");
        {
            let _span = open_span("reporting_write_span");
            std::thread::sleep(Duration::from_millis(3));
        }

        let mut context = Context::new();
        let config = context.report_options();
        config
            .file_prefix("test_")
            .directory(out_dir.clone())
            .overwrite(true);

        context.write_profiling_data();

        let file_path = out_dir.join("test_profiling.json");
        assert!(file_path.exists(), "JSON file should be created");

        let content = fs::read_to_string(&file_path).expect("Failed to read JSON");
        let json: serde_json::Value = serde_json::from_str(&content).expect("Invalid JSON");
        assert!(json["execution_statistics"].is_object());
        assert!(json["named_counts"].is_array());
        assert!(json["named_spans"].is_array());
        assert!(json["computed_statistics"].is_object());
    }

    #[test]
    fn write_profiling_data_respects_overwrite_false() {
        let temp_dir = tempdir().unwrap();
        let out_dir = temp_dir.path().to_path_buf();

        // Pre-create the target file with distinct content
        let file_path = out_dir.join("prefix_profiling.json");
        fs::write(&file_path, "PREEXISTING").unwrap();

        let mut context = Context::new();
        let config = context.report_options();
        config
            .file_prefix("prefix_")
            .directory(out_dir.clone())
            .overwrite(false);

        // Attempt to write; should return early and not modify the file
        context.write_profiling_data();

        let after = fs::read_to_string(&file_path).unwrap();
        assert_eq!(
            after, "PREEXISTING",
            "File should remain unchanged when overwrite=false"
        );
    }

    #[test]
    fn write_profiling_data_overwrites_when_true() {
        let temp_dir = tempdir().unwrap();
        let out_dir = temp_dir.path().to_path_buf();

        let file_path = out_dir
            .join("ow_")
            .join("..") // ensure path handling remains simple
            .canonicalize()
            .unwrap_or(out_dir.clone())
            .join("ow_profiling.json");

        // Ensure directory exists and pre-create file
        let _ = fs::create_dir_all(file_path.parent().unwrap());
        fs::write(&file_path, "OLD").unwrap();

        let mut context = Context::new();
        let config = context.report_options();
        config
            .file_prefix("ow_")
            .directory(file_path.parent().unwrap().to_path_buf())
            .overwrite(true);

        context.write_profiling_data();

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(
            content.starts_with("{"),
            "File should contain JSON after overwrite"
        );
        assert_ne!(
            content, "OLD",
            "File content should be updated when overwrite=true"
        );
    }
}
