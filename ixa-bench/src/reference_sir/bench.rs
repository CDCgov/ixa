use super::*;
use crate::hyperfine_group;

fn build_params() -> Parameters {
    ParametersBuilder::default()
        // when increasing population also increase max_time accordingly.
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
        ixa => {
            sir_ixa::Model::new(build_params(), sir_ixa::ModelOptions::default()).run();
        },
        // The equivalent Ixa implementation, with queries disabled
        ixa_no_queries => {
            sir_ixa::Model::new(build_params(), sir_ixa::ModelOptions {
                queries_enabled: false,
            }).run();
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
