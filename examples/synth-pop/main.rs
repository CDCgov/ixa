use ixa::random::ContextRandomExt;
use ixa::{context::Context, global_properties::ContextGlobalPropertiesExt};
use std::path::Path;


mod parameters_loader;
mod population_manager;
mod population_report;

use crate::parameters_loader::Parameters;

fn main() {
    let mut context = Context::new();
    let current_dir = Path::new(file!()).parent().unwrap();
    let file_path = current_dir.join("input.json");

    match parameters_loader::init_parameters(&mut context, &file_path) {
        Ok(()) => {
            let parameters = context.get_global_property_value(Parameters)
                .unwrap()
                .clone();
            context.init_random(parameters.seed);

            population_report::init(&mut context);
            population_manager::init(&mut context);

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
}
