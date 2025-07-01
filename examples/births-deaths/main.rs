use ixa::runner::run_with_args;
use ixa_example_births_deaths::initialize;
use std::path::Path;

fn main() {
    let source_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    run_with_args(|context, _, _| {
        initialize(context, source_dir);
        Ok(())
    })
    .unwrap();
}
