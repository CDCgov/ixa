use ixa::prelude::*;

use crate::population_loader::{Age, RiskCategory};
use crate::Person;

define_rng!(VaccineRng);

define_property!(
    enum VaccineType {
        A,
        B,
    },
    Person
);
#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct VaccineEfficacy(pub f64);
impl_property!(VaccineEfficacy, Person);
define_property!(struct VaccineDoses(u8), Person);

pub trait ContextVaccineExt: ContextRandomExt {
    fn get_vaccine_props(&self, risk: RiskCategory) -> (VaccineType, VaccineEfficacy) {
        if risk == RiskCategory::High {
            (VaccineType::A, VaccineEfficacy(0.9))
        } else {
            (VaccineType::B, VaccineEfficacy(0.8))
        }
    }

    fn sample_vaccine_doses(&self, age: Age) -> VaccineDoses {
        if age.0 > 10 {
            VaccineDoses(self.sample_range(VaccineRng, 0..5))
        } else {
            VaccineDoses(0)
        }
    }
}
impl ContextVaccineExt for Context {}
