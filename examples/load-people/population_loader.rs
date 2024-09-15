use crate::vaccine::{ContextVaccineExt, VaccineEfficacy, VaccineType};
use ixa::context::Context;
use ixa::define_person_property;
use ixa::people::ContextPeopleExt;
use serde::Deserialize;

#[derive(Deserialize, Copy, Clone, PartialEq, Eq, Debug)]
pub enum RiskCategory {
    High,
    Low,
}

#[derive(Deserialize, Debug)]
struct PeopleRecord {
    age: u8,
    risk_category: RiskCategory,
}

define_person_property!(Age, u8);
define_person_property!(RiskCategoryType, RiskCategory);

pub fn init(context: &mut Context) {
    // Load csv and deserialize records
    let mut reader = csv::Reader::from_path("./examples/load-people/people.csv").unwrap();

    for result in reader.deserialize() {
        let record: PeopleRecord = result.expect("Failed to parse record");

        let person = context.add_person();
        context.set_person_property(person, Age, record.age);
        context.set_person_property(person, RiskCategoryType, record.risk_category);

        // Set vaccine type and efficacy based on risk category
        let (vaccine_type, vaccine_efficacy) =
            context.get_vaccine_type_and_efficacy(record.risk_category);
        context.set_person_property(person, VaccineType, vaccine_type);
        context.set_person_property(person, VaccineEfficacy, vaccine_efficacy);
    }
}
