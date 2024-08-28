use ixa::context::Context;
use ixa::random::ContextRandomExt;

mod incidence_report;
mod infection_manager;
mod people;
mod transmission_manager;

use crate::incidence_report::IncidenceReport;
use crate::infection_manager::InfectionManager;
use crate::people::ContextPeopleExt;
use crate::transmission_manager::TransmissionManager;

static POPULATION: u64 = 10;
static SEED: u64 = 123;
static MAX_TIME: f64 = 100.0;
static FOI: f64 = 0.1;
static INFECTION_DURATION: f64 = 5.0;

fn main() {
    let mut context = Context::new();

    for _ in 0..POPULATION {
        context.create_person();
    }

    context.init_random(SEED);

    context.initialize_transmission();
    context.initialize_infection_manager();
    context.initialize_incidence_report();
    context.execute();
}
