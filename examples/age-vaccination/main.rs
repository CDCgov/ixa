use ixa::random::ContextRandomExt;
use ixa::{
    context::Context, define_person_property, define_person_property_with_default,
    global_properties::ContextGlobalPropertiesExt,
};
use std::path::Path;
use crate::parameters_loader::Parameters;
use serde::{Deserialize, Serialize};

mod parameters_loader;
mod population_manager;
mod incidence_report;
mod people_report;


#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum InfectionStatus {
    S,
    I,
    R,
}
define_person_property_with_default!(InfectionStatusType, InfectionStatus, InfectionStatus::S);

fn main() {
    let mut context = Context::new();
    let current_dir = Path::new(file!()).parent().unwrap();
    let file_path = current_dir
        .join("input.json");

    match parameters_loader::init_parameters(&mut context, &file_path) {
        Ok(()) => {
            let parameters = context.get_global_property_value(Parameters).clone();
            context.init_random(parameters.seed);

            people_report::init(&mut context);
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
