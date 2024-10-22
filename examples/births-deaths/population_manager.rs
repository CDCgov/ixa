use ixa::context::Context;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::{ContextPeopleExt, PersonId};
use ixa::random::ContextRandomExt;
use ixa::{define_derived_person_property, define_global_property, define_person_property, define_person_property_with_default};
use std::collections::HashMap;
use serde::Deserialize;
use crate::parameters_loader::Parameters;
use ixa::random::define_rng;
define_rng!(PeopleRng);
static MAX_AGE: u8 = 100;
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

define_person_property!(Age, u8);
define_person_property!(Alive, bool);
define_derived_person_property!(
    AgeGroupType,
    AgeGroupRisk,
    [Age],
    |age| {
        if age <= 1 {
            AgeGroupRisk::NewBorn
        } else if age <= 65 {
            AgeGroupRisk::General
        } else {
            AgeGroupRisk::OldAdult
        }
    }
);

fn schedule_birth(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters).clone();
    
    let person = context.add_person();
    context.initialize_person_property(person, Age, 0);    
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
        let age = context.sample_range(PeopleRng, 0..(MAX_AGE));
        context.initialize_person_property(person, Age, age);
        context.initialize_person_property(person, Alive, true);
    }

    // Plan for births and deaths
    context.add_plan(0.0, |context| {schedule_birth(context)});
}
