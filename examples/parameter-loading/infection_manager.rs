use ixa::context::Context;
use ixa::define_rng;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::{ContextPeopleExt, PersonId, PersonPropertyChangeEvent};
use ixa::random::ContextRandomExt;
use rand_distr::Exp;

use crate::InfectionStatus;
use crate::InfectionStatusValue;
use crate::Parameters;

define_rng!(InfectionRng);

fn schedule_recovery(context: &mut Context, person_id: PersonId) {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    let infection_duration = parameters.infection_duration;
    let recovery_time = context.get_current_time()
        + context.sample_distr(InfectionRng, Exp::new(1.0 / infection_duration).unwrap());
    context.add_plan(recovery_time, move |context| {
        context.set_person_property(person_id, InfectionStatus, InfectionStatusValue::R);
    });
}

fn handle_infection_status_change(
    context: &mut Context,
    event: PersonPropertyChangeEvent<InfectionStatus>,
) {
    if matches!(event.current, InfectionStatusValue::I) {
        schedule_recovery(context, event.person_id);
    }
}

pub fn init(context: &mut Context) {
    context.subscribe_to_event(
        move |context, event: PersonPropertyChangeEvent<InfectionStatus>| {
            handle_infection_status_change(context, event);
        },
    );
}

#[cfg(test)]
mod test {
    use super::*;
    use ixa::context::Context;
    use ixa::define_data_plugin;
    use ixa::global_properties::ContextGlobalPropertiesExt;
    use ixa::people::{ContextPeopleExt, PersonPropertyChangeEvent};
    use ixa::random::ContextRandomExt;
    define_data_plugin!(RecoveryPlugin, usize, 0);

    use crate::parameters_loader::ParametersValues;

    #[test]
    fn test_handle_infection_change() {
        let p_values = ParametersValues {
            population: 10,
            max_time: 10.0,
            seed: 42,
            foi: 0.15,
            infection_duration: 5.0,
            output_dir: ".".to_string(),
            output_file: ".".to_string(),
        };
        let mut context = Context::new();

        context
            .set_global_property_value(Parameters, p_values.clone())
            .unwrap();
        context.init_random(42);
        init(&mut context);

        context.subscribe_to_event(
            move |context, event: PersonPropertyChangeEvent<InfectionStatus>| {
                if matches!(event.current, InfectionStatusValue::R) {
                    *context.get_data_container_mut(RecoveryPlugin) += 1;
                }
            },
        );

        let population_size: usize = 10;
        for _ in 0..population_size {
            let person = context.add_person(()).unwrap();

            context.add_plan(1.0, move |context| {
                context.set_person_property(person, InfectionStatus, InfectionStatusValue::I);
            });
        }
        context.execute();
        assert_eq!(population_size, context.get_current_population());
        let recovered_size: usize = *context.get_data_container(RecoveryPlugin).unwrap();

        assert_eq!(recovered_size, population_size);
    }
}
