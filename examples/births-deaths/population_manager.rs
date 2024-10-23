use crate::parameters_loader::Parameters;
use ixa::context::Context;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::{ContextPeopleExt, PersonId, PersonProperty};
use ixa::random::define_rng;
use ixa::random::ContextRandomExt;
use ixa::{define_global_property, define_person_property, define_person_property_with_default};
use rand_distr::Uniform;
use serde::Deserialize;
use std::collections::HashMap;

define_rng!(PeopleRng);

static MAX_AGE: f64 = 100.0;
use rand_distr::Exp;
use serde::Serialize;
use std::fmt;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum InfectionStatus {
    S,
    I,
    R,
}

#[derive(Deserialize, Serialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum AgeGroupRisk {
    NewBorn,
    General,
    OldAdult,
}

impl fmt::Display for AgeGroupRisk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

define_global_property!(Foi, HashMap<AgeGroupRisk, f64>);
define_person_property_with_default!(InfectionStatusType, InfectionStatus, InfectionStatus::S);

define_person_property!(Birth, f64);
define_person_property!(Alive, bool);

fn create_new_person(context: &mut Context, birth_time: f64) -> PersonId {
    let person = context.add_person();
    context.initialize_person_property(person, Birth, birth_time);
    context.initialize_person_property(person, Alive, true);
    person
}

fn schedule_birth(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters).clone();
    let _person = create_new_person(context, context.get_current_time());

    let next_birth_event = context.get_current_time()
        + context.sample_distr(PeopleRng, Exp::new(parameters.birth_rate).unwrap());
    context.add_plan(next_birth_event, move |context| {
        schedule_birth(context);
    });
}

fn attempt_death(context: &mut Context, person_id: PersonId) {
    // Where should we assign all the person properties to be dead and cancel plans? people.rs?
    println!(
        "Attempting to remove {:?} - at time: {:?}",
        person_id,
        context.get_current_time()
    );
    context.set_person_property(person_id, Alive, false);
}

fn schedule_death(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters).clone();

    if let Some(person) = context.sample_person_by_property(Alive, true) {
        attempt_death(context, person);

        let next_death_event = context.get_current_time()
            + context.sample_distr(PeopleRng, Exp::new(parameters.death_rate).unwrap());

        context.add_plan(next_death_event, move |context| {
            schedule_death(context);
        });
    }
    // Cancel all plans
}

pub fn init(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters).clone();

    let foi_map = parameters
        .foi_groups
        .clone()
        .into_iter()
        .map(|x| (x.group_name, x.foi))
        .collect::<HashMap<AgeGroupRisk, f64>>();

    context.set_global_property_value(Foi, foi_map.clone());

    for _ in 0..parameters.population {
        // Define age in days
        let age_days: f64 = context.sample_distr(PeopleRng, Uniform::new(0.0, MAX_AGE)) * 365.0;
        let _person = create_new_person(context, -age_days);
    }

    // Plan for births and deaths
    if parameters.birth_rate > 0.0 {
        context.add_plan(0.0, |context| {
            schedule_birth(context);
        });
    }
    if parameters.death_rate > 0.0 {
        context.add_plan(0.0, |context| {
            schedule_death(context);
        });
    }
}

pub trait ContextPopulationExt {
    fn get_person_age_group(&mut self, person_id: PersonId) -> AgeGroupRisk;
    fn get_person_age(&mut self, person_id: PersonId) -> f64;
    fn get_current_group_population(&mut self, age_group: AgeGroupRisk) -> usize;
    fn sample_person(&mut self, age_group: AgeGroupRisk) -> Option<PersonId>;
    #[allow(dead_code)]
    fn get_population_by_property<T: PersonProperty + 'static>(
        &mut self,
        property: T,
        value: T::Value,
    ) -> usize
    where
        <T as PersonProperty>::Value: PartialEq;
    fn sample_person_by_property<T: PersonProperty + 'static>(
        &mut self,
        property: T,
        value: T::Value,
    ) -> Option<PersonId>
    where
        <T as PersonProperty>::Value: PartialEq;
}

impl ContextPopulationExt for Context {
    fn get_person_age_group(&mut self, person_id: PersonId) -> AgeGroupRisk {
        let current_age = self.get_person_age(person_id);
        if current_age <= 1.0 {
            AgeGroupRisk::NewBorn
        } else if current_age <= 65.0 {
            AgeGroupRisk::General
        } else {
            AgeGroupRisk::OldAdult
        }
    }
    fn get_person_age(&mut self, person_id: PersonId) -> f64 {
        let birth_time = self.get_person_property(person_id, Birth);
        (self.get_current_time() - birth_time) / 365.0
    }
    fn get_current_group_population(&mut self, age_group: AgeGroupRisk) -> usize {
        // loop through all population
        // filter those who are alive
        // filter those with age group risk = age_group
        let mut current_population = 0;
        for i in 0..self.get_current_population() {
            let person_id = self.get_person_id(i);
            if self.get_person_property(person_id, Alive)
                && self.get_person_age_group(person_id) == age_group
            {
                current_population += 1;
            }
        }
        current_population
    }

