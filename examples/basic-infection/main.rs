use ixa::runner::run_with_args;
use ixa_example_basic_infection::initialize;

fn main() {
    run_with_args(|context, _, _| {
        initialize(context);
        Ok(())
    })
    .unwrap();
}
