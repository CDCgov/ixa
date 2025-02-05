use std::path::PathBuf;

use ixa::error::IxaError;
use ixa::random::ContextRandomExt;
use ixa::report::ContextReportExt;
use ixa::run_with_args;
use ixa::{context::Context, global_properties::ContextGlobalPropertiesExt};
use population_loader::DiseaseStatus;

mod exposure_manager;
mod incidence_report;
mod infection_manager;
mod parameters_loader;
mod population_loader;

use crate::parameters_loader::Parameters;

fn initialize(context: &mut Context) -> Result<(), IxaError> {
    let args: Vec<String> = std::env::args().collect();
    let file_path = PathBuf::from(&args[1]);

    parameters_loader::init_parameters(context, &file_path)?;
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    context.init_random(parameters.seed);

    exposure_manager::init(context);
    population_loader::init(context);
    infection_manager::init(context);
    incidence_report::init(context)?;
    // add periodic report
    context.add_periodic_report("person_property_count", 1.0, (DiseaseStatus,))?;

    context.add_plan(parameters.max_time, |context| {
        context.shutdown();
    });
    println!("{parameters:?}");
    Ok(())
}

fn main() {
    run_with_args(|context, _, _| initialize(context)).unwrap();
}
