use ixa::prelude::*;
use ixa::runner::run_with_args;
use ixa::PersonId;
mod incidence_report;
mod loader;
mod network;
mod parameters;
mod seir;
use std::path::PathBuf;

define_rng!(MainRng);

fn main() {
    run_with_args(|context, _, _| {
        initialize(context);
        Ok(())
    })
    .unwrap();
}

fn example_dir() -> PathBuf {
    let parameters_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    parameters_path.join("examples").join("network-hhmodel")
}

fn initialize(context: &mut Context) {
    context.init_random(1);

    // Load people from csv and set up some base properties
    let people = loader::init(context);

    // Load parameters from json
    let file_path = example_dir().join("config.json");
    context.load_global_properties(&file_path).unwrap();

    // Load network
    network::init(context, &people);

    // Initialize incidence report
    incidence_report::init(context).unwrap();

    // Initialize infected person with InfectedBy value equal to their own PersonId
    let to_infect: Vec<PersonId> = vec![context.sample_person(MainRng, ()).unwrap()];

    #[allow(clippy::vec_init_then_push)]
    seir::init(context, &to_infect);
    context.execute();
}
