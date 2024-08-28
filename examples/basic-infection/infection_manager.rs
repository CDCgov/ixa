use ixa::context::Context;
use ixa::define_rng;
use ixa::random::ContextRandomExt;

use rand_distr::Exp;

use crate::people::InfectionStatus;
use crate::people::PeopleContext;

use crate::INFECTION_DURATION;

define_rng!(InfectionRng);

pub trait InfectionManager {
    fn schedule_recovery(&mut self, person_id: usize);
}

fn schedule_recovery(context: &mut Context, person_id: usize) {
    let recovery_time = context.get_current_time()
        + context.sample_distr(InfectionRng, Exp::new(1.0 / INFECTION_DURATION).unwrap());
    context.add_plan(recovery_time, move |context| {
        context.set_person_status(person_id, InfectionStatus::R);
        println!("{:?}, {:?}, R", context.get_current_time(), person_id);
    });
}

impl InfectionManager for Context {
    fn schedule_recovery(&mut self, person_id: usize) {
        schedule_recovery(self, person_id);
    }
}
