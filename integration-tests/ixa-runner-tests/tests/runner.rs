#[cfg(test)]
mod tests {

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
