use ixa::context::Context;
use ixa::define_rng;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::{ContextPeopleExt, PersonId, PersonPropertyChangeEvent};
use ixa::random::ContextRandomExt;

use rand_distr::Exp;

use crate::parameters_loader::Parameters;
use crate::population_loader::{DiseaseStatus, DiseaseStatusType, InfectionTime};

define_rng!(InfectionRng);

fn recovery_cdf(context: &mut Context, time_spent_infected: f64) -> f64 {
    1.0 - f64::exp(-time_spent_infected * n_eff_inv_infec(context))
}

#[allow(clippy::cast_precision_loss)]
fn n_eff_inv_infec(context: &mut Context) -> f64 {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    // get number of infected people
    let n_infected = context.query_people_count((DiseaseStatusType, DiseaseStatus::I));
    (1.0 / parameters.infection_duration) / (n_infected as f64)
}

fn evaluate_recovery(
    context: &mut Context,
    person_id: PersonId,
    resampling_rate: f64,
) -> Option<f64> {
    // get time person has spent infected
    let time_spent_infected = context.get_current_time()
        - *context
            .get_person_property(person_id, InfectionTime)
            .unwrap();
    // evaluate whether recovery has happened by this time or not
    let recovery_probability = recovery_cdf(context, time_spent_infected);
    if context.sample_bool(InfectionRng, recovery_probability) {
        // recovery has happened by now
        context.set_person_property(person_id, DiseaseStatusType, DiseaseStatus::R);
        Some(context.get_current_time())
    } else {
        // add plan for recovery evaluation to happen again at fastest rate
        context.add_plan(
            context.get_current_time()
                + context.sample_distr(InfectionRng, Exp::new(resampling_rate).unwrap()),
            move |context| {
                evaluate_recovery(context, person_id, resampling_rate);
            },
        );
        None
    }
}

fn handle_infection_status_change(
    context: &mut Context,
    event: PersonPropertyChangeEvent<DiseaseStatusType>,
) {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    if matches!(event.current, DiseaseStatus::I) {
        // recall resampling rate is sum of maximum foi rate and gamma
        // maximum foi rate is foi * 2 -- the 2 because foi is sin(t + c) + 1
        evaluate_recovery(
            context,
            event.person_id,
            parameters.foi * 2.0 + 1.0 / parameters.infection_duration,
        );
    }
}

pub fn init(context: &mut Context) {
    context.subscribe_to_event(
        move |context, event: PersonPropertyChangeEvent<DiseaseStatusType>| {
            handle_infection_status_change(context, event);
        },
    );
}

#[cfg(test)]
mod test {
    use super::*;
    use ixa::context::Context;
    use ixa::people::{ContextPeopleExt, PersonPropertyChangeEvent};
    use ixa::random::ContextRandomExt;
    use ordered_float::OrderedFloat;

    use crate::parameters_loader::ParametersValues;
    use crate::population_loader::{DiseaseStatus, DiseaseStatusType};

    fn handle_recovery_event(event: PersonPropertyChangeEvent<DiseaseStatusType>) {
        assert_eq!(event.current, DiseaseStatus::R);
        assert_eq!(event.previous, DiseaseStatus::I);
    }

    #[test]
    fn test_handle_infection_change() {
        let p_values = ParametersValues {
            population: 1,
            max_time: 10.0,
            seed: 42,
            foi: 0.15,
            foi_sin_shift: 3.0,
            infection_duration: 5.0,
            report_period: 1.0,
            output_dir: ".".to_string(),
            output_file: ".".to_string(),
        };
        let mut context = Context::new();

        context
            .set_global_property_value(Parameters, p_values)
            .unwrap();
        let parameters = context
            .get_global_property_value(Parameters)
            .unwrap()
            .clone();
        context.init_random(parameters.seed);
        init(&mut context);

        for _ in 0..parameters.population {
            let person_id = context
                .add_person((InfectionTime, Some(OrderedFloat(0.0))))
                .unwrap();
            context.set_person_property(person_id, DiseaseStatusType, DiseaseStatus::I);
        }

        // put this subscription after every agent has become infected
        // so that handle_recovery_event is not triggered by an S --> I transition
        // but only I --> R transitions, which is what it checks for
        context.subscribe_to_event(
            move |_context, event: PersonPropertyChangeEvent<DiseaseStatusType>| {
                handle_recovery_event(event);
            },
        );

        context.execute();
    }

