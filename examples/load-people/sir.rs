use ixa::{
    context::Context,
    define_person_property, define_person_property_with_default,
    people::{ContextPeopleExt, PersonCreatedEvent},
};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum DiseaseStatus {
    S,
    I,
    R,
}

define_person_property_with_default!(DiseaseStatusType, DiseaseStatus, DiseaseStatus::S);

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

#[cfg(test)]
mod tests {
    use super::*;
    use ixa::{context::Context, people::PersonPropertyChangeEvent};

    #[test]
    fn test_disease_status() {
        let mut context = Context::new();
        init(&mut context);

        let person = context.add_person();

        // People should start in the S state
        assert_eq!(
            context.get_person_property(person, DiseaseStatusType),
            DiseaseStatus::S
        );

        // At 1.0, people should be in the I state
        context.subscribe_to_event(
            |context, event: PersonPropertyChangeEvent<DiseaseStatusType>| {
                let person = event.person_id;
                if context.get_current_time() == 1.0 {
                    assert_eq!(
                        context.get_person_property(person, DiseaseStatusType),
                        DiseaseStatus::I
                    );
                }
            },
        );

        context.execute();

        // People should end up in the R state by the end of the simulation
        assert_eq!(
            context.get_person_property(person, DiseaseStatusType),
            DiseaseStatus::R
        );
    }
}
