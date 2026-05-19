use indexmap::IndexSet;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use rand_distr::Exp;

use super::{ModelStats, Parameters};

const MIN_HOUSEHOLD_SIZE: usize = 5;

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
    household_members: Vec<Vec<PersonId>>,
    household_of: Vec<usize>,
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
            household_members: Vec::new(),
            household_of: Vec::new(),
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

    fn random_infectious_person(&mut self) -> PersonId {
        let index = self.rng.random_range(0..self.infectious_people.len());
        *self.infectious_people.get_index(index).unwrap()
    }

    fn random_contact(&mut self, source: PersonId) -> PersonId {
        let itinerary = self.parameters.itinerary;
        let total = itinerary.household + itinerary.community;
        let from_household = itinerary.household > 0.0
            && total > 0.0
            && self.rng.random_bool(itinerary.household / total);
        if !from_household {
            return self.sample_random_person();
        }
        let household_idx = self.household_of[source.id];
        let members = &self.household_members[household_idx];
        if members.len() <= 1 {
            return self.sample_random_person();
        }
        let self_idx = members.iter().position(|&p| p == source).unwrap();
        // Sample over members.len() - 1, then shift past self_idx so source
        // is never selected as their own contact.
        let mut idx = self.rng.random_range(0..members.len() - 1);
        if idx >= self_idx {
            idx += 1;
        }
        members[idx]
    }

    pub fn get_stats(&self) -> &ModelStats {
        &self.stats
    }

    pub fn run(&mut self) {
        // Set up households with at least MIN_HOUSEHOLD_SIZE members each, using
        // the same algorithm as the Ixa implementation for fair comparison.
        let population = self.parameters.population;
        let num_households = (population / MIN_HOUSEHOLD_SIZE).max(1);
        self.household_members.resize_with(num_households, Vec::new);

        let mut assignment: Vec<usize> = Vec::with_capacity(population);
        'fill: for h in 0..num_households {
            for _ in 0..MIN_HOUSEHOLD_SIZE {
                if assignment.len() == population {
                    break 'fill;
                }
                assignment.push(h);
            }
        }
        while assignment.len() < population {
            let h = self.rng.random_range(0..num_households);
            assignment.push(h);
        }

        // Add persons in assignment order and record their household.
        for &h_idx in &assignment {
            let person = self.add_person(InfectionStatus::Susceptible);
            self.household_of.push(h_idx);
            self.household_members[h_idx].push(person);
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
                let source = self.random_infectious_person();
                let person_to_infect = self.random_contact(source);
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

    use super::super::{Itinerary, ParametersBuilder};
    use super::*;

    #[test]
    fn test_attack_rate() {
        let mut context = Model::new(
            ParametersBuilder::default()
                .population(100_000)
                .itinerary(Itinerary {
                    household: 0.0,
                    community: 1.0,
                })
                .build()
                .unwrap(),
        );
        context.run();

        // Final size relation is ~59%
        let incidence = context.get_stats().get_cum_incidence() as f64;
        let expected = context.parameters.population as f64 * 0.59;
        assert_relative_eq!(incidence, expected, max_relative = 0.02);
    }

    #[test]
    fn test_attack_rate_with_households() {
        let population = 10_000;
        let mut model = Model::new(
            ParametersBuilder::default()
                .population(population)
                .initial_infections(100)
                .itinerary(Itinerary {
                    household: 0.5,
                    community: 0.5,
                })
                .build()
                .unwrap(),
        );
        model.run();

        // 50% household-channel contacts (households of 5) suppress the
        // homogeneous mixing ~59% final size to ~42%.
        let incidence = model.get_stats().get_cum_incidence() as f64;
        let expected = population as f64 * 0.42;
        assert_relative_eq!(incidence, expected, max_relative = 0.04);
    }
}
