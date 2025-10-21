use ixa::people::{PersonCreatedEvent, PersonPropertyChangeEvent};
use ixa::prelude::*;

use crate::population_loader::Age;
use crate::sir::DiseaseStatus;
use crate::vaccine::{VaccineDoses, VaccineEfficacy, VaccineType};

pub fn init(context: &mut Context) {
    // This subscribes to the disease status change events
    // Note that no event gets fired when the property is set the first time
    context.subscribe_to_event(
        |_context, event: PersonPropertyChangeEvent<DiseaseStatus>| {
            let person = event.person_id;
            println!(
                "{:?} changed disease status from {:?} to {:?}",
                person, event.previous, event.current,
            );
        },
    );

    // Logs when a person is created
    context.subscribe_to_event(|context, event: PersonCreatedEvent| {
        let person = event.person_id;
        println!(
            "{:?} age: {}, {} vaccine doses, vaccine {:?} ({:?})",
            person,
            context.get_person_property(person, Age),
            context.get_person_property(person, VaccineDoses),
            context.get_person_property(person, VaccineType),
            context.get_person_property(person, VaccineEfficacy)
        );
    });
}
