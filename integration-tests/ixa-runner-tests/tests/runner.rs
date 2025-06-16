#[cfg(test)]
mod tests {
    use assert_cmd::cargo::CargoError;

    pub fn run_external_runner(runner_name: &str) -> Result<assert_cmd::Command, CargoError> {
        assert_cmd::Command::cargo_bin(runner_name)
    }

    #[test]
    fn test_cli_invocation_with_custom_args() {
        // Note this target is defined in the bin section of Cargo.toml
        // and the entry point is in tests/bin/runner_test_custom_args
        run_external_runner("runner_test_custom_args")
            .unwrap()
            .args(["-a", "42"])
            .assert()
            .success()
            .stdout("42\n");
    }

    #[test]
    fn test_run_with_logging_modules() {
        assert_cmd::Command::new("cargo")
            .args(["build", "--bin", "runner_test_debug"])
            .ok()
            .expect("Failed to build runner_test_debug");

        let output = assert_cmd::Command::cargo_bin("runner_test_debug")
            .unwrap()
            .args([
                "--debugger",
                "1.0",
                "--log-level",
                "rustyline=Debug,ixa=Trace",
            ])
            .write_stdin("population\n")
            .output();
        match String::from_utf8(output.unwrap().stdout) {
            Ok(s) => {
                // Check if the output contains some of the expected log messages
                assert!(s.contains("Logging enabled for rustyline at level DEBUG"));
                assert!(s.contains("Logging enabled for ixa at level TRACE"));
                assert!(s.contains("TRACE ixa::plan - adding plan at 1"));
            }
            Err(e) => {
                println!("Failed to convert: {e}");
                panic!();
            }
        }
    }
}
