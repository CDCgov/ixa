use ixa::prelude::*;
use rand_distr::Exp;

use crate::{InfectionStatus, Parameters, Person, PersonId};

define_rng!(TransmissionRng);

fn attempt_infection(context: &mut Context) {
    let population_size: usize = context.get_entity_count::<Person>();
    let person_to_infect: PersonId = context.sample_entity(TransmissionRng, ()).unwrap();
    let person_status: InfectionStatus = context.get_property(person_to_infect);
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    if person_status == InfectionStatus::S {
        context.set_property(person_to_infect, InfectionStatus::I);
    }

    // With a food-borne illness (i.e., constant force of infection), each _person_
    // experiences an exponentially distributed time until infected. Here, we
    // use a per-person force of infection derived from the population-level to
    // represent a constant risk of infection for individuals in the population.

    // An alternative implementation calculates each person's time to infection at
    // the beginning of the simulation and schedules their infection at that time.

    #[allow(clippy::cast_precision_loss)]
    let next_attempt_time = context.get_current_time()
        + context.sample_distr(TransmissionRng, Exp::new(parameters.foi).unwrap())
            / population_size as f64;

    if next_attempt_time <= parameters.max_time {
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
    use crate::parameters_loader::ParametersValues;

    #[test]
    fn test_attempt_infection() {
        let p_values = ParametersValues {
            population: 10,
            max_time: 10.0,
            seed: 42,
            foi: 0.15,
            infection_duration: 5.0,
            output_dir: ".".to_string(),
            output_file: ".".to_string(),
        };

        let mut context = Context::new();
        context
            .set_global_property_value(Parameters, p_values)
            .unwrap();
        context.init_random(123);
        let pid: PersonId = context.add_entity(()).unwrap();
        attempt_infection(&mut context);
        let person_status: InfectionStatus = context.get_property(pid);
        assert_eq!(person_status, InfectionStatus::I);
        context.execute();
    }
}
