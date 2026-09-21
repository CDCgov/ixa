#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn test_profiling_json_written() {
        let temp_dir = tempfile::tempdir().expect("failed to create tempdir");
        let output_dir = temp_dir.path();

        assert_cmd::cargo::cargo_bin_cmd!("runner_test_profiling")
            .arg("--output")
            .arg(output_dir)
            .arg("--prefix")
            .arg("it_")
            .arg("--force-overwrite")
            .arg("--no-stats")
            .assert()
            .success();

        let profiling_path = output_dir.join("it_profiling.json");
        assert!(
            profiling_path.exists(),
            "expected profiling output file to exist at {}",
            profiling_path.display()
        );

        let content = fs::read_to_string(&profiling_path).expect("failed to read profiling output");
        let json: serde_json::Value =
            serde_json::from_str(&content).expect("profiling output was not valid JSON");

        assert!(json["execution_statistics"].is_object());
        assert!(json["named_counts"].is_array());
        assert!(json["named_spans"].is_array());
        assert!(json["query_timings"].is_array());
        assert!(json["computed_statistics"].is_object());

        let counts = json["named_counts"].as_array().unwrap();
        let it_prof_event = counts
            .iter()
            .find(|c| c["label"] == "it_prof_event")
            .expect("it_prof_event not found in named_counts");
        assert_eq!(it_prof_event["count"], 3);

        let spans = json["named_spans"].as_array().unwrap();
        let it_prof_span = spans
            .iter()
            .find(|s| s["label"] == "it_prof_span")
            .expect("it_prof_span not found in named_spans");
        assert!(
            it_prof_span["count"].as_u64().unwrap_or(0) >= 1,
            "expected it_prof_span to be recorded at least once"
        );

        let query_timings = json["query_timings"].as_array().unwrap();
        let profiling_age_query = query_timings
            .iter()
            .find(|q| q["query"] == "ProfilingPerson: (ProfilingAge)")
            .expect("ProfilingPerson age query not found in query_timings");
        assert_eq!(profiling_age_query["count"], 1);
        assert!(profiling_age_query["total"].as_f64().is_some());
        assert!(profiling_age_query["min"].as_f64().is_some());
        assert!(profiling_age_query["max"].as_f64().is_some());

        let profiling_age_county_query = query_timings
            .iter()
            .find(|q| q["query"] == "ProfilingPerson: (ProfilingAge, ProfilingCounty)")
            .expect("ProfilingPerson age/county query not found in query_timings");
        assert_eq!(profiling_age_county_query["count"], 3);
        assert!(profiling_age_county_query["total"].as_f64().is_some());
        assert!(profiling_age_county_query["min"].as_f64().is_some());
        assert!(profiling_age_county_query["max"].as_f64().is_some());

        let computed = &json["computed_statistics"];
        assert_eq!(computed["it_prof_stat"]["description"], "Total test events");
        assert_eq!(computed["it_prof_stat"]["value"], 3);
    }

    #[test]
    fn runner_statistics_respect_no_stats_with_verbose_logging() {
        let temp_dir = tempfile::tempdir().expect("failed to create tempdir");
        let output_dir = temp_dir.path();

        let default_output = assert_cmd::cargo::cargo_bin_cmd!("runner_test_profiling")
            .args([
                "--output",
                output_dir.to_str().unwrap(),
                "--force-overwrite",
            ])
            .output()
            .unwrap();
        let default_stdout = String::from_utf8(default_output.stdout).unwrap();
        assert!(default_stdout.contains("Execution Summary"));
        assert!(default_stdout.contains("Span Label"));
        assert!(default_stdout.contains("Event Label"));
        assert!(default_stdout.contains("Query"));

        let no_stats_output = assert_cmd::cargo::cargo_bin_cmd!("runner_test_profiling")
            .args([
                "--output",
                output_dir.to_str().unwrap(),
                "--force-overwrite",
                "--no-stats",
                "-v",
            ])
            .output()
            .unwrap();
        let no_stats_stdout = String::from_utf8(no_stats_output.stdout).unwrap();
        assert!(!no_stats_stdout.contains("Execution Summary"));
        assert!(!no_stats_stdout.contains("Span Label"));
        assert!(!no_stats_stdout.contains("Event Label"));
        assert!(!no_stats_stdout.contains("Query"));
    }
}
