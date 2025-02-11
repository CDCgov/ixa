use crate::population_loader::{Age, RiskCategoryValue};
use ixa::{
    context::Context, define_person_property, define_rng, people::ContextPeopleExt,
    random::ContextRandomExt,
};
use serde_derive::Serialize;

define_rng!(VaccineRng);

#[allow(clippy::module_name_repetitions)]
#[derive(Serialize, Copy, Clone, PartialEq, Eq, Debug)]
pub enum VaccineTypeValue {
    A,
    B,
}

define_person_property!(VaccineType, VaccineTypeValue);
define_person_property!(VaccineEfficacy, f64);
define_person_property!(VaccineDoses, u8, |context: &Context, person_id| {
    let age = context.get_person_property(person_id, Age);
    if age > 10 {
        context.sample_range(VaccineRng, 0..5)
    } else {
        0
    }
});

pub trait ContextVaccineExt {
    fn get_vaccine_props(&self, risk: RiskCategoryValue) -> (VaccineTypeValue, f64);
}

impl ContextVaccineExt for Context {
    fn get_vaccine_props(self: &Context, risk: RiskCategoryValue) -> (VaccineTypeValue, f64) {
        if risk == RiskCategoryValue::High {
            (VaccineTypeValue::A, 0.9)
        } else {
            (VaccineTypeValue::B, 0.8)
        }
    }
}
