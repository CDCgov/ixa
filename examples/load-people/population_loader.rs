use std::path::PathBuf;

use ixa::prelude::*;
use serde::Deserialize;
use serde_derive::Serialize;

use crate::vaccine::ContextVaccineExt;
use crate::{Person, PersonId};

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum RiskCategory {
    High,
    Low,
}
impl_property!(RiskCategory, Person);

#[derive(Deserialize, Debug)]
struct PeopleRecord {
    age: u8,
    risk_category: RiskCategory,
}

define_property!(struct Age(pub u8), Person);

fn create_person_from_record(context: &mut Context, record: &PeopleRecord) -> PersonId {
    let age = Age(record.age);
    let risk_category = record.risk_category;
    let (vaccine_type, vaccine_efficacy) = context.get_vaccine_props(risk_category);
    let doses = context.sample_vaccine_doses(age);
    context
        .add_entity((age, risk_category, vaccine_type, vaccine_efficacy, doses))
        .unwrap()
}

pub fn init(context: &mut Context) {
    // Load csv and deserialize records
    let data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("load-people")
        .join("people.csv");
    let mut reader = csv::Reader::from_path(data_path).unwrap();

    for result in reader.deserialize() {
        let record: PeopleRecord = result.expect("Failed to parse record");
        create_person_from_record(context, &record);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use ixa::prelude::*;

    use super::*;
    use crate::vaccine::{VaccineDoses, VaccineEfficacy, VaccineType};

    const EXPECTED_ROWS: usize = 5;

    #[test]
    fn test_init_expected_rows() {
        let mut context = Context::new();
        context.init_random(42);
        init(&mut context);
        assert_eq!(context.get_entity_count::<Person>(), EXPECTED_ROWS);
    }

    #[test]
    fn test_creation_event_access_properties() {
        let flag = Rc::new(RefCell::new(false));

        // Define expected computed values for each person. The value for dosage will change for
        // any change in the deterministic RNG.
        let expected_computed = vec![
            // (age, risk_category, vaccine_type, efficacy, doses)
            (20, RiskCategory::Low, VaccineType::B, 0.8, 1),
            (80, RiskCategory::High, VaccineType::A, 0.9, 2),
        ];

        let mut context = Context::new();
        context.init_random(42);

        // Subscribe to property change event
        let flag_clone = Rc::clone(&flag);
        context.subscribe_to_event(
            move |_context, _event: PropertyChangeEvent<Person, VaccineEfficacy>| {
                *flag_clone.borrow_mut() = true;
            },
        );

        let counter = Rc::new(RefCell::new(0));
        let expected_computed = Rc::new(expected_computed);

        context.subscribe_to_event({
            let counter = Rc::clone(&counter);
            let expected_computed = Rc::clone(&expected_computed);

            move |context, event: EntityCreatedEvent<Person>| {
                let person = event.entity_id;
                let current_count = *counter.borrow();
                let (age, risk_category, vaccine_type, efficacy, doses) =
                    expected_computed[current_count];

                let Age(actual_age) = context.get_property(person);
                assert_eq!(actual_age, age);

                let actual_risk_category: RiskCategory = context.get_property(person);
                assert_eq!(actual_risk_category, risk_category);

                let actual_vaccine_type: VaccineType = context.get_property(person);
                assert_eq!(actual_vaccine_type, vaccine_type);

                let VaccineEfficacy(actual_efficacy) = context.get_property(person);
                assert_eq!(actual_efficacy, efficacy);

                // This assert will break for any change that affects the deterministic hasher.
                let VaccineDoses(actual_doses) = context.get_property(person);
                assert_eq!(actual_doses, doses);

                *counter.borrow_mut() += 1;
            }
        });

        // Create people from records based on expected values
        for &(age, risk_category, _, _, _) in expected_computed.iter() {
            create_person_from_record(&mut context, &PeopleRecord { age, risk_category });
        }

        // Execute the context
        context.execute();

        // Make sure PropertyChangeEvent didn't fire
        assert!(!*flag.borrow());
    }
}
