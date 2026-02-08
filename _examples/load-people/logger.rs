use ixa::prelude::*;

use crate::population_loader::Age;
use crate::sir::DiseaseStatus;
use crate::vaccine::{VaccineDoses, VaccineEfficacy, VaccineType};
use crate::Person;

pub fn init(context: &mut Context) {
    // This subscribes to the disease status change events
    // Note that no event gets fired when the property is set the first time
    context.subscribe_to_event(
        |_context, event: PropertyChangeEvent<Person, DiseaseStatus>| {
            let person = event.entity_id;
            println!(
                "{:?} changed disease status from {:?} to {:?}",
                person, event.previous, event.current,
            );
        },
    );

    // Logs when a person is created
    context.subscribe_to_event(|context, event: EntityCreatedEvent<Person>| {
        let person = event.entity_id;
        let Age(age) = context.get_property(person);
        let VaccineDoses(doses) = context.get_property(person);
        let vaccine_type: VaccineType = context.get_property(person);
        let VaccineEfficacy(efficacy) = context.get_property(person);
        println!(
            "{:?} age: {}, {} vaccine doses, vaccine {:?} ({:?})",
            person, age, doses, vaccine_type, efficacy
        );
    });
}
