use ixa::prelude::*;
use ixa::trace;
use rand_distr::Exp;

use crate::people::{InfectionStatus, Person, PersonId};
use crate::{FOI, MAX_TIME};

define_rng!(TransmissionRng);

fn attempt_infection(context: &mut Context) {
    trace!("Attempting infection");
    let population_size: usize = context.get_entity_count::<Person>();
    let person_to_infect: PersonId = context.sample_entity(TransmissionRng, Person).unwrap();

    let person_status: InfectionStatus = context.get_property(person_to_infect);

    if person_status == InfectionStatus::S {
        context.set_property(person_to_infect, InfectionStatus::I);
    }

    // With a food-borne illness (i.e., constant force of infection), each _person_ experiences an
    // exponentially distributed time until infected. Here, we use a per-person force of infection
    // derived from the population-level to represent a constant risk of infection for individuals
    // in the population.

    // An alternative implementation calculates each person's time to infection
    // at the beginning of the simulation and schedules their infection at that time.

    #[allow(clippy::cast_precision_loss)]
    let next_attempt_time = context.get_current_time()
        + context.sample_distr(TransmissionRng, Exp::new(FOI).unwrap()) / population_size as f64;

    if next_attempt_time <= MAX_TIME {
        context.add_plan(next_attempt_time, attempt_infection);
    }
}

pub fn init(context: &mut Context) {
    trace!("Initializing transmission manager");
    context.add_plan(0.0, attempt_infection);
}

#[cfg(test)]
mod test {
    use ixa::context::Context;

    use super::*;
    use crate::people::InfectionStatus;
    use crate::SEED;

    #[test]
    fn test_attempt_infection() {
        let mut context = Context::new();
        context.init_random(SEED);
        let person_id = context.add_entity(Person).unwrap();
        attempt_infection(&mut context);
        let person_status: InfectionStatus = context.get_property(person_id);
        assert_eq!(person_status, InfectionStatus::I);
        context.execute();
    }
}
