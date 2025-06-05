// ANCHOR: imports
use rand_distr::Exp;

use ixa::{prelude::*, trace, PersonId};

use crate::people::{InfectionStatus, InfectionStatusValue};
use crate::FORCE_OF_INFECTION;
use crate::POPULATION;

define_rng!(TransmissionRng);
// ANCHOR_END: imports

// ANCHOR: attempt_infection
fn attempt_infection(context: &mut Context) {
    trace!("Attempting infection");

    if let Some(person_to_infect) = context.sample_person(TransmissionRng, ()) {
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

        context.add_plan(next_attempt_time, attempt_infection);
    }
}

pub fn init(context: &mut Context) {
    trace!("Initializing transmission manager");
    context.add_plan(0.0, attempt_infection);
}
// ANCHOR_END: attempt_infection
