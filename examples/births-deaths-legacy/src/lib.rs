use std::path::{Path, PathBuf};

use ixa::prelude::*;

pub mod demographics_report;
pub mod incidence_report;
pub mod infection_manager;
pub mod parameters_loader;
pub mod population_manager;
pub mod transmission_manager;

use crate::parameters_loader::Parameters;

pub fn initialize(context: &mut Context, output_path: &Path) {
    let output_path_buff = PathBuf::from(&output_path);

    let parameters_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("input.json");
    parameters_loader::init_parameters(context, &parameters_path).unwrap_or_else(|e| {
        eprintln!("failed to init init_parameters: {}", e);
    });

    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    context.init_random(parameters.seed);

    demographics_report::init(context, &output_path_buff).unwrap_or_else(|e| {
        eprintln!("failed to init demographics_report: {}", e);
    });

    incidence_report::init(context, &output_path_buff).unwrap_or_else(|e| {
        eprintln!("failed to init incidence_report: {}", e);
    });
    population_manager::init(context);
    transmission_manager::init(context);
    infection_manager::init(context);

    context.add_plan(parameters.max_time, |context| {
        context.shutdown();
    });
}
