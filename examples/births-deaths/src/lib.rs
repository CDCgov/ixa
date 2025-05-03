use ixa::random::ContextRandomExt;
use ixa::{context::Context, global_properties::ContextGlobalPropertiesExt};
use std::path::Path;
use std::path::PathBuf;

pub mod demographics_report;
pub mod incidence_report;
pub mod infection_manager;
pub mod parameters_loader;
pub mod population_manager;
pub mod transmission_manager;

use crate::parameters_loader::Parameters;

pub fn initialize(context: &mut Context, given_parameters_path: &Path, output_path: &Path) {
    let current_dir = Path::new(file!()).parent().unwrap();
    let output_path_buff = PathBuf::from(&output_path);

    let def_parameters_path = current_dir.join("../input.json");
    let parameters_path = if given_parameters_path.exists() {
        given_parameters_path
    } else {
        def_parameters_path.as_path()
    };

    parameters_loader::init_parameters(context, parameters_path).unwrap_or_else(|e| {
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
