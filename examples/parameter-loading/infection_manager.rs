use ixa::context::Context;

use ixa::define_rng;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::random::ContextRandomExt;

use rand_distr::Exp;

use crate::people::ContextPeopleExt;
use crate::people::InfectionStatus;
use crate::people::InfectionStatusEvent;
use crate::Parameters;

define_rng!(InfectionRng);

fn schedule_recovery(context: &mut Context, person_id: usize) {
    let parameters = context.get_global_property_value(Parameters).clone();
    let infection_duration = parameters.infection_duration;
    let recovery_time = context.get_current_time()
        + context.sample_distr(InfectionRng, Exp::new(1.0 / infection_duration).unwrap());
    context.add_plan(recovery_time, move |context| {
        context.set_person_status(person_id, InfectionStatus::R);
    });
}

fn handle_infection_status_change(context: &mut Context, event: InfectionStatusEvent) {
    if matches!(event.updated_status, InfectionStatus::I) {
        schedule_recovery(context, event.person_id);
    }
}

pub fn init(context: &mut Context) {
    context.subscribe_to_event::<InfectionStatusEvent>(move |context, event| {
        handle_infection_status_change(context, event);
    });
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::people::ContextPeopleExt;
    use crate::people::InfectionStatus;
    use crate::people::InfectionStatusEvent;
    use ixa::context::Context;
    use ixa::define_data_plugin;
    use ixa::global_properties::ContextGlobalPropertiesExt;
    use ixa::random::ContextRandomExt;
    define_data_plugin!(RecoveryPlugin, usize, 0);

    use crate::parameters_loader::ParametersValues;

    fn handle_recovery_event(context: &mut Context, event: InfectionStatusEvent) {
        if matches!(event.updated_status, InfectionStatus::R) {
            *context.get_data_container_mut(RecoveryPlugin) += 1;
        }
    }

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
        context.set_global_property_value(Parameters, p_values);
        context.init_random(42);
        init(&mut context);

        context.subscribe_to_event::<InfectionStatusEvent>(move |context, event| {
            handle_recovery_event(context, event);
        });

        let population_size = 10;
        for id in 0..population_size {
            context.create_person();
            context.set_person_status(id, InfectionStatus::I);
        }

        context.execute();
        let recovered_size: usize = *context.get_data_container(RecoveryPlugin).unwrap();

        assert_eq!(recovered_size, population_size);
    }
}
