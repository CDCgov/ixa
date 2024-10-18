use std::path::Path;

use ixa::random::ContextRandomExt;
use ixa::{context::Context, global_properties::ContextGlobalPropertiesExt};

mod exposure_manager;
mod incidence_report;
mod infection_manager;
mod parameters_loader;
mod population_loader;

use crate::parameters_loader::Parameters;

fn main() {
    let mut context = Context::new();

    let current_dir = Path::new(file!()).parent().unwrap();
    let file_path = current_dir.join("input.json");

    match parameters_loader::init_parameters(&mut context, &file_path) {
        Ok(()) => {
            let parameters = context.get_global_property_value(Parameters).clone();
            context.init_random(parameters.seed);

            exposure_manager::init(&mut context);
            population_loader::init(&mut context);
            infection_manager::init(&mut context);
            incidence_report::init(&mut context);

            context.add_plan(parameters.max_time, |context| {
                context.shutdown();
            });
            println!("{parameters:?}");
            context.execute();
        }
        Err(ixa_error) => {
            println!("Could not read parameters: {ixa_error}");
        }
    }

    context.execute();
}
