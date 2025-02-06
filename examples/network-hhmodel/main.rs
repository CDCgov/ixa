use ixa::runner::run_with_args;
use ixa::{context::Context, random::ContextRandomExt, ContextPeopleExt};
use ixa::{define_rng, ContextGlobalPropertiesExt, PersonId};
use seir::InfectedBy;
use std::path::Path;
mod incidence_report;
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
    network::init(&mut context, &people);
    incidence_report::init(&mut context).unwrap();

    let to_infect: Vec<PersonId> = vec![context
        .sample_person(MainRng, (AgeGroup, AgeGroupValue::Age18to64))
        .unwrap()];
    context.set_person_property(to_infect[0], InfectedBy, Some(to_infect[0]));
    #[allow(clippy::vec_init_then_push)]
    seir::init(&mut context, &to_infect);
    context.execute();
}
