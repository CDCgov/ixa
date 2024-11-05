use ixa::people::ContextPeopleExt;
use ixa::random::ContextRandomExt;
use ixa::{
    context::Context, define_person_property, define_person_property_with_default,
    global_properties::ContextGlobalPropertiesExt,
};
use std::path::Path;

mod incidence_report;
mod infection_manager;
mod parameters_loader;
mod transmission_manager;

use crate::parameters_loader::Parameters;

use serde::{Deserialize, Serialize};

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum InfectionStatus {
    S,
    I,
    R,
}
define_person_property_with_default!(InfectionStatusType, InfectionStatus, InfectionStatus::S);

fn main() {
    let mut context = Context::new();
    let file_path = Path::new("examples")
        .join("parameter-loading")
        .join("input.json");

    match parameters_loader::init_parameters(&mut context, &file_path) {
        Ok(()) => {
            let parameters = context
                .get_global_property_value(Parameters)
                .unwrap()
                .clone();
            context.init_random(parameters.seed);

            for _ in 0..parameters.population {
                context.add_person(()).unwrap();
            }

            transmission_manager::init(&mut context);
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
}
