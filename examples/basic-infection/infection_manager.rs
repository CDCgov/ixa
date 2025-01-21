use ixa::context::Context;

use ixa::random::ContextRandomExt;
use ixa::{define_rng, ContextPeopleExt, PersonId, PersonPropertyChangeEvent};

use rand_distr::Exp;

use crate::people::{InfectionStatus, InfectionStatusValue};
use crate::INFECTION_DURATION;

// Wherever we want to take action based on a person's status change, we need
// to listen to this event.
pub type InfectionStatusEvent = PersonPropertyChangeEvent<InfectionStatus>;

define_rng!(InfectionRng);

fn schedule_recovery(context: &mut Context, person_id: PersonId) {
    let recovery_time = context.get_current_time()
        + context.sample_distr(InfectionRng, Exp::new(1.0 / INFECTION_DURATION).unwrap());
    context.add_plan(recovery_time, move |context| {
        context.set_person_property::<InfectionStatus>(
            person_id,
            InfectionStatus,
            InfectionStatusValue::R,
        );
    });
}

fn handle_infection_status_change(context: &mut Context, event: InfectionStatusEvent) {
    if event.current == InfectionStatusValue::I {
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
    use crate::infection_manager::InfectionStatusEvent;
    use crate::people::InfectionStatus;
    use crate::people::InfectionStatusValue;
    use ixa::context::Context;
    use ixa::random::ContextRandomExt;
    use ixa::{define_data_plugin, ContextPeopleExt};

    define_data_plugin!(RecoveryPlugin, usize, 0);

    fn handle_recovery_event(context: &mut Context, event: InfectionStatusEvent) {
        if event.current == InfectionStatusValue::R {
            *context.get_data_container_mut(RecoveryPlugin) += 1;
        }
    }

    #[test]
    fn test_handle_infection_change() {
        use super::init;
        let mut context = Context::new();
        context.init_random(42);
        init(&mut context);

        context.subscribe_to_event::<InfectionStatusEvent>(move |context, event| {
            handle_recovery_event(context, event);
        });

        let population_size = 10;
        for _ in 0..population_size {
            let person_id = context
                .add_person((InfectionStatus, InfectionStatusValue::S))
                .unwrap();
            context.set_person_property::<InfectionStatus>(
                person_id,
                InfectionStatus,
                InfectionStatusValue::I,
            );
        }

        context.execute();
        let recovered_size: usize = *context.get_data_container(RecoveryPlugin).unwrap();

        assert_eq!(recovered_size, population_size);
    }
}
