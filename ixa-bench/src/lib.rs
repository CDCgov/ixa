pub mod bench_utils;
pub mod generate_population;
pub mod reference_sir;

#[cfg(test)]
#[allow(dead_code)] // Benchmark entry points are unused by the test harness.
#[path = "../criterion/sample_single_excluding.rs"]
mod sample_single_excluding;
