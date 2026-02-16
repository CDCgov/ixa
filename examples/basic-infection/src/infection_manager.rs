use ixa::entity::events::PropertyChangeEvent;
use ixa::prelude::*;
use ixa::trace;
use rand_distr::Exp;

use crate::people::{InfectionStatus, Person, PersonId};
use crate::INFECTION_DURATION;

// Wherever we want to take action based on a person's status change, we need
// to listen to this event.
pub type InfectionStatusEvent = PropertyChangeEvent<Person, InfectionStatus>;

define_rng!(InfectionRng);

fn schedule_recovery(context: &mut Context, person_id: PersonId) {
    trace!("Scheduling recovery");
    let recovery_time = context.get_current_time()
        + context.sample_distr(InfectionRng, Exp::new(1.0 / INFECTION_DURATION).unwrap());
    context.add_plan(recovery_time, move |context| {
        context.set_property(person_id, InfectionStatus::R);
    });
}

fn handle_infection_status_change(context: &mut Context, event: InfectionStatusEvent) {
    trace!(
        "Handling infection status change from {:?} to {:?} for {:?}",
        event.previous,
        event.current,
        event.entity_id
    );
    if event.current == InfectionStatus::I {
        schedule_recovery(context, event.entity_id);
    }
}

pub fn init(context: &mut Context) {
    trace!("Initializing infection_manager");
    context.subscribe_to_event::<InfectionStatusEvent>(handle_infection_status_change);
}

#[cfg(test)]
mod test {
    use ixa::prelude::*;

    use crate::infection_manager::InfectionStatusEvent;
    use crate::people::InfectionStatus;

    define_data_plugin!(RecoveryPlugin, usize, 0);

    fn handle_recovery_event(context: &mut Context, event: InfectionStatusEvent) {
        if event.current == InfectionStatus::R {
            *context.get_data_mut(RecoveryPlugin) += 1;
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
            let person_id = context.add_entity((InfectionStatus::S,)).unwrap();
            context.set_property(person_id, InfectionStatus::I);
        }

        context.execute();
        let recovered_size: usize = *context.get_data(RecoveryPlugin);

        assert_eq!(recovered_size, population_size);
    }
}
