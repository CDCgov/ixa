#![cfg(feature = "core_epi")]

use ixa::core_epi::{
    ConstantRateFn, InfectionManager, InvertibleRate, RateAssignment, ScaledRateFn, StepFn,
    TransmissionManager,
};
use ixa::prelude::*;

define_entity!(Person);

struct Sir;

const POPULATION: usize = 1000;

impl InfectionManager<Sir> for Context {
    type Entity = Person;

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
        type Entity = Person;

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

#[test]
fn models_on_different_entities_keep_separate_infections() {
    define_entity!(Animal);
    struct Zoonosis;
    impl InfectionManager<Zoonosis> for Context {
        type Entity = Animal;

        fn draw_infectiousness_rate_fn(&mut self, _animal: AnimalId) -> RateAssignment {
            ConstantRateFn::new(1.0, 2.0).into()
        }
    }

    let mut context = Context::new();
    context.init_random(1);
    InfectionManager::<Sir>::load_infectiousness_rate_library(
        &mut context,
        vec![ConstantRateFn::new(1.0, 5.0).into()],
    );
    let person = context.add_entity(Person).unwrap();
    let animal = context.add_entity(Animal).unwrap();

    InfectionManager::<Zoonosis>::assign_infectiousness_rate_fn(&mut context, animal);
    assert!(InfectionManager::<Zoonosis>::is_infectious(
        &context, animal
    ));
    assert!(!InfectionManager::<Sir>::is_infectious(&context, person));

    InfectionManager::<Sir>::assign_infectiousness_rate_fn(&mut context, person);
    assert_eq!(
        InfectionManager::<Sir>::infectiousness_end_time(&context, person),
        Some(5.0)
    );
    assert_eq!(
        InfectionManager::<Zoonosis>::infectiousness_end_time(&context, animal),
        Some(2.0)
    );
}

// A model whose contact rate is a factor the test changes mid-run. It counts
// attempts instead of infecting anyone.
struct Boosted;

#[derive(Default)]
struct Boost {
    factor: std::cell::Cell<f64>,
    attempts: std::cell::Cell<usize>,
    reforecast_on_infected: std::cell::Cell<bool>,
}
define_data_plugin!(BoostPlugin, Boost, Boost::default());

const BOOSTED_RATE: f64 = 2.0;

impl InfectionManager<Boosted> for Context {
    type Entity = Person;

    fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
        ConstantRateFn::new(BOOSTED_RATE, 1000.0).into()
    }

    fn on_infected(&mut self, _infector: Option<PersonId>, infectee: PersonId) {
        if self.get_data(BoostPlugin).reforecast_on_infected.get() {
            TransmissionManager::<Boosted>::reforecast_transmission(self, infectee);
        }
    }
}

impl TransmissionManager<Boosted> for Context {
    fn forecast_total_infectiousness(&self, person: PersonId) -> impl InvertibleRate {
        ScaledRateFn::new(
            InfectionManager::<Boosted>::majorizing_infectiousness_rate_fn(self, person),
            self.get_data(BoostPlugin).factor.get(),
        )
    }

    fn evaluate_forecast(&self, person: PersonId, forecast: f64) -> Option<PersonId> {
        let boost = self.get_data(BoostPlugin);
        let total = InfectionManager::<Boosted>::intrinsic_infectiousness(self, person)?
            * boost.factor.get();
        if TransmissionManager::<Boosted>::accept_proposal(self, person, total, forecast) {
            boost.attempts.set(boost.attempts.get() + 1);
        }
        None
    }
}

/// One infector whose factor is 0 until t = 10, 1 until t = 30, then 3, run
/// to t = 60; returns the number of attempts.
fn run_boosted(seed: u64, reforecast: bool) -> usize {
    let mut context = Context::new();
    context.init_random(seed);
    let person = context.add_entity(Person).unwrap();
    TransmissionManager::<Boosted>::setup_transmission_loop(&mut context);
    TransmissionManager::<Boosted>::transmit(&mut context, None, person);
    for (time, factor) in [(10.0, 1.0), (30.0, 3.0)] {
        context.add_plan(time, move |context| {
            context.get_data(BoostPlugin).factor.set(factor);
            if reforecast {
                TransmissionManager::<Boosted>::reforecast_transmission(context, person);
            }
        });
    }
    context.add_plan(60.0, Context::shutdown);
    context.execute();
    context.get_data(BoostPlugin).attempts.get()
}

#[test]
fn reforecast_restarts_an_ended_loop_and_follows_a_rise() {
    // The loop ends at once on a zero forecast; each reforecast picks the new
    // rate up. Expected attempts: 2 * 20 + 6 * 30 = 220 per run.
    let runs = 50;
    let total: usize = (0..runs).map(|seed| run_boosted(seed, true)).sum();
    let mean = total as f64 / runs as f64;
    assert!(
        (mean - 220.0).abs() < 8.0,
        "expected about 220 attempts per run, got {mean}"
    );
}