    #[test]
    fn test_n_eff_inv_infec_recovery_cdf() {
        let mut context = Context::new();
        let parameters = ParametersValues {
            population: 100,
            max_time: 10.0,
            seed: 42,
            foi: 0.15,
            foi_sin_shift: 3.0,
            infection_duration: 5.0,
            report_period: 1.0,
            output_dir: ".".to_string(),
            output_file: ".".to_string(),
        };
        let mut people = Vec::new();

        context
            .set_global_property_value(Parameters, parameters.clone())
            .unwrap();
        context.init_random(parameters.seed);
        for _ in 0..parameters.population {
            let person_id = context.add_person(()).unwrap();
            people.push(person_id);
            context.set_person_property(person_id, DiseaseStatusType, DiseaseStatus::I);
        }
        assert_eq!(
            n_eff_inv_infec(&mut context),
            #[allow(clippy::cast_precision_loss)]
            1.0 / parameters.infection_duration
                / parameters.population as f64
        );
        let time_spent_infected = 0.5;
        let cdf_value_many_infected = recovery_cdf(&mut context, time_spent_infected);
        // now make it so that all but 1 person becomes recovered
        for i in 1..parameters.population {
            context.set_person_property(people[i], DiseaseStatusType, DiseaseStatus::R);
        }
        assert_eq!(
            n_eff_inv_infec(&mut context),
            1.0 / parameters.infection_duration
        );
        // calculate cdf again
        let cdf_value_few_infected = recovery_cdf(&mut context, time_spent_infected);
        // we expect the cdf value when few are infected to be greater than when many are infected
        // if we've written the cdf equation correctly
        assert!(cdf_value_few_infected >= cdf_value_many_infected);
    }

    #[test]
    fn test_rejection_sampling_no_change_infecteds() {
        // if there is no change in the number of infected people
        // the recovery time should be the parameter.infection_duration
        let parameters = ParametersValues {
            population: 1,
            max_time: 10.0,
            seed: 42,
            foi: 0.15,
            foi_sin_shift: 3.0,
            infection_duration: 5.0,
            report_period: 1.0,
            output_dir: ".".to_string(),
            output_file: ".".to_string(),
        };
        let n_iter = 10000;
        let mut sum = 0.0;
        for seed in 0..n_iter {
            let mut context = Context::new();
            context
                .set_global_property_value(Parameters, parameters.clone())
                .unwrap();
            context.init_random(seed);
            init(&mut context);
            let person_id = context
                .add_person((InfectionTime, Some(OrderedFloat(0.0))))
                .unwrap();
            context.set_person_property(person_id, DiseaseStatusType, DiseaseStatus::I);
            // there should only be one infected person in the simulation
            assert_eq!(
                n_eff_inv_infec(&mut context),
                1.0 / parameters.infection_duration
            );
            context.execute();
            // there should be zero infected people in the simulation
            assert_eq!(n_eff_inv_infec(&mut context), 1.0 / 0.0);
            sum += context.get_current_time();
        }
        // permit up to 5% error
        println!(
            "{}",
            (((sum / n_iter as f64) / parameters.infection_duration) - 1.0).abs()
        );
        assert!((((sum / n_iter as f64) / parameters.infection_duration) - 1.0).abs() < 0.05);
        // the implementation of rejection sampling here for constant n_eff_inv_infec
        // should be downwardly biased
        assert!((sum / n_iter as f64) < parameters.infection_duration);
    }
}
