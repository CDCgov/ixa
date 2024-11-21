use ixa::context::Context;
use ixa::define_rng;
use ixa::people::ContextPeopleExt;
use ixa::random::ContextRandomExt;

use rand_distr::Exp;

use crate::population_loader::InfectionStatus;
use crate::population_loader::InfectionStatusType;
use crate::FOI;
use crate::MAX_TIME;

define_rng!(TransmissionRng);

fn attempt_infection(context: &mut Context) {
    let population_size: usize = context.get_current_population();
    let id: usize = context.sample_range(TransmissionRng, 0..population_size);
    let person_to_infect = context.get_person_id(id);
    let person_status = context.get_person_property(person_to_infect, InfectionStatusType);

    if matches!(person_status, InfectionStatus::S) {
        context.set_person_property(person_to_infect, InfectionStatusType, InfectionStatus::I);
    }

    // With a food-borne illness (i.e., constant force of infection),
    // each _person_ experiences an exponentially distributed
    // time until infected. Here, we use a per-person force of infection derived from the population-level to represent a constant risk of infection for individuals in the population.

    // An alternative implementation calculates each person's time to infection
    // at the beginning of the simulation and scheudles their infection at that time.

    #[allow(clippy::cast_precision_loss)]
    let next_attempt_time = context.get_current_time()
        + context.sample_distr(TransmissionRng, Exp::new(FOI).unwrap()) / population_size as f64;

    if next_attempt_time <= MAX_TIME {
        context.add_plan(next_attempt_time, move |context| {
            attempt_infection(context);
        });
    }
}

pub fn init(context: &mut Context) {
    context.add_plan(0.0, |context| {
        attempt_infection(context);
    });
}

#[cfg(test)]
mod test {
    use super::*;
    use ixa::context::Context;

    #[test]
    fn test_attempt_infection() {
        let mut context = Context::new();
        context.init_random(0);
        let person = context.add_person(()).unwrap();

        // Infect a person
        attempt_infection(&mut context);

        // Execute and ensure they are infected
        context.execute();
        assert_eq!(
            context.get_person_property(person, InfectionStatusType),
            InfectionStatus::I
        );
    }
}
