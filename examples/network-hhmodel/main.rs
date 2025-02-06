use ixa::runner::run_with_args;
use ixa::{context::Context, random::ContextRandomExt, ContextPeopleExt};
use ixa::{define_rng, ContextGlobalPropertiesExt, PersonId};
use seir::InfectedBy;
mod incidence_report;
mod loader;
mod network;
mod parameters;
mod seir;
use std::path::Path;
define_rng!(MainRng);

fn main() {
    run_with_args(|context, _, _| {
        initialize(context);
        Ok(())
    })
    .unwrap();
}

fn initialize(context: &mut Context) {
    context.init_random(1);

    // Load people from csv and set up some base properties
    let people = loader::init(context);

    // Load parameters from json
    let file_path = Path::new(file!()).parent().unwrap().join("config.json");
    context.load_global_properties(&file_path).unwrap();

    // Load network
    network::init(context, &people);

    // Initialize incidence report
    incidence_report::init(context).unwrap();

    // Initialize infected person with InfectedBy value equal to their own PersonId
    let to_infect: Vec<PersonId> = vec![context
        .sample_person(MainRng, ())
        .unwrap()];
    context.set_person_property(to_infect[0], InfectedBy, Some(to_infect[0]));

    #[allow(clippy::vec_init_then_push)]
    seir::init(context, &to_infect);
    context.execute();
}
