//! Two variants on the `core_epi` transmission loop. The infection details
//! are the variant: an infection carries its infector's variant, and the
//! variant decides the rate function, how much a past infection protects and
//! how well the vaccine works. An original variant spreads first; an escape
//! variant is seeded once that wave is over and reinfects part of it.
use ixa::core_epi::{ConstantRateFn, InfectionManager, RateAssignment, TransmissionManager};
use ixa::define_modifier;
use ixa::prelude::*;

const SEED: u64 = 42;
const POPULATION: usize = 1000;
const INITIAL_INFECTIONS: usize = 5;
const INFECTIOUS_PERIOD: f64 = 5.0;
const ESCAPE_INTRODUCED: f64 = 100.0;
const VACCINE_COVERAGE: f64 = 0.4;
/// Susceptibility to the escape variant after an infection with the original.
const SUSCEPTIBILITY_AFTER_ORIGINAL: f64 = 0.5;

define_entity!(Person);

/// Marker type the `core_epi` traits are implemented for.
struct CoreEpiExample;

define_modifier!(Vaccinated);

define_rng!(VaccineRng);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Variant {
    #[default]
    Original,
    Escape,
}

impl Variant {
    /// Attempts per day while infectious.
    fn infectiousness(self) -> f64 {
        match self {
            Variant::Original => 0.5,
            Variant::Escape => 0.8,
        }
    }

    /// Susceptibility of a vaccinated person relative to an unvaccinated one.
    fn vaccinated_susceptibility(self) -> f64 {
        match self {
            Variant::Original => 0.2,
            Variant::Escape => 0.7,
        }
    }
}

define_property!(struct InfectionCount(u8), Person, default_const = InfectionCount(0));

impl InfectionManager<CoreEpiExample> for Context {
    type Entity = Person;
    type Details = Variant;

    fn draw_infectiousness_rate_fn(
        &mut self,
        _person: PersonId,
        variant: &Variant,
    ) -> RateAssignment {
        ConstantRateFn::new(variant.infectiousness(), INFECTIOUS_PERIOD).into()
    }

    // The latest infection is enough here, because the escape variant
    // protects against both. A model whose immunity depends on every past
    // infection keeps that history in a property of its own.
    fn base_susceptibility(
        &self,
        person: PersonId,
        _infector: Option<PersonId>,
        variant: &Variant,
    ) -> f64 {
        match (self.infection_details(person), variant) {
            (None, _) => 1.0,
            (Some(Variant::Original), Variant::Escape) => SUSCEPTIBILITY_AFTER_ORIGINAL,
            (Some(_), _) => 0.0,
        }
    }

    fn on_infected(&mut self, _infector: Option<PersonId>, infectee: PersonId, _variant: Variant) {
        let InfectionCount(count) = self.get_property(infectee);
        self.set_property(infectee, InfectionCount(count + 1));
    }
}

impl TransmissionManager<CoreEpiExample> for Context {
    // An infection carries its infector's variant.
    fn details_from(&self, infector: PersonId) -> Variant {
        *self
            .infection_details(infector)
            .expect("an infector is infected")
    }
}

fn count_with_latest(context: &Context, variant: Variant) -> usize {
    context
        .get_entity_iterator::<Person>()
        .filter(|&person| context.infection_details(person) == Some(&variant))
        .count()
}

fn main() {
    let mut context = Context::new();
    context.init_random(SEED);
    for _ in 0..POPULATION {
        let person = context.add_entity(Person).unwrap();
        if context.sample_bool(VaccineRng, VACCINE_COVERAGE) {
            context.set_susceptibility_modifier_fn(
                person,
                Vaccinated,
                |_context, _person, _elapsed, variant| variant.vaccinated_susceptibility(),
            );
        }
    }

    context.setup_transmission_loop();
    context.seed_infections(Person, INITIAL_INFECTIONS);
    context.add_plan(ESCAPE_INTRODUCED, |context| {
        println!(
            "original wave: {} infected",
            count_with_latest(context, Variant::Original)
        );
        context.seed_infections_with(Person, INITIAL_INFECTIONS, Variant::Escape);
    });
    context.execute();

    let reinfected = context.query_entity_count(with!(Person, InfectionCount(2)));
    println!(
        "escape wave:   {} infected, {reinfected} of them for the second time",
        count_with_latest(&context, Variant::Escape)
    );
}
