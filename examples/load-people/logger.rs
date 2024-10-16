use crate::{
    population_loader::Age,
    sir::DiseaseStatusType,
    vaccine::{VaccineDoses, VaccineEfficacy, VaccineType},
};
use ixa::{context::Context, people::PersonCreatedEvent, people::PersonPropertyChangeEvent, people::ContextPeopleExt};

pub fn init(context: &mut Context) {
    // This subscribes to the disease status change events
    // Note that no event gets fired when the property is set the first time
    context.subscribe_to_event::<PersonPropertyChangeEvent<DiseaseStatusType>>(|_context, data| {
        println!(
            "{:?} changed disease status from {:?} to {:?}",
            data.person_id, data.previous, data.current
        );
    });

    // Logs when a person is created
    context.subscribe_to_event::<PersonCreatedEvent>(|context, event| {
        println!(
            "{:?} age: {}, {} vaccine doses, vaccine {:?} ({})",
            event.person_id,
            context.get_person_property(event.person_id, Age),
            context.get_person_property(event.person_id, VaccineDoses),
            context.get_person_property(event.person_id, VaccineType),
            context.get_person_property(event.person_id, VaccineEfficacy)
        );
    });
}
