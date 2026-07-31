use ixa::prelude::*;
use rand_distr::Exp;

use crate::parameters_loader::Foi;
use crate::population_manager::{AgeGroupRisk, Alive, InfectionStatus, Person};
use crate::Parameters;

define_rng!(TransmissionRng1);

//Attempt infection for specific age group risk (meaning different forces of infection)
fn attempt_infection(context: &mut Context, age_group: AgeGroupRisk) {
    let (population_size, person_to_infect) =
        context.count_and_sample_entity(TransmissionRng1, with!(Person, Alive(true), age_group));
    if let Some(person_to_infect) = person_to_infect {
        let parameters = context
            .get_global_property_value(Parameters)
            .expect("births-deaths parameters must be initialized before transmission")
            .clone();
        let foi = *context
            .get_global_property_value(Foi)
            .expect("births-deaths force-of-infection values must be initialized")
            .get(&age_group)
            .expect("births-deaths requires a force of infection for every configured age group");

        let person_status: InfectionStatus = context.get_property(person_to_infect);

        if person_status == InfectionStatus::S {
            context.set_property(person_to_infect, InfectionStatus::I);
        }
        let infection_distribution =
            Exp::new(foi).expect("births-deaths force-of-infection values must be positive");
        let next_attempt_time = context.get_current_time()
            + context.sample_distr(TransmissionRng1, infection_distribution)
                / population_size as f64;

        if next_attempt_time <= parameters.max_time {
            context.add_plan(next_attempt_time, move |context| {
                attempt_infection(context, age_group);
            });
        }
    }
}

pub fn init(context: &mut Context) {
    let foi_age_groups = context
        .get_global_property_value(Foi)
        .expect("births-deaths force-of-infection values must be initialized")
        .clone();
    for (age_group, _) in foi_age_groups {
        context.add_plan(0.0, move |context| {
            attempt_infection(context, age_group);
        });
    }
}
