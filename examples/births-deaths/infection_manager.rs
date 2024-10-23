use ixa::context::Context;
use ixa::define_rng;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::{ContextPeopleExt, PersonId, PersonPropertyChangeEvent};
use ixa::random::ContextRandomExt;
use rand_distr::Exp;

use crate::population_manager::InfectionStatus;
use crate::population_manager::InfectionStatusType;
use crate::population_manager::Alive;
use crate::Parameters;

define_rng!(InfectionRng);

fn schedule_recovery(context: &mut Context, person_id: PersonId) {
    let parameters = context.get_global_property_value(Parameters).clone();
    let infection_duration = parameters.infection_duration;
    let recovery_time = context.get_current_time()
        + context.sample_distr(InfectionRng, Exp::new(1.0 / infection_duration).unwrap());
    if context.get_person_property(person_id, Alive)  {
        context.add_plan(recovery_time, move |context| {
            context.set_person_property(person_id, InfectionStatusType, InfectionStatus::R);
        });
    }
}

fn handle_infection_status_change(
    context: &mut Context,
    event: PersonPropertyChangeEvent<InfectionStatusType>,
) {
    if matches!(event.current, InfectionStatus::I) {
        schedule_recovery(context, event.person_id);
    }
}

pub fn init(context: &mut Context) {
    context.subscribe_to_event(
        move |context, event: PersonPropertyChangeEvent<InfectionStatusType>| {
            handle_infection_status_change(context, event);
        },
    );
}
