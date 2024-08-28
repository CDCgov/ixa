use ixa::context::Context;

use ixa::define_rng;
use ixa::random::ContextRandomExt;

use rand_distr::Exp;

use crate::people::InfectionStatus;
use crate::people::InfectionStatusEvent;
use crate::people::PeopleContext;

use crate::INFECTION_DURATION;

define_rng!(InfectionRng);

pub trait InfectionManager {
    //fn schedule_recovery(&mut self, person_id: usize);
    fn initialize_infection_manager(&mut self);
}

//pub struct InfectionManager {}

fn schedule_recovery(context: &mut Context, person_id: usize) {
    let recovery_time = context.get_current_time()
        + context.sample_distr(InfectionRng, Exp::new(1.0 / INFECTION_DURATION).unwrap());
    context.add_plan(recovery_time, move |context| {
        context.set_person_status(person_id, InfectionStatus::R);
        println!("{:?}, {:?}, R", context.get_current_time(), person_id);
    });
}
fn handle_infection_status_change(context: &mut Context, event: InfectionStatusEvent) {
    if matches!(
        //context.get_person_status(event.person_id),
        event.updated_status,
        InfectionStatus::I
    ) {
        schedule_recovery(context, event.person_id);
    }
}

fn initialize(context: &mut Context) {
    context.subscribe_to_event::<InfectionStatusEvent>(move |context, event| {
        handle_infection_status_change(context, event);
    });
}

impl InfectionManager for Context {
    fn initialize_infection_manager(&mut self) {
        initialize(self);
    }
}
