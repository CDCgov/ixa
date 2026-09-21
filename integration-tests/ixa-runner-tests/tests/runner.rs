#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    #[test]
    fn test_cli_invocation_with_custom_args() {
        // Note this target is defined in the bin section of Cargo.toml
        // and the entry point is in tests/bin/runner_test_custom_args
        assert_cmd::cargo::cargo_bin_cmd!("runner_test_custom_args")
            .arg("-a")
            .arg("42")
            .arg("--no-stats")
            .assert()
            .success()
            .stdout(
                "Current log levels enabled: ERROR
Run runner_test_custom_args --help -v to see more options
42\n",
            );
    }

    #[test]
    fn test_cli_config_accepts_hyphenated_package_name() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");
        fs::write(
            &config_path,
            r#"{
  "ixa-runner-tests.RunnerProperty": {
    "field_int": 7
  }
}
"#,
        )
        .unwrap();

        assert_cmd::cargo::cargo_bin_cmd!("ixa-runner-tests")
            .arg("--config")
            .arg(config_path)
            .arg("--no-stats")
            .assert()
            .success()
            .stdout(format!(
                "Loading global properties from: {:?}\nCurrent log levels enabled: ERROR\nRun ixa-runner-tests --help -v to see more options\n7\n",
                temp_dir.path().join("config.json")
            ));
    }

    #[test]
    fn test_merged_args_help_exits_zero() {
        let output = assert_cmd::cargo::cargo_bin_cmd!("runner_merged_args")
            .arg("--help")
            .output()
            .unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("Usage:"));
        assert!(stdout.contains("--random-seed"));
    }

    #[test]
    fn test_merged_args_config_with_cli_override() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");
        fs::write(
            &config_path,
            r#"{
  "args": {
    "random_seed": 42,
    "custom": { "a": 7 }
  }
}
"#,
        )
        .unwrap();

        assert_cmd::cargo::cargo_bin_cmd!("runner_merged_args")
            .arg("--config")
            .arg(&config_path)
            .arg("--no-stats")
            .assert()
            .success()
            .stdout(format!(
                "Loading global properties from: {:?}\nCurrent log levels enabled: ERROR\nRun runner_merged_args --help -v to see more options\nseed=42 a=7\n",
                config_path
            ));

        assert_cmd::cargo::cargo_bin_cmd!("runner_merged_args")
            .arg("--config")
            .arg(&config_path)
            .arg("--no-stats")
            .arg("-a")
            .arg("9")
            .assert()
            .success()
            .stdout(format!(
                "Loading global properties from: {:?}\nCurrent log levels enabled: ERROR\nRun runner_merged_args --help -v to see more options\nseed=42 a=9\n",
                config_path
            ));
    }

    #[test]
    fn execution_statistics_respect_no_stats_with_verbose_logging() {
        let default_output = assert_cmd::cargo::cargo_bin_cmd!("runner_generic")
            .output()
            .unwrap();
        let default_stdout = String::from_utf8(default_output.stdout).unwrap();
        assert!(default_stdout.contains("━━━━ Execution Summary ━━━━"));
        assert!(default_stdout.contains("Wall time:"));

        let no_stats_output = assert_cmd::cargo::cargo_bin_cmd!("runner_generic")
            .arg("--no-stats")
            .output()
            .unwrap();
        let no_stats_stdout = String::from_utf8(no_stats_output.stdout).unwrap();
        assert!(!no_stats_stdout.contains("Execution Summary"));
        assert!(!no_stats_stdout.contains("Wall time:"));

        let verbose_output = assert_cmd::cargo::cargo_bin_cmd!("runner_generic")
            .args(["--no-stats", "-v"])
            .output()
            .unwrap();
        let verbose_stdout = String::from_utf8(verbose_output.stdout).unwrap();
        assert!(verbose_stdout.contains("An INFO message"));
        assert!(!verbose_stdout.contains("Execution Summary"));
        assert!(!verbose_stdout.contains("Wall time:"));
    }

    #[test]
    fn explicit_execution_statistics_logging_remains_filtered() {
        let filtered_output = assert_cmd::cargo::cargo_bin_cmd!("runner_log_statistics")
            .arg("--no-stats")
            .output()
            .unwrap();
        let filtered_stdout = String::from_utf8(filtered_output.stdout).unwrap();
        assert!(!filtered_stdout.contains("Execution complete."));
        assert!(!filtered_stdout.contains("Wall time: 1s"));

        let info_output = assert_cmd::cargo::cargo_bin_cmd!("runner_log_statistics")
            .args(["--no-stats", "-v"])
            .output()
            .unwrap();
        let info_stdout = String::from_utf8(info_output.stdout).unwrap();
        assert!(info_stdout.contains("Execution complete."));
        assert!(info_stdout.contains("Wall time: 1s"));
        assert!(!info_stdout.contains("Execution Summary"));
    }

    #[test]
    fn test_run_with_logging_modules() {
        assert_cmd::Command::new("cargo")
            .args(["build", "--bin", "runner_generic"])
            .ok()
            .expect("Failed to build runner_generic");

        let output = assert_cmd::cargo::cargo_bin_cmd!("runner_generic")
            .arg("--log-level")
            .arg("ixa=Trace")
            .output();
        match String::from_utf8(output.unwrap().stdout) {
            Ok(s) => {
                // Check if the output contains some of the expected log messages
                assert!(s.contains("Logging enabled for ixa at level TRACE"));
                assert!(s.contains("TRACE ixa::context - entering event loop"));
            }
            Err(e) => {
                println!("Failed to convert: {e}");
                panic!();
            }
        }
    }
}
