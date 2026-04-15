use std::collections::HashSet;

use ixa::log::info;
use ixa::prelude::*;
use ixa::{impl_property, ExecutionPhase};
use rand_distr::Gamma;
use serde::{Deserialize, Serialize};

use crate::network::get_contacts;
use crate::parameters::Parameters;
use crate::{Person, PersonId};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
pub enum DiseaseStatus {
    S,
    E,
    I,
    R,
}

define_rng!(SeirRng);

impl_property!(DiseaseStatus, Person, default_const = DiseaseStatus::S);

define_property!(
    struct InfectedBy(Option<PersonId>),
    Person,
    default_const = InfectedBy(None)
);

fn calculate_waiting_time(context: &Context, shape: f64, mean_period: f64) -> f64 {
    let d = Gamma::new(shape, mean_period / shape).unwrap();
    context.sample_distr(SeirRng, d)
}

fn expose(context: &mut Context, infector: PersonId, infectee: PersonId) {
    info!(
        "{infector:?} exposed {infectee:?} at time {}.",
        context.get_current_time()
    );
    context.set_property(infectee, DiseaseStatus::E);
    context.set_property(infectee, with!(InfectedBy, Some(infector)));
}

fn schedule_waiting_event(
    context: &mut Context,
    person_id: PersonId,
    shape: f64,
    mean_period: f64,
    new_status: DiseaseStatus,
) {
    let ct = context.get_current_time();
    let waiting_time = calculate_waiting_time(context, shape, mean_period);

    context.add_plan(ct + waiting_time, move |context| {
        context.set_property(person_id, new_status);
    });
}

fn schedule_infection(context: &mut Context, person_id: PersonId) {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    schedule_waiting_event(
        context,
        person_id,
        parameters.shape,
        parameters.incubation_period,
        DiseaseStatus::I,
    );
}

fn schedule_recovery(context: &mut Context, person_id: PersonId) {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    schedule_waiting_event(
        context,
        person_id,
        parameters.shape,
        parameters.infectious_period,
        DiseaseStatus::R,
    );
}

pub fn init(context: &mut Context, initial_infections: &Vec<PersonId>, period: f64) {
    context.add_periodic_plan_with_phase(
        period,
        move |context| {
            // get all infector-infectee pairs
            let mut pairs = HashSet::new();
            for infector in context.query(with!(Person, DiseaseStatus::I)) {
                for infectee in get_contacts(context, infector, period) {
                    pairs.insert((infector, infectee));
                }
            }

            // do the exposures
            for (infector, infectee) in pairs {
                expose(context, infector, infectee)
            }
        },
        ExecutionPhase::Normal,
    );

    context.subscribe_to_event(
        move |context, event: PropertyChangeEvent<Person, DiseaseStatus>| match event.current {
            DiseaseStatus::E => schedule_infection(context, event.entity_id),
            DiseaseStatus::I => schedule_recovery(context, event.entity_id),
            _ => (),
        },
    );

    // expose the first people to the disease
    for ii in initial_infections {
        context.set_property(*ii, InfectedBy(Some(*ii)));
        context.set_property(*ii, DiseaseStatus::E);
    }
}

#[cfg(test)]
mod tests {
    use ixa::context::Context;

    use super::*;
    use crate::loader::Id;
    use crate::parameters::ParametersValues;
    use crate::{loader, network};

    #[test]
    fn test_disease_status() {
        let mut context = Context::new();

        context.init_random(42);

        let people = loader::init(&mut context);

        // set sar and between_hh_transmission_reduction to 1.0 so that
        // beta is 1.0
        let parameters = ParametersValues {
            incubation_period: 8.0,
            infectious_period: 27.0,
            sar: 1.0,
            shape: 15.0,
            infection_duration: 5.0,
            between_hh_transmission_reduction: 1.0,
            data_dir: "examples/network-hhmodel/tests".to_owned(),
            output_dir: "examples/network-hhmodel/tests".to_owned(),
        };
        context
            .set_global_property_value(Parameters, parameters)
            .unwrap();

        network::init(&mut context, &people);

        let mut to_infect = Vec::<PersonId>::new();
        context.with_query_results(with!(Person, Id(71)), &mut |people| {
            to_infect.extend(people);
        });

        init(&mut context, &to_infect, 1.0);

        context.execute();

        assert_eq!(
            context.query_entity_count::<Person, _>(with!(Person, DiseaseStatus::S)),
            399
        );
        assert_eq!(
            context.query_entity_count::<Person, _>(with!(Person, DiseaseStatus::E)),
            0
        );
        assert_eq!(
            context.query_entity_count::<Person, _>(with!(Person, DiseaseStatus::I)),
            0
        );
        assert_eq!(
            context.query_entity_count::<Person, _>(with!(Person, DiseaseStatus::R)),
            1207
        );
    }
}
