use super::*;
use crate::hyperfine_group;

fn build_params() -> Parameters {
    ParametersBuilder::default()
        // when increasing population also increase max_time accordingly otherwise simulation will time out.
        .population(10_000)
        .initial_infections(1000)
        .max_time(10.0)
        .build()
        .unwrap()
}

hyperfine_group!(
    // A simple reference implementation of an SIR model at a largest scale (10,000 population)
    large_sir {
        // Static implementation without Ixa
        baseline => {
            sir_baseline::Model::new(build_params()).run();

        },
        // The equivalent Ixa implementation, with queries enabled
        entities => {
            sir_ixa::Model::new(build_params(), sir_ixa::ModelOptions::default()).run();
        }
    }
);
