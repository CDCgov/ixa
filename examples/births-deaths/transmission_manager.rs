use ixa::context::Context;
use ixa::define_rng;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::ContextPeopleExt;
use ixa::random::ContextRandomExt;

use crate::parameters_loader::Foi;
use crate::population_manager::AgeGroupRisk;
use crate::population_manager::ContextPopulationExt;
use crate::population_manager::InfectionStatus;
use crate::population_manager::InfectionStatusType;
use crate::Parameters;
use rand_distr::Exp;

define_rng!(TransmissionRng);

//Attempt infection for specific age group risk (meaning diferent forces of infection)
fn attempt_infection(context: &mut Context, age_group: AgeGroupRisk) {
    let population_size: usize = context.get_current_group_population(age_group);
    let parameters = context.get_global_property_value(Parameters).clone();
    let foi = *context
        .get_global_property_value(Foi)
        .get(&age_group)
        .unwrap();
    if population_size > 0 {
        let person_to_infect = context.sample_person(age_group).unwrap();

        let person_status: InfectionStatus =
            context.get_person_property(person_to_infect, InfectionStatusType);

        if matches!(person_status, InfectionStatus::S) {
            context.set_person_property(person_to_infect, InfectionStatusType, InfectionStatus::I);
        }
        #[allow(clippy::cast_precision_loss)]
        let next_attempt_time = context.get_current_time()
            + context.sample_distr(TransmissionRng, Exp::new(foi).unwrap())
                / population_size as f64;

        if next_attempt_time <= parameters.max_time {
            context.add_plan(next_attempt_time, move |context| {
                attempt_infection(context, age_group);
            });
        }
    }
}

pub fn init(context: &mut Context) {
    let foi_age_groups = context.get_global_property_value(Foi).clone();
    for (age_group, _) in foi_age_groups {
        context.add_plan(0.0, move |context| {
            attempt_infection(context, age_group);
        });
    }
}
