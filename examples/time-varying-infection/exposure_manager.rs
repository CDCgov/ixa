use ixa::define_rng;
use ixa::people::{ContextPeopleExt, PersonCreatedEvent, PersonId};
use ixa::random::ContextRandomExt;
use ixa::{context::Context, global_properties::ContextGlobalPropertiesExt};

use crate::parameters_loader::Parameters;
use crate::population_loader::{DiseaseStatus, DiseaseStatusType};
use rand_distr::Exp1;

use reikna::integral::*;
use roots::find_root_secant;

define_rng!(ExposureRng);

fn expose_person_to_deviled_eggs(context: &mut Context, person_created_event: PersonCreatedEvent) {
    // when the person is exposed to deviled eggs, make a plan for them to fall
    // sick based on foi(t), where inverse sampling is used to draw times from
    // the corresponding distribution
    inverse_sampling_infection(context, person_created_event.person_id);
}

// parameterize the foi
fn foi_t(t: f64, foi: f64, sin_shift: f64) -> f64 {
    foi * (f64::sin(t + sin_shift) + 1.0) // foi must always be greater than 1
}

fn inverse_sampling_infection(context: &mut Context, person_id: PersonId) {
    // random exponential value
    let s = context.sample_distr(ExposureRng, Exp1);
    // get the time by following the formula described above
    // first need to get the simulation's sin_shift
    let parameters = context.get_global_property_value(Parameters).clone();
    let sin_shift = parameters.foi_sin_shift;
    let foi = parameters.foi;
    let f = move |t| foi_t(t, foi, sin_shift);
    // as easy as Python to integrate and find roots in Rust!
    let f_int_shifted = move |t| integrate(&f, 0, t) - s;
    let t = find_root_secant(
        0f64,
        100f64, // lower and upper bounds for the root finding
        f_int_shifted,
        &mut 1e-3f64,
    )
    .unwrap();
    context.add_plan(t, move |context| {
        context.set_person_property(person_id, DiseaseStatusType, DiseaseStatus::I);
        // for reasons that will become apparent with the recovery rate example,
        // we also need to record the time at which a person becomes infected
        context.set_person_property(person_id, InfectionTime, t);
    });
}

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

fn init(context: &mut Context) {
    // let deviled eggs be our food borne illness
    // as soon as a person enters the simulation, they are exposed to deviled eggs
    // based on foi(t), they will have their infection planned at a given time
    context.subscribe_to_event(move |context, event: PersonCreatedEvent| {
        expose_person_to_deviled_eggs(context, event);
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
            foi_sin_shift: 3,
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
