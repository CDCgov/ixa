#[cfg(test)]
mod tests {
    use std::sync::{LazyLock, Mutex};

    use assert_cmd::cargo::CargoError;

    static TEST_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(Mutex::default);

    pub fn run_external_runner(runner_name: &str) -> Result<assert_cmd::Command, CargoError> {
        assert_cmd::Command::cargo_bin(runner_name)
    }

    #[test]
    fn test_cli_debugger_integration() {
        run_external_runner("runner_test_debug")
            .unwrap()
            .args(["--debugger", "1.0"])
            .write_stdin("population\n")
            .write_stdin("continue\n")
            .assert()
            .success();
    }

    #[test]
    fn command_line_args_sets_level() {
        let _guard = TEST_MUTEX.lock().expect("Mutex poisoned");
        run_external_runner("runner_test_debug")
            .unwrap()
            .args(["--log-level=trace"])
            .assert()
            .success();
    }
}
