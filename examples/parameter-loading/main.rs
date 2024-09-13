use ixa::random::ContextRandomExt;
use ixa::{context::Context, global_properties::ContextGlobalPropertiesExt};
mod incidence_report;
mod infection_manager;
mod parameters_loader;
mod people;
mod transmission_manager;

use crate::parameters_loader::Parameters;
use crate::people::ContextPeopleExt;

fn main() {
    let mut context = Context::new();
    parameters_loader::init_parameters(&mut context, "examples/parameter-loading/input.toml");

    let parameters = context.get_global_property_value(Parameters).clone();
    context.init_random(parameters.seed);

    for _ in 0..parameters.population {
        context.create_person();
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
