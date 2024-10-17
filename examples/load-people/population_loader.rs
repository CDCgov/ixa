use std::path::Path;

use crate::vaccine::{ContextVaccineExt, VaccineEfficacy, VaccineType};
use ixa::context::Context;
use ixa::people::{ContextPeopleExt, PersonId};
use ixa::{define_derived_person_property, define_person_property};
use serde::Deserialize;

#[derive(Deserialize, Copy, Clone, PartialEq, Eq, Debug)]
pub enum RiskCategory {
    High,
    Low,
}

#[derive(Deserialize, Debug)]
struct PeopleRecord {
    age: u8,
    underlying_conditions: u8,
}

define_person_property!(Age, u8);
define_person_property!(UnderlyingConditions, u8);
define_derived_person_property!(
    RiskCategoryType,
    RiskCategory,
    [Age, UnderlyingConditions],
    |age, underlying_conditions| {
        if underlying_conditions >= 3 || age >= 65 {
            RiskCategory::High
        } else {
            RiskCategory::Low
        }
    }
);

fn create_person_from_record(context: &mut Context, record: &PeopleRecord) -> PersonId {
    let person = context.add_person();
    context.initialize_person_property(person, Age, record.age);
    context.initialize_person_property(person, UnderlyingConditions, record.underlying_conditions);

    // Set vaccine type and efficacy based on risk category
    let (t, e) = context.get_vaccine_props(context.get_person_property(person, RiskCategoryType));
    context.initialize_person_property(person, VaccineType, t);
    context.initialize_person_property(person, VaccineEfficacy, e);

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
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use crate::{
        population_loader::Age,
        vaccine::{VaccineDoses, VaccineEfficacy, VaccineType, VaccineTypeValue},
    };
    use ixa::{
        context::Context,
        people::{PersonCreatedEvent, PersonPropertyChangeEvent},
        random::ContextRandomExt,
    };

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

        // Define expected computed values for each person
        let expected_computed = vec![
            (20, 1, RiskCategory::Low, VaccineTypeValue::B, 0.8, 1),
            (80, 3, RiskCategory::High, VaccineTypeValue::A, 0.9, 2),
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
                let (age, underlying_conditions, risk_category, vaccine_type, efficacy, doses) =
                    expected_computed[current_count];

                assert_eq!(context.get_person_property(person, Age), age);
                assert_eq!(
                    context.get_person_property(person, UnderlyingConditions),
                    underlying_conditions
                );
                assert_eq!(
                    context.get_person_property(person, RiskCategoryType),
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
                assert_eq!(context.get_person_property(person, VaccineDoses), doses);

                *counter.borrow_mut() += 1;
            }
        });

        // Create people from records based on expected values
        for &(age, underlying_conditions, _, _, _, _) in expected_computed.iter() {
            create_person_from_record(
                &mut context,
                &PeopleRecord {
                    age,
                    underlying_conditions,
                },
            );
        }

        // Execute the context
        context.execute();

        // Make sure PersonPropertyChangeEvent didn't fire
        assert!(!*flag.borrow());
    }
}
