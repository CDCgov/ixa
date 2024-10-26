use crate::population_loader::{Age, RiskCategory};
use ixa::{
    context::Context, define_person_property, define_rng, people::ContextPeopleExt,
    random::ContextRandomExt,
};

define_rng!(VaccineRng);

#[allow(clippy::module_name_repetitions)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum VaccineTypeValue {
    A,
    B,
}
define_person_property!(VaccineType, VaccineTypeValue, true);
define_person_property!(VaccineEfficacy, f64, true);
define_person_property!(
    VaccineDoses,
    u8,
    true,
    |context: &mut Context, person_id| {
        let age = context.get_person_property(person_id, Age);
        if age > 10 {
            context.sample_range(VaccineRng, 0..5)
        } else {
            0
        }
    }
);

pub trait ContextVaccineExt {
    fn get_vaccine_props(&self, risk: RiskCategory) -> (VaccineTypeValue, f64);
}

impl ContextVaccineExt for Context {
    fn get_vaccine_props(self: &Context, risk: RiskCategory) -> (VaccineTypeValue, f64) {
        if risk == RiskCategory::High {
            (VaccineTypeValue::A, 0.9)
        } else {
            (VaccineTypeValue::B, 0.8)
        }
    }
}
