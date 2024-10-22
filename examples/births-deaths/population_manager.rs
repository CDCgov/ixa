use ixa::context::Context;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::{ContextPeopleExt, PersonId};
use ixa::random::ContextRandomExt;
use ixa::{define_data_plugin, define_derived_person_property, define_global_property, define_person_property, define_person_property_with_default};
use rand_distr::Uniform;
use std::collections::HashMap;
use serde::Deserialize;
use crate::parameters_loader::Parameters;
use ixa::random::define_rng;
use ixa::error::IxaError;
define_rng!(PeopleRng);

static MAX_AGE: f64 = 100.0;
use std::fmt;
use rand_distr::Exp;
use serde::Serialize;

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
    OldAdult
}

impl fmt::Display for AgeGroupRisk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

define_global_property!(Foi, HashMap<AgeGroupRisk, f64>);
define_person_property_with_default!(
    InfectionStatusType,
    InfectionStatus,
    InfectionStatus::S
);

define_person_property!(Birth, f64);
define_person_property!(Alive, bool);

fn schedule_birth(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters).clone();
    
    let person = context.add_person();
    context.initialize_person_property(person, Birth, context.get_current_time());    
    context.initialize_person_property(person, Alive, true);

    let next_birth_event = context.get_current_time() +
        context.sample_distr(PeopleRng, Exp::new(parameters.birth_rate).unwrap());
    context.add_plan(next_birth_event,
        move |context| {
            schedule_birth(context);
    });
}

// fn schedule_death(context: &mut Context) {
//     let parameters = context.get_global_property_value(Parameters).clone();
//     let id = context.sample_range(PeopleRng, 0..context.get_current_population());
//     let person = context.get_person_id(id);

//     context.remove_person(person);
//     // Where should we assign all the person properties to be dead and cancel plans? people.rs?
//     context.set_person_property(person, Alive, false);
    
//     let next_death_event = context.get_current_time() +
//         context.sample_distr(PeopleRng, Exp::new(parameters.death_rate).unwrap());

//     println!("Attempting to remove {:?} - Next death event: {:?}", person, next_death_event);
    
//     context.add_plan(next_death_event,
//         move |context| {
//             schedule_death(context);
//     });
// }
// struct PopulationData {
    
// }
// define_data_plugin!(PopulationPlugin,
    
//     );

pub fn init(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters).clone();

    let foi_map = parameters
        .foi_groups
        .clone()
        .into_iter()
        .map(|x| (x.group_name,x.foi))
        .collect::<HashMap<AgeGroupRisk, f64>>();
    
    
    context.set_global_property_value(Foi, foi_map.clone());
    
    for _ in 0..parameters.population {
        let person = context.add_person();
        // Define age in days
        let age_days:f64 = context.sample_distr(PeopleRng,
            Uniform::new(0.0, MAX_AGE)) * 365.0;
        context.initialize_person_property(person, Birth, -age_days);
        context.initialize_person_property(person, Alive, true);
    }

    // Plan for births and deaths
    if parameters.birth_rate > 0.0 {
        context.add_plan(0.0, |context| {schedule_birth(context)});
    }
}


pub trait ContextPopulationExt {
    fn get_person_age_group(&mut self, person_id: PersonId) -> AgeGroupRisk;
    fn get_person_age(&mut self, person_id: PersonId) -> f64;        
    fn get_current_group_population(&mut self, age_group:AgeGroupRisk) -> usize;
    fn sample_person(&mut self, age_group: AgeGroupRisk) -> Option<PersonId>;
}

impl ContextPopulationExt for Context {
    fn get_person_age_group(&mut self, person_id: PersonId) -> AgeGroupRisk {
        let current_age = self.get_person_age(person_id);
        if current_age <= 1.0  {
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
    fn get_current_group_population(&mut self, age_group:AgeGroupRisk) -> usize {
        // loop through all population
        // filter those who are alive
        // filter those with age group risk = age_group
        let mut current_population = 0;
        for i in 0..self.get_current_population() {
            let person_id = self.get_person_id(i);
            if self.get_person_property(person_id, Alive) == true {
                if self.get_person_age_group(person_id) == age_group {
                    current_population += 1;
                }
            }
        }
        current_population
    }
    fn sample_person(&mut self, age_group: AgeGroupRisk) -> Option<PersonId> {
        let mut people_vec = Vec::<PersonId>::new();
        for i in 0..self.get_current_population() {
            let person_id = self.get_person_id(i);
            if self.get_person_property(person_id, Alive) == true {
                if self.get_person_age_group(person_id) == age_group {
                    people_vec.push(person_id);
                }
            }
        }
        if people_vec.len() == 0 {
            None
        } else {
            Some(people_vec[self.sample_range(PeopleRng, 0..people_vec.len())])
        }

    }
}
