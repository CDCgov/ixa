use std::path::Path;

use crate::vaccine::{ContextVaccineExt, VaccineEfficacy, VaccineType};
use ixa::prelude::*;
use ixa::PersonId;
use serde::Deserialize;
use serde_derive::Serialize;

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum RiskCategoryValue {
    High,
    Low,
}

#[derive(Deserialize, Debug)]
struct PeopleRecord {
    age: u8,
    risk_category: RiskCategoryValue,
}

define_person_property!(Age, u8);
define_person_property!(RiskCategory, RiskCategoryValue);

fn create_person_from_record(context: &mut Context, record: &PeopleRecord) -> PersonId {
    let (t, e) = context.get_vaccine_props(record.risk_category);
    context
        .add_person((
            (Age, record.age),
            (RiskCategory, record.risk_category),
            (VaccineType, t),
            (VaccineEfficacy, e),
        ))
        .unwrap()
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
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use crate::{
        population_loader::Age,
        vaccine::{VaccineDoses, VaccineEfficacy, VaccineType, VaccineTypeValue},
    };
    use ixa::prelude::*;

    const EXPECTED_ROWS: usize = 5;

    #[test]
    fn test_init_expected_rows() {
        let mut context = Context::new();
        context.init_random(42);
        init(&mut context);
        assert_eq!(context.get_current_population(), EXPECTED_ROWS);
    }

    #[test]
    fn test_creation_event_access_properties() {
        let flag = Rc::new(RefCell::new(false));

        // Define expected computed values for each person. The value for dosage will change for
        // any change in the deterministic RNG.
        let expected_computed = vec![
            // (age, risk_category, vaccine_type, efficacy, doses)
            (20, RiskCategoryValue::Low, VaccineTypeValue::B, 0.8, 3),
            (80, RiskCategoryValue::High, VaccineTypeValue::A, 0.9, 1),
        ];

        let mut context = Context::new();
        context.init_random(42);

        // Subscribe to person property change event
        let flag_clone = Rc::clone(&flag);
        context.subscribe_to_event(
            move |_context, _event: PersonPropertyChangeEvent<VaccineEfficacy>| {
                *flag_clone.borrow_mut() = true;
            },
        );

        let counter = Rc::new(RefCell::new(0));
        let expected_computed = Rc::new(expected_computed);

        context.subscribe_to_event({
            let counter = Rc::clone(&counter);
            let expected_computed = Rc::clone(&expected_computed);

            move |context, event: PersonCreatedEvent| {
                let person = event.person_id;
                let current_count = *counter.borrow();
                let (age, risk_category, vaccine_type, efficacy, doses) =
                    expected_computed[current_count];

                assert_eq!(context.get_person_property(person, Age), age);
                assert_eq!(
                    context.get_person_property(person, RiskCategory),
                    risk_category
                );
                assert_eq!(
                    context.get_person_property(person, VaccineType),
                    vaccine_type
                );
                assert_eq!(
                    context.get_person_property(person, VaccineEfficacy),
                    efficacy
                );
                // This assert will break for any change that affects the deterministic hasher.
                assert_eq!(context.get_person_property(person, VaccineDoses), doses);

                *counter.borrow_mut() += 1;
            }
        });

        // Create people from records based on expected values
        for &(age, risk_category, _, _, _) in expected_computed.iter() {
            create_person_from_record(&mut context, &PeopleRecord { age, risk_category });
        }

        // Execute the context
        context.execute();

        // Make sure PersonPropertyChangeEvent didn't fire
        assert!(!*flag.borrow());
    }
}
