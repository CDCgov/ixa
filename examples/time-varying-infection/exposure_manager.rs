use ixa::define_rng;
use ixa::people::{ContextPeopleExt, PersonCreatedEvent};
use ixa::random::ContextRandomExt;
use ixa::{context::Context, global_properties::ContextGlobalPropertiesExt};
use std::rc::Rc;

use crate::parameters_loader::Parameters;
use crate::population_loader::{DiseaseStatus, DiseaseStatusType, InfectionTime};
use rand_distr::Exp1;

use reikna::func;
use reikna::func::Function;
use reikna::integral::integrate;
use roots::find_root_brent;

define_rng!(ExposureRng);

fn expose_person_to_deviled_eggs(context: &mut Context, person_created_event: PersonCreatedEvent) {
    // when the person is exposed to deviled eggs, make a plan for them to fall
    // sick based on foi(t), where inverse sampling is used to draw times from
    // the corresponding distribution
    let t = inverse_sampling_infection(context);
    let person_id = person_created_event.person_id;
    context.add_plan(t, move |context| {
        context.set_person_property(person_id, DiseaseStatusType, DiseaseStatus::I);
        // for reasons that will become apparent with the recovery rate example,
        // we also need to record the time at which a person becomes infected
        context.initialize_person_property(person_id, InfectionTime, t);
    });
}

// parameterize the foi
fn foi_t(t: f64, foi: f64, sin_shift: f64) -> f64 {
    foi * (f64::sin(t + sin_shift) + 1.0) // foi must always be greater than 0
}

fn inverse_sampling_infection(context: &mut Context) -> f64 {
    // random exponential value
    let s: f64 = context.sample_distr(ExposureRng, Exp1);
    // get the time by following the formula described above
    // first need to get the simulation's sin_shift
    let parameters = context.get_global_property_value(Parameters).clone();
    let sin_shift = parameters.foi_sin_shift;
    let foi = parameters.foi;
    let f = func!(move |t| foi_t(t, foi, sin_shift));
    // as easy as Python to integrate and find roots in Rust!
    let f_int_shifted = move |t| integrate(&f, 0.0, t) - s;
    find_root_brent(
        0.0,
        parameters.max_time, // lower and upper bounds for the root finding
        f_int_shifted,
        &mut 1e-2f64,
    )
    .unwrap()
}

pub fn init(context: &mut Context) {
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
    use reikna::integral::integrate;

    use crate::parameters_loader::ParametersValues;

    #[test]
    fn test_attempt_infection() {
        let p_values = ParametersValues {
            population: 1,
            max_time: 200.0,
            seed: 42,
            foi: 0.15,
            foi_sin_shift: 3.0,
            infection_duration: 5.0,
            report_period: 1.0,
            output_dir: ".".to_string(),
            output_file: ".".to_string(),
        };
        let mut context = Context::new();
        context.set_global_property_value(Parameters, p_values);
        let parameters = context.get_global_property_value(Parameters).clone();
        context.init_random(parameters.seed);
        init(&mut context);
        context.add_person();
        context.execute();
        let person_status =
            context.get_person_property(context.get_person_id(0), DiseaseStatusType);
        assert_eq!(person_status, DiseaseStatus::I);
        let infection_time = context.get_person_property(context.get_person_id(0), InfectionTime);
        assert_eq!(infection_time, context.get_current_time());
    }

    #[test]
    fn test_mean_inverse_sampling() {
        // calculate empirical mean and compare to theoretical mean
        // challenging to test the distribution of times
        // we get out from inverse sampling is correct,
        // but we can at least test that whether the mean is correct
        // because the theoretical mean is easily calculatable form
        // the hazard rate using survival analysis
        let p_values = ParametersValues {
            population: 1,
            max_time: 200.0,
            seed: 42,
            foi: 0.15,
            foi_sin_shift: 3.0,
            infection_duration: 5.0,
            report_period: 1.0,
            output_dir: ".".to_string(),
            output_file: ".".to_string(),
        };
        let mut context = Context::new();
        context.set_global_property_value(Parameters, p_values);
        let parameters = context.get_global_property_value(Parameters).clone();
        context.init_random(parameters.seed);
        // empirical mean
        let mut sum = 0.0;
        let n = 1000;
        for _ in 0..n {
            sum += inverse_sampling_infection(&mut context);
        }
        let mean = sum / n as f64;

        // now calculate theoretical mean
        // use the fact that integral from 0 to infinity of survival fcn is mean
        let hazard_fcn = func!(move |t| foi_t(t, parameters.foi, parameters.foi_sin_shift));
        let survival_fcn = func!(move |t| f64::exp(-integrate(&hazard_fcn, 0.0, t)));
        let theoretical_mean = integrate(&survival_fcn, 0.0, 10000.0); // large enough upper bound
        assert!((mean - theoretical_mean).abs() < 0.1);
    }
}
