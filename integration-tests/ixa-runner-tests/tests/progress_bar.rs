#[cfg(test)]
mod tests {

    #[test]
    fn test_run_with_logging_modules() {
        assert_cmd::Command::new("cargo")
            .args(["build", "--bin", "runner_test_debug"])
            .ok()
            .expect("Failed to build runner_test_debug");

        let output = assert_cmd::Command::cargo_bin("runner_test_debug")
            .unwrap()
            .args([
                "--timeline-progress-max",
                "300.0",
                "--log-level",
                "ixa=Trace",
            ])
            .write_stdin("population\n")
            .output();
        match String::from_utf8(output.unwrap().stdout) {
            Ok(s) => {
                // Check if the output contains some of the expected log messages
                assert!(s.contains("initializing timeline progress bar with max time 300"));
            }
            Err(e) => {
                println!("Failed to convert: {e}");
                panic!();
            }
        }
    }
}
