use ixa::{prelude::*, PersonId, PersonPropertyChangeEvent};

use rand_distr::Exp;

use crate::people::{InfectionStatus, InfectionStatusValue};
use crate::INFECTION_DURATION;

// ANCHOR: infection_status_event
pub type InfectionStatusEvent = PersonPropertyChangeEvent<InfectionStatus>;
// ANCHOR_END: infection_status_event
define_rng!(InfectionRng);

// ANCHOR: schedule_recovery
fn schedule_recovery(context: &mut Context, person_id: PersonId) {
    trace!("Scheduling recovery");
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
// ANCHOR_END: schedule_recovery

// ANCHOR: handle_infection_status_change
fn handle_infection_status_change(context: &mut Context, event: InfectionStatusEvent) {
    trace!(
        "Handling infection status change from {:?} to {:?} for {:?}",
        event.previous,
        event.current,
        event.person_id
    );
    if event.current == InfectionStatusValue::I {
        schedule_recovery(context, event.person_id);
    }
}
// ANCHOR_END: handle_infection_status_change

// ANCHOR: init
pub fn init(context: &mut Context) {
    trace!("Initializing infection_manager");
    context.subscribe_to_event::<InfectionStatusEvent>(handle_infection_status_change);
}
// ANCHOR_END: init
