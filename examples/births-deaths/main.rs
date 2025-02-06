use ixa::runner::run_with_args;
use ixa_example_births_deaths::initialize;

fn main() {
    run_with_args(|context, _, _| {
        initialize(context);
        Ok(())
    })
    .unwrap();
}