    fn sample_person(&mut self, age_group: AgeGroupRisk) -> Option<PersonId> {
        let mut people_vec = Vec::<PersonId>::new();
        for i in 0..self.get_current_population() {
            let person_id = self.get_person_id(i);
            if self.get_person_property(person_id, Alive)
                && self.get_person_age_group(person_id) == age_group
            {
                people_vec.push(person_id);
            }
        }
        if people_vec.is_empty() {
            None
        } else {
            Some(people_vec[self.sample_range(PeopleRng, 0..people_vec.len())])
        }
    }
    fn get_population_by_property<T: PersonProperty + 'static>(
        &mut self,
        property: T,
        value: T::Value,
    ) -> usize
    where
        <T as PersonProperty>::Value: PartialEq,
    {
        let mut population_counter = 0;
        for i in 0..self.get_current_population() {
            let person_id = self.get_person_id(i);
            if self.get_person_property(person_id, property) == value {
                population_counter += 1;
            }
        }
        population_counter
    }

    fn sample_person_by_property<T: PersonProperty + 'static>(
        &mut self,
        property: T,
        value: T::Value,
    ) -> Option<PersonId>
    where
        <T as PersonProperty>::Value: PartialEq,
    {
        let mut people_vec = Vec::<PersonId>::new();
        for i in 0..self.get_current_population() {
            let person_id = self.get_person_id(i);
            if self.get_person_property(person_id, property) == value {
                people_vec.push(person_id);
            }
        }
        if people_vec.is_empty() {
            None
        } else {
            Some(people_vec[self.sample_range(PeopleRng, 0..people_vec.len())])
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::parameters_loader::{FoiAgeGroups, ParametersValues};
    use ixa::context::{self, Context};

    #[test]
    fn test_birth_death() {
        let mut context = Context::new();

        let person = create_new_person(&mut context, -10.0);
        context.add_plan(10.0, |context| {
            _ = create_new_person(context, 10.0);
        });
        context.add_plan(20.0, move |context| {
            attempt_death(context, person);
        });
        context.add_plan(11.0, |context| {
            let pop = context.get_population_by_property(Alive, true);
            assert_eq!(pop, 2);
        });
        context.add_plan(21.0, |context| {
            let pop = context.get_population_by_property(Alive, true);
            assert_eq!(pop, 1);
        });
        context.execute();
        let population = context.get_current_population();

        // Even if these people have died during simulation, we can still get their properties
        let birth_day_0 = context.get_person_property(context.get_person_id(0), Birth);
        let birth_day_1 = context.get_person_property(context.get_person_id(1), Birth);
        assert_eq!(birth_day_0, -10.0);
        assert_eq!(birth_day_1, 10.0);

        // Ixa population contains all individuals ever created
        assert_eq!(population, 2);
    }

    #[test]
    #[should_panic]
    fn test_null_birth_rates() {
        let p_values = ParametersValues {
            population: 10,
            max_time: 10.0,
            seed: 42,
            birth_rate: 0.0,
            death_rate: 0.1,
            foi_groups: Vec::<FoiAgeGroups>::new(),
            infection_duration: 5.0,
            output_file: ".".to_string(),
            demographic_output_file: ".".to_string(),
        };

        let mut context = Context::new();
        context.set_global_property_value(Parameters, p_values.clone());
        context.init_random(p_values.seed);

        schedule_birth(&mut context);
    }

    #[test]
    #[should_panic]
    fn test_null_death_rates() {
        let p_values = ParametersValues {
            population: 10,
            max_time: 10.0,
            seed: 42,
            birth_rate: 0.1,
            death_rate: 0.0,
            foi_groups: Vec::<FoiAgeGroups>::new(),
            infection_duration: 5.0,
            output_file: ".".to_string(),
            demographic_output_file: ".".to_string(),
        };

        let mut context = Context::new();
        context.set_global_property_value(Parameters, p_values.clone());
        context.init_random(p_values.seed);
        let _person = create_new_person(&mut context, -10.0);
        schedule_death(&mut context);
    }

    #[test]
    fn test_current_age() {
        let mut context = Context::new();
        let person = create_new_person(&mut context, -5.0);
        context.add_plan(30.0, move |context| {
            assert_eq!(context.get_person_age(person), 35.0 / 365.0);
        });
        context.execute();
    }

    #[test]
    fn test_sample_person_group() {
        let mut context = Context::new();
        let age_vec = vec![0.5, 5.0, 62.0, 80.0];
        let years = 5.0;
        let age_groups = vec![
            AgeGroupRisk::NewBorn,
            AgeGroupRisk::General,
            AgeGroupRisk::General,
            AgeGroupRisk::OldAdult,
        ];
        for age in &age_vec {
            let birth = age * (-365.0);
            let _person = create_new_person(&mut context, birth);
        }

        for p in 0..context.get_current_population() {
            let person = context.get_person_id(p);
            let age_group = age_groups[p];
            assert_eq!(age_group, context.get_person_age_group(person));
        }

        // Plan to check in 5 years
        let future_age_groups = vec![
            AgeGroupRisk::General,
            AgeGroupRisk::General,
            AgeGroupRisk::OldAdult,
            AgeGroupRisk::OldAdult,
        ];
        context.add_plan(years * 365.0, move |context| {
            for p in 0..context.get_current_population() {
                let person = context.get_person_id(p);
                let age_group = future_age_groups[p];
                assert_eq!(age_group, context.get_person_age_group(person));
            }
        });
        context.execute();
    }
}
