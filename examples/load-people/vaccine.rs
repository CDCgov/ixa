use ixa::{
    context::Context,
    define_rng,
    people::{ContextPeopleExt, PersonId, PersonProperty},
    random::ContextRandomExt,
};

use crate::population_loader::Age;
define_rng!(VaccineRng);

#[allow(clippy::module_name_repetitions)]
#[derive(Copy, Clone)]
pub struct VaccineDoses;
impl PersonProperty for VaccineDoses {
    type Value = u8;
    fn initialize(context: &Context, person_id: PersonId) -> Option<Self::Value> {
        let age = context.get_person_property(person_id, Age);
        Some(context.get_random_vaccine_doses(age))
    }
}

pub trait ContextVaccineExt {
    fn get_random_vaccine_doses(&self, age: u8) -> u8;
}

impl ContextVaccineExt for Context {
    fn get_random_vaccine_doses(self: &Context, age: u8) -> u8 {
        if age > 10 {
            self.sample_range(VaccineRng, 0..5)
        } else {
            0
        }
    }
}
