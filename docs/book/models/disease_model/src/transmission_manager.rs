// ANCHOR: imports
use ixa::prelude::*;
use ixa::trace;
use rand_distr::Exp;

use crate::FORCE_OF_INFECTION;
use crate::people::{InfectionStatus, Person, PersonId};

define_rng!(TransmissionRng);
// ANCHOR_END: imports

// ANCHOR: attempt_infection
fn attempt_infection(context: &mut Context, infectee: PersonId) {
    trace!("Attempting infection");
    // check that this person is infectable
    let person_status: InfectionStatus = context.get_property(infectee);
    if person_status == InfectionStatus::S {
        context.set_property(infectee, InfectionStatus::I);
    }
}

pub fn init(context: &mut Context) {
    trace!("Initializing transmission manager");
    for person_id in context
        .query_result_iterator::<Person, _>(())
        .collect::<Vec<PersonId>>()
    {
        let infection_time =
            context.sample_distr(TransmissionRng, Exp::new(FORCE_OF_INFECTION).unwrap());
        context.add_plan(infection_time, move |c| attempt_infection(c, person_id));
    }
}
// ANCHOR_END: attempt_infection
