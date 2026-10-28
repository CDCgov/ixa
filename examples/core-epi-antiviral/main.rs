//! The homogeneous `core-epi` model plus an antiviral: a share of infected
//! people start treatment some days after infection, which scales their
//! infectiousness down for the rest of the infection through a scheduled
//! infectiousness modifier. Runs the same seed with and without the antiviral
//! and prints both final sizes.
mod parameters;

use ixa::core_epi::{
    ConstantRateFn, InfectionManager, Person, PersonId, RateAssignment, TransmissionManager,
};
use ixa::prelude::*;
use rand_distr::Exp;

use crate::parameters::{Parameters, ParametersExt};

/// Marker type the `core_epi` traits are implemented for.
struct CoreEpiExample;

/// Identifies the antiviral's infectiousness modifier.
struct Antiviral;

define_rng!(InfectiousPeriodRng);
define_rng!(AntiviralRng);

define_property!(
    enum InfectionStatus {
        Susceptible,
        Infected,
    },
    Person,
    default_const = InfectionStatus::Susceptible
);

define_property!(
    enum Treatment {
        None,
        Antiviral,
    },
    Person,
    default_const = Treatment::None
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
        let &Parameters {
            antiviral_coverage,
            antiviral_delay,
            antiviral_effect,
            ..
        } = self.get_parameters();
        if self.sample_bool(AntiviralRng, antiviral_coverage) {
            self.set_property(infectee, Treatment::Antiviral);
            // Applies only if they are still infectious when treatment starts,
            // and lasts until this infection ends.
            self.schedule_infectiousness_modifier::<Antiviral>(
                infectee,
                antiviral_effect,
                self.get_current_time() + antiviral_delay,
                None,
            );
        }
    }
}

// Homogeneous mixing is the default; the antiviral reaches the loop through
// `intrinsic_infectiousness`.
impl TransmissionManager<CoreEpiExample> for Context {}

/// Final number infected and treated.
fn run(parameters: Parameters) -> (usize, usize) {
    let mut context = Context::new();
    let &Parameters {
        seed,
        population,
        initial_infections,
        ..
    } = &parameters;
    context.set_parameters(parameters);

    context.init_random(seed);
    for _ in 0..population {
        context.add_entity(Person).unwrap();
    }
    context.index_property::<Person, InfectionStatus>();
    context.index_property::<Person, Treatment>();

    context.setup_transmission_loop();
    context.seed_infections(Person, initial_infections);
    context.execute();
    (
        context.query_entity_count(with!(Person, InfectionStatus::Infected)),
        context.query_entity_count(with!(Person, Treatment::Antiviral)),
    )
}

fn main() {
    let parameters = Parameters::default();
    let (untreated, _) = run(Parameters {
        antiviral_coverage: 0.0,
        ..parameters.clone()
    });
    let (treated_run, treated) = run(parameters);
    println!("without antiviral: {untreated} infected");
    println!("with antiviral:    {treated_run} infected, {treated} treated");
}
