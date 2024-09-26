use crate::population_loader::{Age, RiskCategory};
use std::hash::{Hash, Hasher};

use ixa::{
    context::Context, define_person_property, define_rng, people::ContextPeopleExt,
    random::ContextRandomExt,
};

define_rng!(VaccineRng);

#[allow(clippy::module_name_repetitions)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum VaccineTypeValue {
    A,
    B,
}
define_person_property!(VaccineType, VaccineTypeValue);
#[derive(Copy, Clone, Debug)]
pub struct VaccineEfficacyValue(pub f64);

impl Hash for VaccineEfficacyValue {
    fn hash<H: Hasher>(&self, _state: &mut H) {
        // TODO(cym4@cdc.gov): Actually implement this.
        panic!("Unimplemented")
    }
}
define_person_property!(VaccineEfficacy, VaccineEfficacyValue);
define_person_property!(VaccineDoses, u8, |context: &Context, person_id| {
    let age = context.get_person_property(person_id, Age);
    if age > 10 {
        context.sample_range(VaccineRng, 0..5)
    } else {
        0
    }
});

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