#[test]
fn without_a_reforecast_a_zero_forecast_ends_the_loop() {
    assert_eq!(run_boosted(1, false), 0);
}

#[test]
#[should_panic(expected = "exceeds its forecast")]
fn a_rise_without_a_reforecast_panics() {
    let mut context = Context::new();
    context.init_random(2);
    let person = context.add_entity(Person).unwrap();
    context.get_data(BoostPlugin).factor.set(1.0);
    TransmissionManager::<Boosted>::setup_transmission_loop(&mut context);
    TransmissionManager::<Boosted>::transmit(&mut context, None, person);
    context.add_plan(10.0, |context| {
        context.get_data(BoostPlugin).factor.set(3.0);
    });
    context.add_plan(60.0, Context::shutdown);
    context.execute();
}

#[test]
fn reinfection_and_reforecast_leave_one_loop_running() {
    // Fifty reinfections and fifty reforecasts at t = 0 must not leave a
    // hundred loops: expected attempts stay 2 * 10 = 20 per run.
    let runs = 50;
    let mut total = 0;
    for seed in 0..runs {
        let mut context = Context::new();
        context.init_random(seed);
        let person = context.add_entity(Person).unwrap();
        context.get_data(BoostPlugin).factor.set(1.0);
        TransmissionManager::<Boosted>::setup_transmission_loop(&mut context);
        for _ in 0..50 {
            TransmissionManager::<Boosted>::transmit(&mut context, None, person);
        }
        context.add_plan(0.0, move |context| {
            for _ in 0..50 {
                TransmissionManager::<Boosted>::reforecast_transmission(context, person);
            }
        });
        context.add_plan(10.0, Context::shutdown);
        context.execute();
        total += context.get_data(BoostPlugin).attempts.get();
    }
    let mean = total as f64 / runs as f64;
    assert!(
        (mean - 20.0).abs() < 3.0,
        "expected about 20 attempts per run, got {mean}"
    );
}

#[test]
fn reforecast_is_a_no_op_for_someone_never_infected() {
    let mut context = Context::new();
    context.init_random(1);
    let person = context.add_entity(Person).unwrap();
    TransmissionManager::<Boosted>::reforecast_transmission(&mut context, person);
    context.execute();
    assert_eq!(context.get_data(BoostPlugin).attempts.get(), 0);
}

/// Mean attempts per run over ten time units at factor 1 (expected 20), after
/// `infect` has infected the one person at time zero.
fn mean_boosted_attempts(infect: impl Fn(&mut Context, PersonId)) -> f64 {
    let runs = 50;
    let mut total = 0;
    for seed in 0..runs {
        let mut context = Context::new();
        context.init_random(seed);
        let person = context.add_entity(Person).unwrap();
        context.get_data(BoostPlugin).factor.set(1.0);
        TransmissionManager::<Boosted>::setup_transmission_loop(&mut context);
        infect(&mut context, person);
        context.add_plan(10.0, Context::shutdown);
        context.execute();
        total += context.get_data(BoostPlugin).attempts.get();
    }
    total as f64 / runs as f64
}

#[test]
fn repeated_infection_before_the_loop_starts_leaves_one_loop_running() {
    let mean = mean_boosted_attempts(|context, person| {
        for _ in 0..50 {
            TransmissionManager::<Boosted>::transmit(context, None, person);
        }
    });
    assert!(
        (mean - 20.0).abs() < 3.0,
        "expected about 20 attempts per run, got {mean}"
    );
}

#[test]
fn reforecast_in_on_infected_leaves_one_loop_running() {
    let mean = mean_boosted_attempts(|context, person| {
        context
            .get_data(BoostPlugin)
            .reforecast_on_infected
            .set(true);
        TransmissionManager::<Boosted>::transmit(context, None, person);
    });
    assert!(
        (mean - 20.0).abs() < 3.0,
        "expected about 20 attempts per run, got {mean}"
    );
}

#[test]
fn reforecast_after_infectiousness_ended_schedules_nothing() {
    let mut context = Context::new();
    context.init_random(4);
    let person = context.add_entity(Person).unwrap();
    InfectionManager::<Sir>::load_infectiousness_rate_library(
        &mut context,
        vec![ConstantRateFn::new(1.0, 2.0).into()],
    );
    InfectionManager::<Sir>::assign_infectiousness_rate_fn(&mut context, person);
    context.add_plan(5.0, move |context| {
        TransmissionManager::<Sir>::reforecast_transmission(context, person);
    });
    context.execute();
    // Nothing was scheduled past the reforecast itself.
    assert_eq!(context.get_current_time(), 5.0);
}
