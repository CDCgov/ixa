use std::path::PathBuf;

use ixa::Context;
use ixa_example_births_deaths::try_initialize;

#[test]
fn missing_parameter_file_exits_nonzero() {
    let missing_parameters =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("missing-parameters-for-exit-test.json");
    assert!(!missing_parameters.exists());

    let assertion = assert_cmd::cargo::cargo_bin_cmd!("births_deaths")
        .arg("--parameters")
        .arg(&missing_parameters)
        .arg("--no-stats")
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&assertion.get_output().stderr);
    assert!(
        stderr.contains("missing-parameters-for-exit-test.json"),
        "unexpected stderr: {stderr}",
    );
}

#[test]
fn report_initialization_failure_is_returned() {
    let temp_dir = tempfile::tempdir().expect("temporary directory should be created");
    let missing_output_dir = temp_dir.path().join("missing-output-directory");
    let mut context = Context::new();

    let error = try_initialize(&mut context, &missing_output_dir)
        .expect_err("a missing report directory should fail initialization");

    assert!(matches!(error, ixa::IxaError::IoError(_)));
}
