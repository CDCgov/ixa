use std::hash::Hash;

use crate::population_loader::{Age, RiskCategory};
use ixa::{
    context::Context, define_person_property, define_rng, people::ContextPeopleExt,
    random::ContextRandomExt,
};
use ordered_float::OrderedFloat;

define_rng!(VaccineRng);

#[allow(clippy::module_name_repetitions)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum VaccineTypeValue {
    A,
    B,
}

define_person_property!(VaccineType, VaccineTypeValue);
define_person_property!(VaccineEfficacy, OrderedFloat<f64>);
define_person_property!(VaccineDoses, u8, |context: &Context, person_id| {
    let age = context.get_person_property(person_id, Age);
    if age > 10 {
        context.sample_range(VaccineRng, 0..5)
    } else {
        0
    }
});

pub trait ContextVaccineExt {
    fn get_vaccine_props(&self, risk: RiskCategory) -> (VaccineTypeValue, OrderedFloat<f64>);
}

impl ContextVaccineExt for Context {
    fn get_vaccine_props(
        self: &Context,
        risk: RiskCategory,
    ) -> (VaccineTypeValue, OrderedFloat<f64>) {
        if risk == RiskCategory::High {
            (VaccineTypeValue::A, OrderedFloat(0.9))
        } else {
            (VaccineTypeValue::B, OrderedFloat(0.8))
        }
    }
}
