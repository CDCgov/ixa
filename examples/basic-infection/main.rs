use ixa::context::Context;
mod people;
mod transmission_manager;
use crate::people::PeopleContext;
use crate::transmission_manager::TransmissionManager;

static POPULATION: u64 = 100;
static SEED:u64 = 123;

fn main() {
    let mut context = Context::new();

    for person_id in 0..POPULATION {
        context.create_person();
    }
    context.init_random(SEED);
    
    context.initialize_transmission();
    
    context.execute();
}
