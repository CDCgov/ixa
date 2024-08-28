use ixa::context::Context;
use ixa::define_rng;
use ixa::random::ContextRandomExt;

use rand_distr::Exp;

use crate::infection_manager::InfectionManager;
use crate::people::InfectionStatus;
use crate::people::PeopleContext;

use crate::FOI;
use crate::MAX_TIME;

define_rng!(TransmissionRng);

pub trait TransmissionManager {
    fn initialize_transmission(&mut self);
}

fn attempt_infection(context: &mut Context) {
    let population_size: usize = context.get_population();
    let person_to_infect: usize = context.sample_range(TransmissionRng, 0..population_size);

    let person_status: InfectionStatus = context.get_person_status(person_to_infect);

    if matches!(person_status, InfectionStatus::S) {
        context.set_person_status(person_to_infect, InfectionStatus::I);
        //context.schedule_recovery(person_to_infect);
        println!(
            "{:?}, {:?}, I",
            context.get_current_time(),
            person_to_infect
        );
    }

    let next_attempt_time =
        context.get_current_time() + context.sample_distr(TransmissionRng, Exp::new(FOI).unwrap());

    if next_attempt_time <= MAX_TIME {
        context.add_plan(next_attempt_time, move |context| {
            attempt_infection(context);
        });
    }
}

fn initial_infection(context: &mut Context) {
    context.add_plan(0.0, move |context| {
        attempt_infection(context);
    });
}

impl TransmissionManager for Context {
    fn initialize_transmission(&mut self) {
        initial_infection(self);
    }
}
