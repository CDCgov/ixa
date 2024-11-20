use crate::parameters_loader::Parameters;
use ixa::{
    context::Context,
    define_derived_property, define_person_property, define_person_property_with_default,
    global_properties::ContextGlobalPropertiesExt,
    people::{ContextPeopleExt, PersonId, PersonProperty},
    random::define_rng,
};

use strum_macros::EnumIter;

use std::path::Path;
use serde::Deserialize;

define_rng!(PeopleRng);

use serde::Serialize;
use std::fmt;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum InfectionStatus {
    S,
    I,
    R,
}

#[derive(Deserialize, Serialize, Copy, Clone, PartialEq, Eq, Debug, Hash, EnumIter)]
pub enum AgeGroupRisk {
    NewBorn,
    YoungChild,
    General,
    OldAdult,
    Elderly,
}

impl fmt::Display for AgeGroupRisk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Deserialize, Debug)]
pub struct PeopleRecord {
    age: u8,
    homeId: usize,
}


define_person_property!(Age, u8);
define_person_property!(HomeId, usize);
define_person_property_with_default!(Alive, bool, true);

define_derived_property!(VaccineAgeGroup, AgeGroupRisk, [Age], |age| {
    if age <= 1 {
        AgeGroupRisk::NewBorn
    } else if age <= 2 {
        AgeGroupRisk::YoungChild
    } else if age < 60 {
        AgeGroupRisk::General
    } else if age < 75 {
        AgeGroupRisk::OldAdult
    } else {
        AgeGroupRisk::Elderly
    }
});

define_derived_property!(CensusTract, usize, [HomeId], |home_id| {
    home_id / 10000
});

pub fn create_new_person(context: &mut Context, person_record: &PeopleRecord) -> PersonId {
    let person = context
        .add_person( ((Age, person_record.age),
        (HomeId, person_record.homeId))).unwrap();
    person
}

pub fn init(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters)
        .unwrap()
        .clone();

    let record_dir = Path::new(file!()).parent().unwrap();
    let mut reader = csv::Reader::from_path(record_dir.join(parameters.synth_population_file)).unwrap();

    for result in reader.deserialize() {
        let record: PeopleRecord = result.expect("Failed to parse record");
        create_new_person(context, &record);
    }

}

pub trait ContextPopulationExt {
    fn get_population_by_properties<T: PersonProperty + 'static, U: PersonProperty + 'static>(
        &mut self,
        property_a: T,
        value_a: T::Value,
        property_b: U,
        value_b: U::Value,
    ) -> usize
    where
        <T as PersonProperty>::Value: PartialEq,
        <U as PersonProperty>::Value: PartialEq;
}

impl ContextPopulationExt for Context {
    fn get_population_by_properties<T: PersonProperty + 'static, U: PersonProperty + 'static>(
        &mut self,
        property_a: T,
        value_a: T::Value,
        property_b: U,
        value_b: U::Value,
    ) -> usize
    where
        <T as PersonProperty>::Value: PartialEq,
        <U as PersonProperty>::Value: PartialEq,
    {
        let mut population_counter = 0;
        for i in 0..self.get_current_population() {
            let person_id = self.get_person_id(i);

            if self.get_person_property(person_id, Alive) {
                if self.get_person_property(person_id, property_a) == value_a
                    && self.get_person_property(person_id, property_b) == value_b {
                    population_counter += 1;
                }
            }
        }

        population_counter
    }

}
