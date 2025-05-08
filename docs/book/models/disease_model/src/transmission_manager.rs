use rand_distr::Exp;

use ixa::prelude::*;

use crate::people::{InfectionStatus, InfectionStatusValue};
use crate::FORCE_OF_INFECTION;
use crate::MAX_TIME;
use crate::POPULATION;

define_rng!(TransmissionRng);

fn attempt_infection(context: &mut Context) {
    trace!("Attempting infection");

    let person_to_infect: PersonId = context.sample_person(TransmissionRng, ()).unwrap();
    let person_status: InfectionStatusValue =
        context.get_person_property(person_to_infect, InfectionStatus);

    if person_status == InfectionStatusValue::S {
        context.set_person_property::<InfectionStatus>(
            person_to_infect,
            InfectionStatus,
            InfectionStatusValue::I,
        );
    }

    #[allow(clippy::cast_precision_loss)]
    let next_attempt_time = context.get_current_time()
        + context.sample_distr(TransmissionRng, Exp::new(FORCE_OF_INFECTION).unwrap())
            / POPULATION as f64;

    if next_attempt_time <= MAX_TIME {
        context.add_plan(next_attempt_time, attempt_infection);
    }
}

pub fn init(context: &mut Context) {
    trace!("Initializing transmission manager");
    context.add_plan(0.0, attempt_infection);
}
