use ixa::prelude::*;
use rand_distr::Exp;

use crate::parameters_loader::Foi;
use crate::population_manager::{AgeGroupRisk, Alive, InfectionStatus, Person};
use crate::Parameters;

define_rng!(TransmissionRng1);

//Attempt infection for specific age group risk (meaning different forces of infection)
fn attempt_infection(context: &mut Context, age_group: AgeGroupRisk) {
    let query = with!(Person, Alive(true), age_group);
    let population_size: usize = context.query_entity_count(query);
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    let foi = *context
        .get_global_property_value(Foi)
        .unwrap()
        .get(&age_group)
        .unwrap();
    if population_size > 0 {
        let person_to_infect = context.sample_entity(TransmissionRng1, query).unwrap();

        let person_status: InfectionStatus = context.get_property(person_to_infect);

        if person_status == InfectionStatus::S {
            context.set_property(person_to_infect, InfectionStatus::I);
        }
        #[allow(clippy::cast_precision_loss)]
        let next_attempt_time = context.get_current_time()
            + context.sample_distr(TransmissionRng1, Exp::new(foi).unwrap())
                / population_size as f64;

        if next_attempt_time <= parameters.max_time {
            context.add_plan(next_attempt_time, move |context| {
                attempt_infection(context, age_group);
            });
        }
    }
}

pub fn init(context: &mut Context) {
    let foi_age_groups = context.get_global_property_value(Foi).unwrap().clone();
    for (age_group, _) in foi_age_groups {
        context.add_plan(0.0, move |context| {
            attempt_infection(context, age_group);
        });
    }
}
