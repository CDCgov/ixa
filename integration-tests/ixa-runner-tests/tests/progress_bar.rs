#[cfg(test)]
mod tests {

    #[test]
    fn test_run_with_logging_modules() {
        assert_cmd::Command::new("cargo")
            .args(["build", "--bin", "runner_generic"])
            .ok()
            .expect("Failed to build runner_generic");

        let output = assert_cmd::cargo::cargo_bin_cmd!("runner_generic")
            .arg("--timeline-progress-max")
            .arg("300.0")
            .arg("--log-level")
            .arg("ixa=Trace")
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
