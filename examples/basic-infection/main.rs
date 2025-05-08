use ixa::prelude::*;
use ixa_example_basic_infection::initialize;

fn main() {
    run_with_args(|context, _, _| {
        initialize(context);
        Ok(())
    })
    .unwrap();
}
