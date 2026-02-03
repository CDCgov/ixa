use ixa::prelude::*;

use crate::reference_sir::{
    periodic_counts::InfectionStatus,
    sir_baseline_alt::Model,
    sir_ixa::{entities_sir::InfectionLoop, NextPersonRng},
    Parameters, ParametersBuilder,
};

fn get_parameters(population: usize) -> Parameters {
    ParametersBuilder::default()
        .population(population)
        .build()
        .unwrap()
}

pub fn baseline_model_set_random(population: usize) {
    type InfectionStatus = crate::reference_sir::sir_baseline_alt::InfectionStatus;
    let mut model = Model::new(get_parameters(population));
    // Add all susceptible people
    for _ in 0..population {
        model.add_person(InfectionStatus::Susceptible);
    }
    // Randomly change people to infected
    for _ in 0..population {
        let person_id = model.sample_random_person();
        model.set_infection_status(person_id, InfectionStatus::Infectious);
    }
    // Randomly select infected people and recover them
    for _ in 0..model.n_infectious() {
        let person_id = model.sample_random_infected_person();
        model.set_infection_status(person_id, InfectionStatus::Recovered);
    }
}

pub fn ixa_model_set_random(population: usize) {
    type Person = crate::reference_sir::sir_ixa::entities_sir::Person;
    type InfectionStatus = crate::reference_sir::sir_ixa::entities_sir::InfectionStatus;

    let mut context = Context::new();
    context.init_random(8675309);
    context.index_property::<Person, InfectionStatus>();

    // Add all susceptible people
    for _ in 0..population {
        let _ = context.add_entity::<Person, _>((InfectionStatus::Susceptible,));
    }
    // Randomly change people to infected
    for _ in 0..population {
        let person_id = context.sample_entity(NextPersonRng, ()).unwrap();
        context.set_property(person_id, InfectionStatus::Infectious);
    }
    // Randomly select infected people and recover them
    for _ in 0..context.infected_people() {
        let person_id = context
            .sample_entity(NextPersonRng, (InfectionStatus::Infectious,))
            .unwrap();
        context.set_property(person_id, InfectionStatus::Infectious);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn ixa() {
        ixa_model_set_random(10_000);
    }

    #[test]
    fn baseline() {
        ixa_model_set_random(10_000);
    }
}
