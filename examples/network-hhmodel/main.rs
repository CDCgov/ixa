use ixa::{context::Context, random::ContextRandomExt, ContextPeopleExt};
use ixa::{define_rng, PersonId};
mod loader;
mod seir;
mod parameters;
mod network;

define_rng!(MainRng);

fn main() {
    let mut context = Context::new();

    context.init_random(42);

    // Load people from csv and set up some base properties
    let people = loader::init(&mut context);
    
    // Load parameters from json
    parameters::init(&mut context, "config.json").unwrap();

    // Load network
    network::init(&mut context, &people);

    let mut to_infect: Vec<PersonId> = Vec::new();
    to_infect.push(context.sample_person(MainRng, ()).unwrap());

    seir::init(&mut context, to_infect);

    context.execute();
}
