use ixa::context::Context;
use ixa::random::ContextRandomExt;

use crate::people::PeopleContext;

static FOI: f64 = 0.01;

pub trait TransmissionManager {
    fn initialize_transmission(&mut self);
}


fn attempt_infection<>(context: &mut Context) {
    population = context.get_population();
    person_to_infect = 1;

    /*if (context.get_infection_status(person_to_infect) == Susceptible) {
        context.set_infection_status(person_to_infect, Infected);
    }
    context.sample_distr<TransmissionRng, f64>(ExpDist)

    foi = parameters.get_parameter(foi);
    time_next_infection = transmission_rng.draw_exponential(1/foi);
    context.add_plan(attempt_infection(context), time = context.get_time() + time_next_infection);
     */
}


impl TransmissionManager for Context {
    fn initialize_transmission(&mut self) {
        define_rng!(TransmissionRng);
        self.add_plan(0, |self| {
            attempt_infection(self)
        });
    }
}
