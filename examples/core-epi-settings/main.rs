//! A minimal SIR outbreak on the `core_epi` transmission loop: people loaded
//! from a line list into households, schools and workplaces (see `settings`),
//! each transmission attempt finding the person in one of those or in the
//! community, a constant intrinsic infectiousness over an infectious period
//! drawn per person from an exponential distribution, and a few seeded
//! infections.
//!
//! Total infectiousness is the intrinsic infectiousness times the setting's
//! `(n - 1)^alpha`, averaged over the person's itinerary. R0 is roughly
//! `intrinsic_infectiousness * infectious_period * mean multiplier`.
mod parameters;
mod settings;

use ixa::core_epi::{
    ConstantRateFn, InfectionManager, RateAssignment, SettingKind, SettingsExt, TransmissionManager,
};
use ixa::prelude::*;
use ixa::ExecutionPhase;
use rand_distr::Exp;

use crate::parameters::{Parameters, ParametersExt};
use crate::settings::{Person, PersonId, Setting};

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

/// Transmissions counted by the setting they happened in, by kind index.
#[derive(Default)]
struct InfectionsBySetting(Vec<usize>);
define_data_plugin!(
    InfectionsBySettingPlugin,
    InfectionsBySetting,
    InfectionsBySetting::default()
);

impl InfectionManager<CoreEpiExample> for Context {
    type Entity = Person;
    // The setting each transmission happened in; `None` for a seed.
    type Exposure = Option<Setting>;

    fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
        let params = self.get_parameters();
        let duration = self.sample_distr(
            InfectiousPeriodRng,
            Exp::new(1.0 / params.infectious_period).unwrap(),
        );
        ConstantRateFn::new(params.intrinsic_infectiousness, duration).into()
    }

    fn on_infected(
        &mut self,
        _infector: Option<PersonId>,
        infectee: PersonId,
        exposure: Option<Setting>,
    ) {
        self.set_property(infectee, InfectionStatus::Infected);
        if let Some(setting) = exposure {
            let infections = &mut self.get_data_mut(InfectionsBySettingPlugin).0;
            if infections.len() <= setting.index() {
                infections.resize(setting.index() + 1, 0);
            }
            infections[setting.index()] += 1;
        }
    }
}

impl TransmissionManager<CoreEpiExample> for Context {
    fn max_contact_multiplier(&self, person: PersonId) -> f64 {
        self.max_setting_multiplier::<Setting>(person)
    }

    fn evaluate_forecast(
        &self,
        person: PersonId,
        forecast: f64,
    ) -> Option<(PersonId, Option<Setting>)> {
        // Where the attempt happens decides its rate and who it can reach.
        let setting = self.sample_setting::<Setting>(person)?;
        let total =
            self.intrinsic_infectiousness(person)? * self.setting_multiplier(person, setting);
        if !self.accept_proposal(person, total, forecast) {
            return None;
        }
        let contact = self.sample_setting_contact(person, setting)?;
        Some((contact, Some(setting)))
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
    settings::setup(&mut context);
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

    let infections = &context.get_data(InfectionsBySettingPlugin).0;
    for setting in context.setting_kinds::<Setting>() {
        println!(
            "infected at {}: {}",
            context.setting_name(setting),
            infections.get(setting.index()).copied().unwrap_or(0)
        );
    }
}
