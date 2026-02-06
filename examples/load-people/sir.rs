use ixa::prelude::*;

use crate::Person;

define_property!(
    enum DiseaseStatus {
        S,
        I,
        R,
    },
    Person,
    default_const = DiseaseStatus::S
);

pub fn init(context: &mut Context) {
    context.subscribe_to_event(move |context, event: EntityCreatedEvent<Person>| {
        let person = event.entity_id;
        context.add_plan(1.0, move |context| {
            context.set_property(person, DiseaseStatus::I);
        });
        context.add_plan(2.0, move |context| {
            context.set_property(person, DiseaseStatus::R);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::population_loader::{Age, RiskCategory};
    use crate::vaccine::{VaccineDoses, VaccineEfficacy, VaccineType};
    use crate::{Person, PersonId};

    #[test]
    fn test_disease_status() {
        let mut context = Context::new();
        init(&mut context);

        let person: PersonId = context
            .add_entity((
                Age(0),
                RiskCategory::Low,
                VaccineType::A,
                VaccineEfficacy(0.0),
                VaccineDoses(0),
            ))
            .unwrap();

        // People should start in the S state
        assert_eq!(
            context.get_property::<Person, DiseaseStatus>(person),
            DiseaseStatus::S
        );

        // At 1.0, people should be in the "I" state
        context.subscribe_to_event(
            |context, event: PropertyChangeEvent<Person, DiseaseStatus>| {
                let person = event.entity_id;
                if context.get_current_time() == 1.0 {
                    assert_eq!(
                        context.get_property::<Person, DiseaseStatus>(person),
                        DiseaseStatus::I
                    );
                }
            },
        );

        context.execute();

        // People should end up in the R state by the end of the simulation
        assert_eq!(
            context.get_property::<Person, DiseaseStatus>(person),
            DiseaseStatus::R
        );
    }
}
