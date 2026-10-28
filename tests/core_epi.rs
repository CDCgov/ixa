#![cfg(feature = "core_epi")]

use ixa::core_epi::{
    ConstantRateFn, InfectionManager, InvertibleRate, Person, PersonId, RateAssignment,
    ScaledRateFn, StepFn, TransmissionManager,
};
use ixa::prelude::*;

struct Sir;

const POPULATION: usize = 1000;

impl InfectionManager<Sir> for Context {
    fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
        RateAssignment::Library(0)
    }
}

// Homogeneous mixing: all defaults.
impl TransmissionManager<Sir> for Context {}

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
    TransmissionManager::<Sir>::setup_transmission_loop(&mut context);
    TransmissionManager::<Sir>::seed_infections(&mut context, Person, 5);
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
fn susceptibility_modifiers_compose_and_persist_across_infection() {
    struct Vaccine;
    struct Age;
    let mut context = Context::new();
    context.init_random(1);
    let person = context.add_entity(Person).unwrap();
    InfectionManager::<Sir>::load_infectiousness_rate_library(
        &mut context,
        vec![ConstantRateFn::new(1.0, 5.0).into()],
    );
    assert_eq!(
        InfectionManager::<Sir>::susceptibility(&context, person),
        1.0
    );
    InfectionManager::<Sir>::set_susceptibility_modifier::<Vaccine>(&mut context, person, 0.5);
    InfectionManager::<Sir>::set_susceptibility_modifier::<Age>(&mut context, person, 0.5);
    assert_eq!(
        InfectionManager::<Sir>::susceptibility(&context, person),
        0.25
    );
    InfectionManager::<Sir>::remove_susceptibility_modifier::<Age>(&mut context, person);
    assert_eq!(
        InfectionManager::<Sir>::susceptibility(&context, person),
        0.5
    );
    // Infection zeroes the base, but the modifier outlives it.
    InfectionManager::<Sir>::assign_infectiousness_rate_fn(&mut context, person);
    assert_eq!(
        InfectionManager::<Sir>::susceptibility(&context, person),
        0.0
    );
    assert_eq!(
        InfectionManager::<Sir>::susceptibility_multiplier(&context, person),
        0.5
    );
}

#[test]
fn function_modifiers_follow_elapsed_time() {
    struct Antiviral;
    struct Vaccine;
    let mut context = Context::new();
    context.init_random(1);
    let person = context.add_entity(Person).unwrap();
    InfectionManager::<Sir>::load_infectiousness_rate_library(
        &mut context,
        vec![ConstantRateFn::new(1.0, 10.0).into()],
    );
    // Protection that wanes after a day, as a step function.
    let waning = StepFn::new(0.2).step_at(1.0, 0.5);
    InfectionManager::<Sir>::set_susceptibility_modifier_fn::<Vaccine>(
        &mut context,
        person,
        move |_, _, t| waning.at(t),
    );
    assert_eq!(
        InfectionManager::<Sir>::susceptibility(&context, person),
        0.2
    );
    context.add_plan(2.0, move |ctx| {
        assert_eq!(
            InfectionManager::<Sir>::susceptibility_multiplier(ctx, person),
            0.5
        );
        InfectionManager::<Sir>::assign_infectiousness_rate_fn(ctx, person);
        // Treatment effect that ramps in over the first two days of infection,
        // reading the context to check it is the one passed in.
        InfectionManager::<Sir>::set_infectiousness_modifier_fn::<Antiviral>(
            ctx,
            person,
            |ctx, p, t| {
                assert!(InfectionManager::<Sir>::is_infectious(ctx, p));
                1.0 - 0.25 * t.min(2.0)
            },
        );
        assert_eq!(
            InfectionManager::<Sir>::intrinsic_infectiousness(ctx, person),
            Some(1.0)
        );
    });
    context.add_plan(3.0, move |ctx| {
        assert_eq!(
            InfectionManager::<Sir>::intrinsic_infectiousness(ctx, person),
            Some(0.75)
        );
        // Constant and function factors compose.
        InfectionManager::<Sir>::set_infectiousness_modifier::<Vaccine>(ctx, person, 0.5);
        assert_eq!(
            InfectionManager::<Sir>::intrinsic_infectiousness(ctx, person),
            Some(0.375)
        );
        InfectionManager::<Sir>::remove_infectiousness_modifier::<Antiviral>(ctx, person);
        assert_eq!(
            InfectionManager::<Sir>::intrinsic_infectiousness(ctx, person),
            Some(0.5)
        );
    });
    context.execute();
    assert_eq!(context.get_current_time(), 3.0);
}

#[test]
fn zero_susceptibility_never_transmits() {
    struct Immune;
    let mut context = Context::new();
    context.init_random(42);
    for _ in 0..POPULATION {
        let person = context.add_entity(Person).unwrap();
        InfectionManager::<Sir>::set_susceptibility_modifier::<Immune>(&mut context, person, 0.0);
    }
    InfectionManager::<Sir>::load_infectiousness_rate_library(
        &mut context,
        vec![ConstantRateFn::new(2.0 / 5.0, 5.0).into()],
    );
    TransmissionManager::<Sir>::setup_transmission_loop(&mut context);
    TransmissionManager::<Sir>::seed_infections(&mut context, Person, 5);
    context.execute();
    let infected = context
        .get_entity_iterator::<Person>()
        .filter(|&p| InfectionManager::<Sir>::infection_time(&context, p).is_some())
        .count();
    assert_eq!(infected, 5);
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

#[test]
#[should_panic(expected = "exceeds its forecast")]
fn loop_panics_when_the_forecast_is_not_a_bound() {
    struct Broken;
    impl InfectionManager<Broken> for Context {
        fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
            RateAssignment::Library(0)
        }
    }
    impl TransmissionManager<Broken> for Context {
        fn forecast_total_infectiousness(&self, person: PersonId) -> impl InvertibleRate {
            // Half the real rate: not a bound.
            ScaledRateFn::new(
                InfectionManager::<Broken>::majorizing_infectiousness_rate_fn(self, person),
                0.5,
            )
        }

        fn evaluate_forecast(&self, person: PersonId, forecast: f64) -> Option<PersonId> {
            let total = InfectionManager::<Broken>::intrinsic_infectiousness(self, person)?;
            TransmissionManager::<Broken>::accept_proposal(self, person, total, forecast);
            None
        }
    }
    let mut context = Context::new();
    context.init_random(3);
    for _ in 0..10 {
        context.add_entity(Person).unwrap();
    }
    InfectionManager::<Broken>::load_infectiousness_rate_library(
        &mut context,
        vec![ConstantRateFn::new(1.0, 5.0).into()],
    );
    TransmissionManager::<Broken>::setup_transmission_loop(&mut context);
    TransmissionManager::<Broken>::seed_infections(&mut context, Person, 1);
    context.execute();
}
