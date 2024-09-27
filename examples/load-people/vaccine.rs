use std::hash::{Hash, Hasher};

use crate::population_loader::{Age, RiskCategory};
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

#[allow(clippy::module_name_repetitions)]
#[derive(Copy, Clone, Debug)]
pub struct VaccineEfficacyValue(pub f64);

impl Hash for VaccineEfficacyValue {
    fn hash<H: Hasher>(&self, _state: &mut H) {
        // TODO(ryl8@cdc.gov): Hashing for floats?
        panic!("Unimplemented")
    }
}
define_person_property!(VaccineType, VaccineTypeValue);
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
    fn get_vaccine_props(&self, risk: RiskCategory) -> (VaccineTypeValue, VaccineEfficacyValue);
}

impl ContextVaccineExt for Context {
    fn get_vaccine_props(
        self: &Context,
        risk: RiskCategory,
    ) -> (VaccineTypeValue, VaccineEfficacyValue) {
        if risk == RiskCategory::High {
            (VaccineTypeValue::A, VaccineEfficacyValue(0.9))
        } else {
            (VaccineTypeValue::B, VaccineEfficacyValue(0.8))
        }
    }
}
