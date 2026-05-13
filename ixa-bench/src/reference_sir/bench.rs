use super::*;
use crate::hyperfine_group;

fn build_params(itinerary: Itinerary) -> Parameters {
    ParametersBuilder::default()
        // when increasing population also increase max_time accordingly otherwise simulation will time out.
        .population(10_000)
        .initial_infections(1000)
        .max_time(10.0)
        .itinerary(itinerary)
        .build()
        .unwrap()
}

const HOMOGENEOUS: Itinerary = Itinerary {
    household: 0.0,
    community: 1.0,
};
const HALF_HOUSEHOLD: Itinerary = Itinerary {
    household: 0.5,
    community: 0.5,
};

hyperfine_group!(
    // A simple reference implementation of an SIR model at a largest scale (10,000 population)
    large_sir {
        // Static implementation without Ixa, homogeneous mixing
        baseline => {
            sir_baseline::Model::new(build_params(HOMOGENEOUS)).run();
        },
        // Static implementation without Ixa, 50% of contacts via household Vec lookup
        baseline_households => {
            sir_baseline::Model::new(build_params(HALF_HOUSEHOLD)).run();
        },
        // The equivalent Ixa implementation, homogeneous mixing (no household structure)
        entities => {
            sir_ixa::Model::new(build_params(HOMOGENEOUS), sir_ixa::ModelOptions::default()).run();
        },
        // Same Ixa implementation, with 50% of contacts drawn from the
        // infectious person's household via an IndexableMap lookup.
        households => {
            sir_ixa::Model::new(build_params(HALF_HOUSEHOLD), sir_ixa::ModelOptions::default()).run();
        }
    }
);