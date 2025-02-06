use ixa::runner::run_with_args;
use ixa::{context::Context, random::ContextRandomExt, ContextPeopleExt};
use ixa::{define_rng, ContextGlobalPropertiesExt};
use std::path::Path;
mod loader;
mod network;
mod parameters;
mod seir;

define_rng!(MainRng);

fn main() {
    run_with_args(|context, _, _| {
        initialize(context);
        Ok(())
    })
    .unwrap();
}

fn initialize(context: &mut Context) {
    context.init_random(42);

    // Load people from csv and set up some base properties
    let people = loader::init(context);

    // Load parameters from json
    let file_path = Path::new(file!()).parent().unwrap().join("config.json");
    context.load_global_properties(&file_path).unwrap();

    // Load network
    network::init(context, &people);

    let to_infect = vec![context.sample_person(MainRng, ()).unwrap()];
    seir::init(context, &to_infect);
}
