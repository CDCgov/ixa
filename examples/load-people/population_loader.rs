use std::path::Path;

use crate::vaccine::{ContextVaccineExt, VaccineEfficacy, VaccineType};
use ixa::context::Context;
use ixa::define_person_property;
use ixa::people::{ContextPeopleExt, PersonId};
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

fn create_person_from_record(context: &mut Context, record: &PeopleRecord) -> PersonId {
    let person = context.add_person();
    context.set_person_property(person, Age, record.age);
    context.set_person_property(person, RiskCategoryType, record.risk_category);

    // Set vaccine type and efficacy based on risk category
    let (t, e) = context.get_vaccine_props(record.risk_category);
    context.set_person_property(person, VaccineType, t);
    context.set_person_property(person, VaccineEfficacy, e);

    person
}

pub fn init(context: &mut Context) {
    // Load csv and deserialize records
    let current_dir = Path::new(file!()).parent().unwrap();
    let mut reader = csv::Reader::from_path(current_dir.join("people.csv")).unwrap();

    for result in reader.deserialize() {
        let record: PeopleRecord = result.expect("Failed to parse record");
        create_person_from_record(context, &record);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        population_loader::Age,
        sir::DiseaseStatusType,
        vaccine::{VaccineDoses, VaccineEfficacy, VaccineType, VaccineTypeValue},
    };
    use ixa::{context::Context, random::ContextRandomExt};

    const EXPECTED_ROWS: usize = 5;

    #[test]
    fn test_init_expected_rows() {
        let mut context = Context::new();
        context.init_random(42);
        init(&mut context);
        assert_eq!(context.get_current_population(), EXPECTED_ROWS);
    }

    fn test_init_access_properties() {
        let mut context = Context::new();
        context.init_random(42);
        init(&mut context);
        assert_eq!(context.get_current_population(), EXPECTED_ROWS);
        let person = PersonId { id: 0 };
        context.get_person_property(person, VaccineDoses);
        context.get_person_property(person, VaccineEfficacy);
        context.get_person_property(person, VaccineType);
    }

    fn test_creation_event() {
        let mut context = Context::new();
        context.init_random(42);
        context.subscribe_to_event(|context, event: PersonCreatedEvent| {
            let person = event.person_id;
            context.get_person_property(person, Age);
            context.get_person_property(person, VaccineDoses);
            context.get_person_property(person, VaccineType);
            context.get_person_property(person, VaccineEfficacy);
        });
        init(&mut context);
    }
}
