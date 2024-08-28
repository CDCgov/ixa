use ixa::context::Context;
use ixa::define_rng;
use ixa::random::ContextRandomExt;

use crate::people::ContextPeopleExt;
use crate::people::InfectionStatus;
use rand_distr::Exp;

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
    }

    let next_attempt_time =
        context.get_current_time() + context.sample_distr(TransmissionRng, Exp::new(FOI).unwrap());

    if next_attempt_time <= MAX_TIME {
        context.add_plan(next_attempt_time, move |context| {
            attempt_infection(context);
        });
    }
}

impl TransmissionManager for Context {
    fn initialize_transmission(&mut self) {
        self.add_plan(0.0, |context| {
            attempt_infection(context);
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::people::ContextPeopleExt;
    use crate::people::InfectionStatus;
    use crate::transmission_manager::TransmissionManager;
    use crate::SEED;
    use ixa::context::Context;
    use rand_distr::Exp;

    #[test]
    fn test_attempt_infection() {
        let mut context = Context::new();
        context.init_random(SEED);
        context.create_person();
        attempt_infection(&mut context);
        let person_status = context.get_person_status(0);
        assert_eq!(person_status, InfectionStatus::I);
        context.execute();
    }
}
