//! The smallest model on the `core_epi` transmission loop: an SIR outbreak
//! with homogeneous mixing. Each person gets a constant intrinsic
//! infectiousness over an infectious period drawn from an exponential
//! distribution, and every attempt contacts someone uniformly at random. See
//! `core-epi-settings` for a population with households, schools and
//! workplaces.
mod parameters;

use ixa::core_epi::{
    ConstantRateFn, InfectionManager, Person, PersonId, RateAssignment, TransmissionManager,
};
use ixa::prelude::*;
use ixa::ExecutionPhase;
use rand_distr::Exp;

use crate::parameters::{Parameters, ParametersExt};

/// Marker type the `core_epi` traits are implemented for.
struct CoreEpiExample;

define_rng!(InfectiousPeriodRng);

define_property!(
    enum InfectionStatus {
        Susceptible,
        Infected,
    },
    Person,
    default_const = InfectionStatus::Susceptible
);

impl InfectionManager<CoreEpiExample> for Context {
    fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
        let params = self.get_parameters();
        let duration = self.sample_distr(
            InfectiousPeriodRng,
            Exp::new(1.0 / params.infectious_period).unwrap(),
        );
        ConstantRateFn::new(params.intrinsic_infectiousness, duration).into()
    }

    fn on_infected(&mut self, _infector: Option<PersonId>, infectee: PersonId) {
        self.set_property(infectee, InfectionStatus::Infected);
    }
}

// Homogeneous mixing is the default: forecast, evaluation and contact.
impl TransmissionManager<CoreEpiExample> for Context {}

fn main() {
    let mut context = Context::new();
    context.set_parameters(Parameters::default());
    let &Parameters {
        seed,
        population,
        initial_infections,
        ..
    } = context.get_parameters();

    context.init_random(seed);
    for _ in 0..population {
        context.add_entity(Person).unwrap();
    }
    context.index_property::<Person, InfectionStatus>();

    context.setup_transmission_loop();
    context.seed_infections(Person, initial_infections);

    context.add_periodic_plan_with_phase(
        1.0,
        |context| {
            println!(
                "day {:>3}: {} infected",
                context.get_current_time(),
                context.query_entity_count(with!(Person, InfectionStatus::Infected))
            );
        },
        ExecutionPhase::Last,
    );
    context.execute();
}
