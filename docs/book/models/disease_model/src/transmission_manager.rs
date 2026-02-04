// ANCHOR: imports
use ixa::prelude::*;
use ixa::trace;
use rand_distr::Exp;

use crate::people::{InfectionStatus, PersonId};
use crate::{FORCE_OF_INFECTION, POPULATION};

define_rng!(TransmissionRng);
// ANCHOR_END: imports

// ANCHOR: attempt_infection
fn attempt_infection(context: &mut Context) {
    trace!("Attempting infection");
    let person_to_infect: PersonId = context.sample_entity(TransmissionRng, all!(Person)).unwrap();
    let person_status: InfectionStatus = context.get_property(person_to_infect);

    if person_status == InfectionStatus::S {
        context.set_property(person_to_infect, InfectionStatus::I);
    }

    #[allow(clippy::cast_precision_loss)]
    let next_attempt_time = context.get_current_time()
        + context.sample_distr(TransmissionRng, Exp::new(FORCE_OF_INFECTION).unwrap())
        / POPULATION as f64;

    context.add_plan(next_attempt_time, attempt_infection);
}

pub fn init(context: &mut Context) {
    trace!("Initializing transmission manager");
    context.add_plan(0.0, attempt_infection);
}
// ANCHOR_END: attempt_infection
