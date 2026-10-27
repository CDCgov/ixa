//! A minimal SIR outbreak on the `core_epi` transmission loop: people in
//! households of random size (see `settings`), each transmission attempt
//! finding the person at home or in the community at random, a constant
//! intrinsic infectiousness over an infectious period drawn per person from an
//! exponential distribution, and a few seeded infections.
//!
//! Total infectiousness is the intrinsic infectiousness times the setting's
//! `(n - 1)^alpha`: with the default exponents, 1 in the community and the
//! number of other members at home. R0 is roughly `intrinsic_infectiousness *
//! infectious_period * mean multiplier`; about 2 with the default parameters.
mod parameters;
mod settings;

use ixa::core_epi::{
    ConstantRateFn, InfectionManager, InvertibleRate, Person, PersonId, RateAssignment,
    ScaledRateFn, Transmission,
};
use ixa::prelude::*;
use ixa::ExecutionPhase;
use rand_distr::Exp;

use crate::parameters::{Parameters, ParametersExt};
use crate::settings::Setting;

/// Marker type the `core_epi` traits are implemented for.
struct CoreEpiExample;

define_rng!(ContactRng);
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

impl Transmission<CoreEpiExample> for Context {
    fn forecast_total_infectiousness(&self, person: PersonId) -> impl InvertibleRate {
        // Households never change, so the larger setting multiplier bounds the run.
        ScaledRateFn::new(
            self.majorizing_infectiousness_rate_fn(person),
            Setting::max_multiplier(self, person),
        )
    }

    fn evaluate_forecast(&self, person: PersonId, forecast: f64) -> Option<PersonId> {
        let setting = Setting::draw(self);
        let total = self.intrinsic_infectiousness(person)? * setting.multiplier(self, person);
        let attempt =
            total >= forecast || (total > 0.0 && self.sample_bool(ContactRng, total / forecast));
        if !attempt {
            return None;
        }
        setting.sample_contact(self, person)
    }

    fn susceptibility(&self, person: PersonId) -> f64 {
        // Infection gives permanent immunity.
        match self.get_property(person) {
            InfectionStatus::Susceptible => 1.0,
            InfectionStatus::Infected => 0.0,
        }
    }
}

fn main() {
    let mut context = Context::new();
    context.set_parameters(Parameters::default());
    let &Parameters {
        seed,
        initial_infections,
        ..
    } = context.get_parameters();

    context.init_random(seed);
    settings::create_households(&mut context);
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
