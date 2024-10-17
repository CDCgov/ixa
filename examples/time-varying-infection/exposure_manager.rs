use ixa::define_rng;
use ixa::people::ContextPeopleExt;
use ixa::random::ContextRandomExt;
use ixa::{context::Context, global_properties::ContextGlobalPropertiesExt};

use crate::parameters_loader::Parameters;
use crate::population_loader::{DiseaseStatus, DiseaseStatusType};
use rand_distr::Exp;

define_rng!(ExposureRng);

fn attempt_infection(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters).clone();
    let population_size: usize = context.get_current_population();
    let person_to_infect: usize = context.sample_range(ExposureRng, 0..population_size);

    let person_status: DiseaseStatus =
        context.get_person_property(context.get_person_id(person_to_infect), DiseaseStatusType);

    if matches!(person_status, DiseaseStatus::S) {
        context.set_person_property(
            context.get_person_id(person_to_infect),
            DiseaseStatusType,
            DiseaseStatus::I,
        );
    }

    // With a food-borne illness (i.e., constant force of infection),
    // each _person_ experiences an exponentially distributed
    // time until infected. Here, we use a per-person force of infection derived
    // from the population-level to represent a constant risk of infection for individuals
    // in the population.

    // An alternative implementation calculates each person's time to infection
    // at the beginning of the simulation and scheudles their infection at that time.

    #[allow(clippy::cast_precision_loss)]
    let next_attempt_time = context.get_current_time()
        + context.sample_distr(ExposureRng, Exp::new(parameters.foi).unwrap())
            / population_size as f64;

    context.add_plan(next_attempt_time, move |context| {
        attempt_infection(context);
    });
}

pub fn init(context: &mut Context) {
    context.add_plan(0.0, |context| {
        attempt_infection(context);
    });
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::population_loader::{DiseaseStatus, DiseaseStatusType};
    use ixa::context::Context;
    use ixa::global_properties::ContextGlobalPropertiesExt;
    use ixa::people::ContextPeopleExt;
    use ixa::random::ContextRandomExt;

    use crate::parameters_loader::ParametersValues;

    #[test]
    fn test_attempt_infection() {
        let p_values = ParametersValues {
            population: 1,
            max_time: 10.0,
            seed: 42,
            foi: 0.15,
            infection_duration: 5.0,
            output_dir: ".".to_string(),
            output_file: ".".to_string(),
        };
        let mut context = Context::new();
        context.set_global_property_value(Parameters, p_values);
        let parameters = context.get_global_property_value(Parameters).clone();
        context.init_random(parameters.seed);
        context.add_person();
        attempt_infection(&mut context);
        let person_status =
            context.get_person_property(context.get_person_id(0), DiseaseStatusType);
        assert_eq!(person_status, DiseaseStatus::I);
    }
}
