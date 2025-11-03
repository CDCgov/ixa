#[cfg(test)]
mod tests {

    #[test]
    fn test_run_with_logging_modules() {
        assert_cmd::Command::new("cargo")
            .args(["build", "--bin", "runner_test_debug"])
            .ok()
            .expect("Failed to build runner_test_debug");

        let output = assert_cmd::cargo::cargo_bin_cmd!("runner_test_debug")
            .arg("--debugger")
            .arg("1.0")
            .arg("--log-level")
            .arg("rustyline=Debug,ixa=Trace")
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

    #[test]
    fn test_verbosity_levels() {
        assert_cmd::Command::new("cargo")
            .args(["build", "--bin", "runner_test_debug"])
            .ok()
            .expect("Failed to build runner_test_debug");

        // `-v`
        let output = assert_cmd::cargo::cargo_bin_cmd!("runner_test_debug")
            .arg("-v")
            .output();
        match String::from_utf8(output.unwrap().stdout) {
            Ok(s) => {
                // Check if the output contains some of the expected log messages
                assert!(!s.contains("A TRACE message"));
                assert!(!s.contains("A DEBUG message"));
                assert!(s.contains("An INFO message"));
            }
            Err(e) => {
                println!("Failed to convert: {e}");
                panic!();
            }
        }

        // `-vv`
        let output = assert_cmd::cargo::cargo_bin_cmd!("runner_test_debug")
            .arg("-vv")
            .output();
        match String::from_utf8(output.unwrap().stdout) {
            Ok(s) => {
                // Check if the output contains some of the expected log messages
                assert!(!s.contains("A TRACE message"));
                assert!(s.contains("A DEBUG message"));
                assert!(s.contains("An INFO message"));
            }
            Err(e) => {
                println!("Failed to convert: {e}");
                panic!();
            }
        }

        // `-vvv`
        let output = assert_cmd::cargo::cargo_bin_cmd!("runner_test_debug")
            .arg("-vvv")
            .output();
        match String::from_utf8(output.unwrap().stdout) {
            Ok(s) => {
                // Check if the output contains some of the expected log messages
                assert!(s.contains("A TRACE message"));
                assert!(s.contains("A DEBUG message"));
                assert!(s.contains("An INFO message"));
            }
            Err(e) => {
                println!("Failed to convert: {e}");
                panic!();
            }
        }
    }
}
