use crate::{
    population_loader::Age,
    sir::DiseaseStatusType,
    vaccine::{VaccineDoses, VaccineEfficacy, VaccineType},
};
use ixa::{context::Context, people::ContextPeopleExt};

pub fn init(context: &mut Context) {
    // This subscribes to the disease status change events
    // Note that no event gets fired when the property is set the first time
    context.subscribe_to_person_property_changed(DiseaseStatusType, |_context, data| {
        println!(
            "{:?} changed disease status from {:?} to {:?}",
            data.person_id, data.previous, data.current
        );
    });

    // Logs when a person is created
    context.subscribe_to_person_created(|context, person| {
        println!(
            "{:?} age: {}, {} vaccine doses, vaccine {:?} ({})",
            person,
            context.get_person_property(person, Age),
            context.get_person_property(person, VaccineDoses),
            context.get_person_property(person, VaccineType),
            context.get_person_property(person, VaccineEfficacy)
        );
    });
}
