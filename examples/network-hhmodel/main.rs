use std::path::Path;

use ixa::{context::Context, random::ContextRandomExt, ContextPeopleExt};
use ixa::{define_rng, ContextGlobalPropertiesExt};
mod loader;
mod network;
mod parameters;
mod seir;

define_rng!(MainRng);

fn main() {
    let mut context = Context::new();

    context.init_random(42);

    // Load people from csv and set up some base properties
    let people = loader::init(&mut context);

    // Load parameters from json
    let file_path = Path::new(file!()).parent().unwrap().join("config.json");
    context.load_global_properties(&file_path).unwrap();

    // Load network
    network::init(&mut context, &people);

    let to_infect = vec![context.sample_person(MainRng, ()).unwrap()];
    seir::init(&mut context, &to_infect);

    context.execute();
}
