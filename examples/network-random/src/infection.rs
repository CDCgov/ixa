use ixa::impl_property;
use ixa::log::info;
use ixa::prelude::*;
use serde::{Deserialize, Serialize};

use crate::parameters::Parameters;
use crate::{network, Person, PersonId};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
pub enum DiseaseStatus {
    S,
    I,
}

impl_property!(DiseaseStatus, Person, default_const = DiseaseStatus::S);

define_rng!(InfectionRng);

fn infect(context: &mut Context, infector: Option<PersonId>, infectee: PersonId) {
    let generation_interval = context
        .get_global_property_value(Parameters)
        .unwrap()
        .generation_interval;

    let status: DiseaseStatus = context.get_property(infectee);
    if status == DiseaseStatus::S {
        info!(
            "{infector:?} infected {infectee:?} at time {}.",
            context.get_current_time()
        );

        context.set_property(infectee, DiseaseStatus::I);

        // schedule onward infections: this infectee becomes the next infector
        let next_infector = infectee;
        for next_infectee in network::get_connections(context, infectee) {
            schedule_relative!(
                context,
                generation_interval,
                infect,
                Some(next_infector),
                next_infectee
            );
        }
    } else {
        info!("{infector:?} could not infect {infectee:?}, who was already infected");
    }
}

pub fn init(context: &mut Context, n_initial_infections: usize) {
    for infectee in context.sample_entities(InfectionRng, with!(Person), n_initial_infections) {
        infect(context, None, infectee);
    }
}

#[cfg(test)]
mod tests {
    use ixa::context::Context;

    use super::*;
    use crate::network;
    use crate::parameters::ParametersValues;

    #[test]
    fn test_disease_status() {
        let mut context = Context::new();
        context.init_random(42);

        let parameters = ParametersValues {
            generation_interval: 1.0,
            population_size: 100,
            n_connections: 10,
            n_initial_infected: 1,
        };
        context
            .set_global_property_value(Parameters, parameters.clone())
            .unwrap();

        let seed = 128381;
        network::init(
            &mut context,
            parameters.population_size,
            parameters.n_connections,
            seed,
        );
        init(&mut context, parameters.n_initial_infected);

        context.execute();

        let n_i = context.query_entity_count(with!(Person, DiseaseStatus::I));
        let n_s = context.query_entity_count(with!(Person, DiseaseStatus::S));

        assert_eq!(n_i + n_s, parameters.population_size);
        assert_eq!(n_i, 1);
    }
}
