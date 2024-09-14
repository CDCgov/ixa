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

define_person_property!(DiseaseStatusType, DiseaseStatus, DiseaseStatus::S);

pub fn init(context: &mut Context) {
    context.subscribe_to_event(move |context, event: PersonCreatedEvent| {
        let person = event.person_id;
        context.add_plan(1.0, move |context| {
            context.set_person_property(person, DiseaseStatusType, DiseaseStatus::I);
        });
        context.add_plan(2.0, move |context| {
            context.set_person_property(person, DiseaseStatusType, DiseaseStatus::R);
        });
    });
}
