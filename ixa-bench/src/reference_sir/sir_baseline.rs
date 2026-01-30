use indexmap::IndexSet;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use rand_distr::Exp;

use super::{ModelStats, Parameters};

#[derive(Clone, Copy)]
pub enum InfectionStatus {
    Susceptible,
    Infectious,
    Recovered,
}

pub struct Model {
    parameters: Parameters,
    time: f64,
    rng: SmallRng,
    infection_status_lookup: Vec<InfectionStatus>,
    susceptible_people: IndexSet<PersonId>,
    infectious_people: IndexSet<PersonId>,
    recovered_people: IndexSet<PersonId>,
    population: usize,
    stats: ModelStats,
}

impl Model {
    pub fn new(parameters: Parameters) -> Model {
        let stats = ModelStats::new(parameters.initial_infections, parameters.population, 0.2);
        Model {
            infection_status_lookup: Vec::new(),
            susceptible_people: IndexSet::new(),
            infectious_people: IndexSet::new(),
            recovered_people: IndexSet::new(),
            population: 0,
            rng: SmallRng::seed_from_u64(parameters.seed),
            parameters,
            time: 0.0,
            stats,
        }
    }

    fn add_person(&mut self, infection_status: InfectionStatus) -> PersonId {
        self.infection_status_lookup.push(infection_status);
        let person_id = PersonId {
            id: self.population,
        };
        self.population += 1;
        match infection_status {
            InfectionStatus::Susceptible => {
                self.susceptible_people.insert(person_id);
            }
            InfectionStatus::Infectious => {
                self.infectious_people.insert(person_id);
            }
            InfectionStatus::Recovered => {
                self.recovered_people.insert(person_id);
            }
        }
        person_id
    }

    fn get_infection_status(&self, person_id: PersonId) -> InfectionStatus {
        *self.infection_status_lookup.get(person_id.id).unwrap()
    }

    fn set_infection_status(&mut self, person_id: PersonId, infection_status: InfectionStatus) {
        match infection_status {
            InfectionStatus::Susceptible => {
                self.susceptible_people.insert(person_id);
            }
            InfectionStatus::Infectious => {
                self.susceptible_people.swap_remove(&person_id);
                self.infectious_people.insert(person_id);
            }
            InfectionStatus::Recovered => {
                self.infectious_people.swap_remove(&person_id);
                self.recovered_people.insert(person_id);
            }
        }
        *self.infection_status_lookup.get_mut(person_id.id).unwrap() = infection_status;
    }

    fn infect_person(&mut self, person_id: PersonId, t: Option<f64>) {
        self.set_infection_status(person_id, InfectionStatus::Infectious);
        if let Some(current_t) = t {
            self.stats.record_infection(current_t);
        }
    }

    fn sample_random_person(&mut self) -> PersonId {
        let index = self.rng.random_range(0..self.population);
        PersonId { id: index }
    }

    pub fn get_stats(&self) -> &ModelStats {
        &self.stats
    }

    pub fn run(&mut self) {
        // Set up population
        for _ in 0..self.parameters.population {
            self.add_person(InfectionStatus::Susceptible);
        }

        // Seed infections
        for _ in 0..self.parameters.initial_infections {
            let n_susceptible = self.susceptible_people.len();
            let index = self.rng.random_range(0..n_susceptible);
            let person_to_infect = *self.susceptible_people.get_index(index).unwrap();
            self.infect_person(person_to_infect, None);
        }

        // Start infection loop
        let infection_rate = self.parameters.r0 / self.parameters.infectious_period;
        let mut n_infectious = self.infectious_people.len();

        while n_infectious > 0 && self.time < self.parameters.max_time {
            let infection_event_rate = infection_rate * (n_infectious as f64);
            let recovery_event_rate = (n_infectious as f64) / self.parameters.infectious_period;

            let infection_event_time = self.rng.sample(Exp::new(infection_event_rate).unwrap());
            let recovery_event_time = self.rng.sample(Exp::new(recovery_event_rate).unwrap());

            if infection_event_time < recovery_event_time {
                let person_to_infect = self.sample_random_person();
                if let InfectionStatus::Susceptible = self.get_infection_status(person_to_infect) {
                    self.time += infection_event_time;
                    self.infect_person(person_to_infect, Some(self.time));
                }
            } else {
                let index = self.rng.random_range(0..n_infectious);
                let person_to_recover = *self.infectious_people.get_index(index).unwrap();
                self.set_infection_status(person_to_recover, InfectionStatus::Recovered);
                self.stats.record_recovery();
                self.time += recovery_event_time;
            }

            n_infectious = self.infectious_people.len();
        }

        self.stats.check_extinction();
        // Print final stats
        println!("Cumulative incidence: {}", self.stats.get_cum_incidence());
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PersonId {
    id: usize,
}

#[cfg(test)]
mod test {
    use approx::assert_relative_eq;

    use super::super::ParametersBuilder;
    use super::*;

    #[test]
    fn test_attack_rate() {
        let mut context = Model::new(
            ParametersBuilder::default()
                .population(100_000)
                .build()
                .unwrap(),
        );
        context.run();

        // Final size relation is ~59%
        let incidence = context.get_stats().get_cum_incidence() as f64;
        let expected = context.parameters.population as f64 * 0.59;
        assert_relative_eq!(incidence, expected, max_relative = 0.02);
    }
}
