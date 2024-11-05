use crate::parameters_loader::Parameters;
use ixa::{
    context::Context,
    define_derived_property, define_person_property, define_person_property_with_default,
    global_properties::ContextGlobalPropertiesExt,
    people::{ContextPeopleExt, PersonId, PersonProperty},
    random::{define_rng, ContextRandomExt},
};
use serde::Deserialize;

define_rng!(PeopleRng);

static MAX_AGE: u8 = 100;
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

pub struct PersonRecord {
    age: u8,
    home_id: usize,
    school_id: usize
}


define_person_property!(Age, u8);
define_person_property_with_default!(Alive, bool, true);

define_derived_property!(VaccineAgeGroup, AgeGroupRisk, [Age], |age| {
    if age <= 1 {
        AgeGroupRisk::NewBorn
    } else if age <= 65 {
        AgeGroupRisk::General
    } else {
        AgeGroupRisk::OldAdult
    }
});


pub fn init(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters)
        .unwrap()
        .clone();

    for _ in 0..parameters.population {
        let p_record = PersonRecord {
            age: context.sample_range(PeopleRng, 0..MAX_AGE),
            home_id: 0,
            school_id: 0,
        };
        let person = context.create_new_person(p_record);        
    }
}

pub trait ContextPopulationExt {
    fn create_new_person(&mut self, person_record: PersonRecord) -> PersonId;
}

impl ContextPopulationExt for Context {
    fn create_new_person(&mut self, person_record: PersonRecord) -> PersonId{
        let person = self.add_person();
        self.initialize_person_property(person, Age, person_record.age);
        person
    }
}

