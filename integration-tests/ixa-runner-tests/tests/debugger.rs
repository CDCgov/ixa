#[cfg(test)]
mod tests {
    use std::sync::{LazyLock, Mutex};

    static TEST_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(Mutex::default);

    #[test]
    fn test_cli_debugger_integration() {
        assert_cmd::cargo::cargo_bin_cmd!("runner_test_debug")
            .arg("--debugger")
            .arg("1.0")
            .write_stdin("population\n")
            .write_stdin("continue\n")
            .assert()
            .success();
    }

    #[test]
    fn command_line_args_sets_level() {
        let _guard = TEST_MUTEX.lock().expect("Mutex poisoned");
        assert_cmd::cargo::cargo_bin_cmd!("runner_test_debug")
            .arg("--log-level=trace")
            .assert()
            .success();
    }
}
