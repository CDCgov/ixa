use ixa::context::Context;
use ixa::define_rng;
use ixa::people::{ContextPeopleExt, PersonId, PersonPropertyChangeEvent};
use ixa::random::ContextRandomExt;

use rand_distr::Exp;

use crate::population_loader::{InfectionStatus, InfectionStatusType};
use crate::INFECTION_DURATION;

define_rng!(InfectionRng);

/// Schedules a recovery for every infected person
fn schedule_recovery(context: &mut Context, person_id: PersonId) {
    let recovery_time = context.get_current_time()
        + context.sample_distr(InfectionRng, Exp::new(1.0 / INFECTION_DURATION).unwrap());

    context.add_plan(recovery_time, move |context| {
        context.set_person_property(person_id, InfectionStatusType, InfectionStatus::R);
    });
}

/// Handles changes to the `InfectionStatusType` property
fn handle_infection_status_change(
    context: &mut Context,
    event: PersonPropertyChangeEvent<InfectionStatusType>,
) {
    if matches!(event.current, InfectionStatus::I) {
        schedule_recovery(context, event.person_id);
    }
}

/// Initializes the infection status change event handling in the given context.
pub fn init(context: &mut Context) {
    context.subscribe_to_event::<PersonPropertyChangeEvent<InfectionStatusType>>(
        move |context, event| {
            handle_infection_status_change(context, event);
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ixa::context::Context;
    use ixa::people::ContextPeopleExt;

    #[test]
    fn test_schedule_recovery() {
        let mut context = Context::new();
        context.init_random(0);
        init(&mut context);

        // Add a person and infect them
        let person = context.add_person(()).unwrap();
        context.set_person_property(person, InfectionStatusType, InfectionStatus::I);

        // Execute and ensure person recovered
        context.execute();
        assert_eq!(
            context.get_person_property(person, InfectionStatusType),
            InfectionStatus::R
        );
    }
}
