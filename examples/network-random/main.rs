use ixa::prelude::*;
use ixa::runner::run_with_args;
use ixa_example_network_random::init;

define_entity!(Person);
define_rng!(MainRng);

fn main() {
    run_with_args(|context, _, _| {
        init(context);
        Ok(())
    })
    .unwrap();
}
