use crate::{
    population_loader::Age,
    sir::DiseaseStatusType,
    vaccine::{VaccineDoses, VaccineEfficacy, VaccineType},
};
use ixa::{
    context::Context,
    people::{ContextPeopleExt, PersonCreatedEvent, PersonPropertyChangeEvent},
};

pub fn init(context: &mut Context) {
    // This subscribes to the disease status change events
    // Note that no event get fired when the property is set the first time
    context.subscribe_to_event(
        |_context, event: PersonPropertyChangeEvent<DiseaseStatusType>| {
            let person = event.person_id;
            println!(
                "Person {} changed disease status from {:?} to {:?}",
                person.id, event.previous, event.current,
            );
        },
    );

    // Logs when a person is created
    context.subscribe_to_event(|context, event: PersonCreatedEvent| {
        let person = event.person_id;
        println!(
            "Person {} age: {}, {} vaccine doses, vaccine {:?} ({})",
            person.id,
            context.get_person_property(person, Age),
            context.get_person_property(person, VaccineDoses),
            context.get_person_property(person, VaccineType),
            context.get_person_property(person, VaccineEfficacy)
        );
    });
}
