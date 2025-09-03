use clap::Args;
use ixa::prelude::*;
use ixa::run_with_custom_args;
use population_loader::DiseaseStatus;
use std::path::PathBuf;

mod exposure_manager;
mod incidence_report;
mod infection_manager;
mod parameters_loader;
mod population_loader;

use crate::parameters_loader::Parameters;

#[derive(Args, Debug)]
struct CustomArgs {
    config_file: Option<String>,
}

fn initialize(context: &mut Context, custom_args: Option<CustomArgs>) -> Result<(), IxaError> {
    // If the user does not specify a config file, use the default one.
    let file_path = match custom_args.unwrap().config_file {
        Some(config_file) => PathBuf::from(config_file),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join("time-varying-infection")
            .join("input.json"),
    };

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
    run_with_custom_args(|context, _, custom_args: Option<CustomArgs>| {
        initialize(context, custom_args)
    })
    .unwrap();
}
