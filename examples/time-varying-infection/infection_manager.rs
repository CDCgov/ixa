use ixa::context::Context;
use ixa::define_rng;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::{ContextPeopleExt, PersonId, PersonPropertyChangeEvent};
use ixa::random::ContextRandomExt;

use rand_distr::Exp;

use crate::parameters_loader::Parameters;
use crate::population_loader::{DiseaseStatus, DiseaseStatusType};

define_rng!(InfectionRng);

fn schedule_recovery(context: &mut Context, person_id: PersonId) {
    let parameters = context.get_global_property_value(Parameters).clone();
    let recovery_time = context.get_current_time()
        + context.sample_distr(
            InfectionRng,
            Exp::new(1.0 / parameters.infection_duration).unwrap(),
        );
    context.add_plan(recovery_time, move |context| {
        context.set_person_property(person_id, DiseaseStatusType, DiseaseStatus::R);
    });
}

fn handle_infection_status_change(
    context: &mut Context,
    event: PersonPropertyChangeEvent<DiseaseStatusType>,
) {
    if matches!(event.current, DiseaseStatus::I) {
        schedule_recovery(context, event.person_id);
    }
}

pub fn init(context: &mut Context) {
    context.subscribe_to_event(
        move |context, event: PersonPropertyChangeEvent<DiseaseStatusType>| {
            handle_infection_status_change(context, event);
        },
    );
}

#[cfg(test)]
mod test {
    use super::*;
    use ixa::context::Context;
    use ixa::people::{ContextPeopleExt, PersonPropertyChangeEvent};
    use ixa::random::ContextRandomExt;

    use crate::parameters_loader::ParametersValues;
    use crate::population_loader::{DiseaseStatus, DiseaseStatusType};

    fn handle_recovery_event(event: PersonPropertyChangeEvent<DiseaseStatusType>) {
        assert_eq!(event.current, DiseaseStatus::R);
        assert_eq!(event.previous, DiseaseStatus::I);
    }

    #[test]
    fn test_handle_infection_change() {
        let p_values = ParametersValues {
            population: 1,
            max_time: 10.0,
            seed: 42,
            foi: 0.15,
            infection_duration: 5.0,
            output_dir: ".".to_string(),
            output_file: ".".to_string(),
        };
        let mut context = Context::new();

        context.set_global_property_value(Parameters, p_values);
        let parameters = context.get_global_property_value(Parameters).clone();
        context.init_random(parameters.seed);
        init(&mut context);

        for id in 0..parameters.population {
            context.add_person();
            context.set_person_property(
                context.get_person_id(id),
                DiseaseStatusType,
                DiseaseStatus::I,
            );
        }

        context.subscribe_to_event(
            move |_context, event: PersonPropertyChangeEvent<DiseaseStatusType>| {
                handle_recovery_event(event);
            },
        );

        context.execute();
    }
}
