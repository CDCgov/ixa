#![cfg(feature = "core_epi")]

use ixa::core_epi::{
    ConstantRateFn, InfectionManager, InvertibleRate, Person, PersonId, RateAssignment,
    Transmission,
};
use ixa::prelude::*;

struct Sir;

define_rng!(ContactRng);

const POPULATION: usize = 1000;

impl InfectionManager<Sir> for Context {
    fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
        RateAssignment::Library(0)
    }
}

impl Transmission<Sir> for Context {
    fn forecast_total_infectiousness(&self, person: PersonId) -> impl InvertibleRate {
        // Homogeneous mixing at rate 1: total is intrinsic.
        self.majorizing_infectiousness_rate_fn(person)
    }

    fn evaluate_forecast(&self, person: PersonId, _forecast: f64) -> Option<PersonId> {
        // Constant infectiousness and homogeneous mixing: the forecast is tight.
        let contact = self.sample_entity(ContactRng, Person)?;
        (contact != person).then_some(contact)
    }

    fn susceptibility(&self, person: PersonId) -> f64 {
        if InfectionManager::<Sir>::infection_time(self, person).is_some() {
            0.0
        } else {
            1.0
        }
    }
}

fn run(r0: f64) -> (Context, usize) {
    let mut context = Context::new();
    context.init_random(42);
    for _ in 0..POPULATION {
        context.add_entity(Person).unwrap();
    }
    InfectionManager::<Sir>::load_infectiousness_rate_library(
        &mut context,
        vec![ConstantRateFn::new(r0 / 5.0, 5.0).into()],
    );
    Transmission::<Sir>::setup_transmission_loop(&mut context);
    Transmission::<Sir>::seed_infections(&mut context, Person, 5);
    context.execute();
    let infected = context
        .get_entity_iterator::<Person>()
        .filter(|&p| InfectionManager::<Sir>::infection_time(&context, p).is_some())
        .count();
    (context, infected)
}

#[test]
fn outbreak_spreads_and_ends() {
    // Final size for R0 = 2 is ~80%; `execute` returning means every loop ended.
    let (_, infected) = run(2.0);
    assert!(
        (700..=900).contains(&infected),
        "unexpected final size {infected}"
    );
}

#[test]
fn zero_infectiousness_never_transmits() {
    let (_, infected) = run(0.0);
    assert_eq!(infected, 5);
}

#[test]
fn modifier_scales_infectiousness_for_current_infection() {
    struct Mask;
    let mut context = Context::new();
    context.init_random(1);
    let person = context.add_entity(Person).unwrap();
    InfectionManager::<Sir>::load_infectiousness_rate_library(
        &mut context,
        vec![ConstantRateFn::new(1.0, 5.0).into()],
    );
    InfectionManager::<Sir>::assign_infectiousness_rate_fn(&mut context, person);
    InfectionManager::<Sir>::set_infectiousness_modifier::<Mask>(&mut context, person, 0.5);
    assert_eq!(
        InfectionManager::<Sir>::intrinsic_infectiousness(&context, person),
        Some(0.5)
    );
    InfectionManager::<Sir>::remove_infectiousness_modifier::<Mask>(&mut context, person);
    assert_eq!(
        InfectionManager::<Sir>::infectiousness_multiplier(&context, person),
        1.0
    );
    // Reinfection clears modifiers.
    InfectionManager::<Sir>::set_infectiousness_modifier::<Mask>(&mut context, person, 0.5);
    InfectionManager::<Sir>::assign_infectiousness_rate_fn(&mut context, person);
    assert_eq!(
        InfectionManager::<Sir>::intrinsic_infectiousness(&context, person),
        Some(1.0)
    );
}
