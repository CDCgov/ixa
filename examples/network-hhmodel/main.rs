use std::path::Path;

use ixa::{context::Context, random::ContextRandomExt, ContextPeopleExt};
use ixa::{define_rng, ContextGlobalPropertiesExt, PersonId};
use seir::InfectedBy;
mod incidence_report;
mod loader;
mod network;
mod parameters;
mod seir;

define_rng!(MainRng);

fn main() {
    let mut context = Context::new();

    context.init_random(1);

    // Load people from csv and set up some base properties
    let people = loader::init(&mut context);

    // Load parameters from json
    let file_path = Path::new(file!()).parent().unwrap().join("config.json");
    context.load_global_properties(&file_path).unwrap();

    // Load network
    network::init(&mut context, &people);

    // Initialize incidence report
    incidence_report::init(&mut context).unwrap();

    // Initialize infected person with InfectedBy value equal to their own PersonId
    let to_infect: Vec<PersonId> = vec![context
        .sample_person(MainRng, ())
        .unwrap()];
    context.set_person_property(to_infect[0], InfectedBy, Some(to_infect[0]));

    #[allow(clippy::vec_init_then_push)]
    seir::init(&mut context, &to_infect);
    seir::init(&mut context, &to_infect);
    context.execute();
}
