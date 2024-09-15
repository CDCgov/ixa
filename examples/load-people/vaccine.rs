use crate::population_loader::{Age, RiskCategory};
use ixa::{
    context::Context,
    define_person_property, define_rng,
    people::{ContextPeopleExt, PersonId, PersonProperty},
    random::ContextRandomExt,
};

#[allow(clippy::module_name_repetitions)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum VaccineTypeValue {
    A,
    B,
}
define_person_property!(VaccineType, VaccineTypeValue);
define_person_property!(VaccineEfficacy, f64);

define_rng!(VaccineRng);

#[allow(clippy::module_name_repetitions)]
#[derive(Copy, Clone)]
pub struct VaccineDoses;
impl PersonProperty for VaccineDoses {
    type Value = u8;
    fn initialize(context: &Context, person_id: PersonId) -> Self::Value {
        let age = context.get_person_property(person_id, Age);
        if age > 10 {
            context.sample_range(VaccineRng, 0..5)
        } else {
            0
        }
    }
}

pub trait ContextVaccineExt {
    fn get_vaccine_type_and_efficacy(&self, risk: RiskCategory) -> (VaccineTypeValue, f64);
}

impl ContextVaccineExt for Context {
    fn get_vaccine_type_and_efficacy(
        self: &Context,
        risk: RiskCategory,
    ) -> (VaccineTypeValue, f64) {
        if risk == RiskCategory::High {
            (VaccineTypeValue::A, 0.9)
        } else {
            (VaccineTypeValue::B, 0.8)
        }
    }
}
