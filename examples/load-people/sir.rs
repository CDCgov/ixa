use ixa::{
    context::Context,
    define_person_property,
    people::{ContextPeopleExt, PersonCreatedEvent},
};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum DiseaseStatus {
    S,
    I,
    R,
}

define_person_property!(DiseaseStatusType, DiseaseStatus);

pub fn init(context: &mut Context) {
    // This runs immediately after creation, so before regular event callbacks
    context.set_person_property_default_value(DiseaseStatusType, DiseaseStatus::S);

    // Disease status should be set at this point
    context.subscribe_to_event(move |context, event: PersonCreatedEvent| {
        let person = event.person_id;
        let disease_status = context.get_person_property(person, DiseaseStatusType);
        println!(
            "Person created with id {}, disease status {:?}",
            person.id, disease_status
        );
        context.add_plan(1.0, move |context| {
            context.set_person_property(person, DiseaseStatusType, DiseaseStatus::I);
        });
        context.add_plan(2.0, move |context| {
            context.set_person_property(person, DiseaseStatusType, DiseaseStatus::R);
        });
    });
}
