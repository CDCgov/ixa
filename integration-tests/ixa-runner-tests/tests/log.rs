#[cfg(test)]
mod tests {

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

    #[test]
    fn test_verbosity_levels() {
        assert_cmd::Command::new("cargo")
            .args(["build", "--bin", "runner_generic"])
            .ok()
            .expect("Failed to build runner_generic");

        // `-v`
        let output = assert_cmd::cargo::cargo_bin_cmd!("runner_generic")
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
        let output = assert_cmd::cargo::cargo_bin_cmd!("runner_generic")
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
        let output = assert_cmd::cargo::cargo_bin_cmd!("runner_generic")
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
