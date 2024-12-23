use ixa::{
    context::Context,
    define_person_property_with_default,
    people::{ContextPeopleExt, PersonCreatedEvent},
};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum DiseaseStatusValue {
    S,
    I,
    R,
}

define_person_property_with_default!(DiseaseStatus, DiseaseStatusValue, DiseaseStatusValue::S);

pub fn init(context: &mut Context) {
    context.subscribe_to_event(move |context, event: PersonCreatedEvent| {
        let person = event.person_id;
        context.add_plan(1.0, move |context| {
            context.set_person_property(person, DiseaseStatus, DiseaseStatusValue::I);
        });
        context.add_plan(2.0, move |context| {
            context.set_person_property(person, DiseaseStatus, DiseaseStatusValue::R);
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

        let person = context.add_person(()).unwrap();

        // People should start in the S state
        assert_eq!(
            context.get_person_property(person, DiseaseStatus),
            DiseaseStatusValue::S
        );

        // At 1.0, people should be in the I state
        context.subscribe_to_event(|context, event: PersonPropertyChangeEvent<DiseaseStatus>| {
            let person = event.person_id;
            if context.get_current_time() == 1.0 {
                assert_eq!(
                    context.get_person_property(person, DiseaseStatus),
                    DiseaseStatusValue::I
                );
            }
        });

        context.execute();

        // People should end up in the R state by the end of the simulation
        assert_eq!(
            context.get_person_property(person, DiseaseStatus),
            DiseaseStatusValue::R
        );
    }
}
