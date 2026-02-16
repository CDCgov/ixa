use ixa::prelude::*;
use rand_distr::Exp;

use crate::INFECTION_DURATION;
use crate::people::{InfectionStatus, Person, PersonId};

// ANCHOR: infection_status_event
pub type InfectionStatusEvent = PropertyChangeEvent<Person, InfectionStatus>;
// ANCHOR_END: infection_status_event

define_rng!(InfectionRng);

// ANCHOR: schedule_recovery
fn schedule_recovery(context: &mut Context, person_id: PersonId) {
    trace!("Scheduling recovery");
    let current_time = context.get_current_time();
    let sampled_infection_duration =
        context.sample_distr(InfectionRng, Exp::new(1.0 / INFECTION_DURATION).unwrap());
    let recovery_time = current_time + sampled_infection_duration;

    context.add_plan(recovery_time, move |context| {
        context.set_property(person_id, InfectionStatus::R);
    });
}
// ANCHOR_END: schedule_recovery

// ANCHOR: handle_infection_status_change
fn handle_infection_status_change(context: &mut Context, event: InfectionStatusEvent) {
    trace!(
        "Handling infection status change from {:?} to {:?} for {:?}",
        event.previous, event.current, event.entity_id
    );
    if event.current == InfectionStatus::I {
        schedule_recovery(context, event.entity_id);
    }
}
// ANCHOR_END: handle_infection_status_change

// ANCHOR: init
pub fn init(context: &mut Context) {
    trace!("Initializing infection_manager");
    context.subscribe_to_event::<InfectionStatusEvent>(handle_infection_status_change);
}
// ANCHOR_END: init
