use ixa::runner::run_with_args;
use ixa_example_births_deaths::initialize;
use std::path::PathBuf;

fn main() {
    let output_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("births-deaths")
        .join("output");

    run_with_args(|context, _, _| {
        initialize(context, &*output_path);
        Ok(())
    })
    .unwrap();
}
