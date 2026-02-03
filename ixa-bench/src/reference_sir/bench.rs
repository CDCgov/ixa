use super::*;
use crate::hyperfine_group;

fn build_params() -> Parameters {
    ParametersBuilder::default()
        // when increasing population also increase max_time accordingly otherwise simulation will time out.
        .population(100_000)
        .initial_infections(1000)
        .max_time(100.0)
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

        // Static implementation without Ixa but with more Ixa-like implementation
        baseline_alt => {
            sir_baseline_alt::Model::new(build_params()).run();

        },

        // The equivalent Ixa implementation, with queries enabled
        // legacy => {
        //     sir_ixa::legacy::Model::new(build_params(), sir_ixa::ModelOptions::default()).run();
        // },

        entities => {
            sir_ixa::entities_sir::Model::new(build_params(), sir_ixa::ModelOptions::default()).run();
        }
    }
);

hyperfine_group!(
    // Benchmarks for periodic counting/reporting functionality
    periodic_counts_bench {
        // Baseline: run model without periodic reporting
        no_periodic_reports => {
            use super::periodic_counts;
            periodic_counts::Model::new(build_params(), periodic_counts::ModelOptions {
                periodic_reporting: false,
            }).run();
        },
        // With periodic reporting of infection counts
        with_periodic_reports => {
            use super::periodic_counts;
            periodic_counts::Model::new(build_params(), periodic_counts::ModelOptions {
                periodic_reporting: true,
            }).run();
        }
    }
);

fn population() -> usize {
    100_000
}

hyperfine_group!(
    property_setting {
        baseline => {
            sir_ixa::properties::baseline_model_set_random(population());
        },

        ixa => {
            sir_ixa::properties::ixa_model_set_random(population());
        }
    }
);
