use ixa::runner::run_with_args;
use ixa_example_births_deaths::initialize;
use std::path::Path;

fn main() {
    let current_dir = Path::new(file!()).parent().unwrap();
    run_with_args(|context, _, _| {
        initialize(context, current_dir, current_dir);
        Ok(())
    })
    .unwrap();
}
