#![cfg(feature = "core_epi")]

use std::cell::Cell;

use ixa::core_epi::{
    ConstantRateFn, InfectionManager, InvertibleRate, Itinerary, RateAssignment, ScaledRateFn,
    SettingKind, SettingsExt, StepFn, TransmissionManager, MAX_SETTINGS,
};
use ixa::prelude::*;

define_entity!(Person);

struct Sir;

const POPULATION: usize = 1000;

impl InfectionManager<Sir> for Context {
    type Entity = Person;
    type Exposure = ();

    fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
        RateAssignment::Library(0)
    }
}

// Homogeneous mixing: all defaults.
impl TransmissionManager<Sir> for Context {}

/// A context with `people` people and `Sir`'s library holding one constant
/// rate function.
fn sir_context(seed: u64, people: usize, rate: f64, duration: f64) -> (Context, Vec<PersonId>) {
    let mut context = Context::new();
    context.init_random(seed);
    let people = (0..people)
        .map(|_| context.add_entity(Person).unwrap())
        .collect();
    InfectionManager::<Sir>::load_infectiousness_rate_library(
        &mut context,
        vec![ConstantRateFn::new(rate, duration).into()],
    );
    (context, people)
}

fn sir_infected_count(context: &Context) -> usize {
    context
        .get_entity_iterator::<Person>()
        .filter(|&p| InfectionManager::<Sir>::infection_time(context, p).is_some())
        .count()
}

/// Final size of an outbreak seeded with five infections.
fn run(r0: f64) -> usize {
    let (mut context, _) = sir_context(42, POPULATION, r0 / 5.0, 5.0);
    TransmissionManager::<Sir>::setup_transmission_loop(&mut context);
    TransmissionManager::<Sir>::seed_infections(&mut context, Person, 5);
    context.execute();
    sir_infected_count(&context)
}

const RUNS: u64 = 50;

/// Mean of `attempts(seed)` over `RUNS` seeds.
fn mean_attempts(attempts: impl Fn(u64) -> usize) -> f64 {
    let mut total = 0;
    for seed in 0..RUNS {
        total += attempts(seed);
    }
    total as f64 / RUNS as f64
}

fn assert_about(mean: f64, expected: f64, tolerance: f64) {
    assert!(
        (mean - expected).abs() < tolerance,
        "expected about {expected} attempts per run, got {mean}"
    );
}

#[test]
fn outbreak_spreads_and_ends() {
    // Final size for R0 = 2 is ~80%; `execute` returning means every loop ended.
    let infected = run(2.0);
    assert!(
        (700..=900).contains(&infected),
        "unexpected final size {infected}"
    );
}

#[test]
fn susceptibility_modifiers_compose_and_persist_across_infection() {
    struct Vaccine;
    struct Age;
    let (mut context, people) = sir_context(1, 1, 1.0, 5.0);
    let person = people[0];
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
    let (mut context, people) = sir_context(1, 1, 1.0, 10.0);
    let person = people[0];
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
        // Treatment effect that ramps in over its first two days, reading
        // the context to check it is the one passed in.
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
    let (mut context, people) = sir_context(42, POPULATION, 2.0 / 5.0, 5.0);
    for person in people {
        InfectionManager::<Sir>::set_susceptibility_modifier::<Immune>(&mut context, person, 0.0);
    }
    TransmissionManager::<Sir>::setup_transmission_loop(&mut context);
    TransmissionManager::<Sir>::seed_infections(&mut context, Person, 5);
    context.execute();
    assert_eq!(sir_infected_count(&context), 5);
}

#[test]
#[should_panic(expected = "is not in [0, 1]")]
fn susceptibility_above_one_panics() {
    struct Frail;
    let (mut context, people) = sir_context(42, 10, 2.0, 5.0);
    for person in people {
        InfectionManager::<Sir>::set_susceptibility_modifier::<Frail>(&mut context, person, 1.5);
    }
    TransmissionManager::<Sir>::setup_transmission_loop(&mut context);
    TransmissionManager::<Sir>::seed_infections(&mut context, Person, 1);
    context.execute();
}

#[test]
fn zero_infectiousness_never_transmits() {
    let infected = run(0.0);
    assert_eq!(infected, 5);
}

#[test]
fn infectiousness_modifier_belongs_to_the_person() {
    struct Mask;
    let (mut context, people) = sir_context(1, 1, 1.0, 5.0);
    let person = people[0];
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
    // It survives reinfection.
    InfectionManager::<Sir>::set_infectiousness_modifier::<Mask>(&mut context, person, 0.5);
    InfectionManager::<Sir>::assign_infectiousness_rate_fn(&mut context, person);
    assert_eq!(
        InfectionManager::<Sir>::intrinsic_infectiousness(&context, person),
        Some(0.5)
    );
    // And can be set before someone is ever infected.
    let other = context.add_entity(Person).unwrap();
    InfectionManager::<Sir>::set_infectiousness_modifier::<Mask>(&mut context, other, 0.25);
    assert_eq!(
        InfectionManager::<Sir>::intrinsic_infectiousness(&context, other),
        None
    );
    InfectionManager::<Sir>::assign_infectiousness_rate_fn(&mut context, other);
    assert_eq!(
        InfectionManager::<Sir>::intrinsic_infectiousness(&context, other),
        Some(0.25)
    );
}

#[test]
fn an_infectiousness_modifier_above_one_raises_the_rate_and_its_bound() {
    struct Boost;
    struct Mask;
    let (mut context, people) = sir_context(1, 1, 1.0, 5.0);
    let person = people[0];
    let bound = |context: &Context| {
        InfectionManager::<Sir>::majorizing_infectiousness_rate_fn(context, person).rate(0.0)
    };
    // Set before infection, it is in the first forecast.
    InfectionManager::<Sir>::set_infectiousness_modifier::<Boost>(&mut context, person, 3.0);
    InfectionManager::<Sir>::assign_infectiousness_rate_fn(&mut context, person);
    assert_eq!(
        InfectionManager::<Sir>::intrinsic_infectiousness(&context, person),
        Some(3.0)
    );
    assert_eq!(bound(&context), 3.0);
    // A factor below one lowers the rate; the bound follows the product
    // down but never below the unmodified rate.
    InfectionManager::<Sir>::set_infectiousness_modifier::<Mask>(&mut context, person, 0.5);
    assert_eq!(
        InfectionManager::<Sir>::intrinsic_infectiousness(&context, person),
        Some(1.5)
    );
    assert_eq!(bound(&context), 1.5);
    InfectionManager::<Sir>::remove_infectiousness_modifier::<Boost>(&mut context, person);
    assert_eq!(bound(&context), 1.0);
    // Removing the mask alone raises it again.
    InfectionManager::<Sir>::set_infectiousness_modifier::<Boost>(&mut context, person, 3.0);
    InfectionManager::<Sir>::remove_infectiousness_modifier::<Mask>(&mut context, person);
    assert_eq!(bound(&context), 3.0);
}

fn run_with_a_boost(boost_time: Option<f64>) -> usize {
    struct Boost;
    // R0 of 0.5 unmodified: the outbreak dies out unless boosted.
    let (mut context, _) = sir_context(42, POPULATION, 0.1, 5.0);
    TransmissionManager::<Sir>::setup_transmission_loop(&mut context);
    TransmissionManager::<Sir>::seed_infections(&mut context, Person, 20);
    if let Some(boost_time) = boost_time {
        // Everyone, including the people infectious at the time, whose
        // pending proposals were drawn at the unmodified rate.
        context.add_plan(boost_time, |ctx| {
            let people: Vec<PersonId> = ctx.get_entity_iterator::<Person>().collect();
            for person in people {
                InfectionManager::<Sir>::set_infectiousness_modifier::<Boost>(ctx, person, 5.0);
            }
        });
    }
    context.execute();
    sir_infected_count(&context)
}

#[test]
fn raising_an_infectiousness_modifier_mid_infection_reforecasts() {
    let unboosted = run_with_a_boost(None);
    assert!(unboosted < 100, "unexpected final size {unboosted}");
    // R0 of 2.5 from day 1 on. Without the reforecast the loop panics on
    // the first proposal drawn before the boost.
    let boosted = run_with_a_boost(Some(1.0));
    assert!(boosted > 700, "unexpected final size {boosted}");
}

#[test]
fn attempts_follow_infectiousness_modifiers_through_every_kind_of_change() {
    struct Boost;
    struct Mask;
    struct Surge;
    struct Antiviral;
    struct Wave;
    // The modifier product over each ten days, with the change that starts it.
    // Expected attempts at rate 2:
    // 2 * (10 * (2 + 1 + 2 + 1 + 1 + 3 + 1 + 1 + 4 + 2 + 4) + 5 * (1 + 3)) = 480.
    let mean = mean_attempts(|seed| {
        let mut context = Context::new();
        context.init_random(seed);
        let person = context.add_entity(Person).unwrap();
        TransmissionManager::<Counting>::setup_transmission_loop(&mut context);
        // 2: a boost set before infection.
        InfectionManager::<Counting>::set_infectiousness_modifier::<Boost>(
            &mut context,
            person,
            2.0,
        );
        TransmissionManager::<Counting>::seed_infection(&mut context, person);
        // 1: a mask.
        context.add_plan(10.0, move |ctx| {
            InfectionManager::<Counting>::set_infectiousness_modifier::<Mask>(ctx, person, 0.5);
        });
        // 2: the mask replaced by a function, which raises the bound.
        context.add_plan(20.0, move |ctx| {
            InfectionManager::<Counting>::set_infectiousness_modifier_fn::<Mask>(
                ctx,
                person,
                |_, _, _| 1.0,
            );
        });
        // 1: the boost removed.
        context.add_plan(30.0, move |ctx| {
            InfectionManager::<Counting>::remove_infectiousness_modifier::<Boost>(ctx, person);
        });
        // 1, then 3 from day 50 to 60, then 1: a scheduled factor above one.
        context.add_plan(40.0, move |ctx| {
            InfectionManager::<Counting>::schedule_infectiousness_modifier::<Surge>(
                ctx,
                person,
                3.0,
                50.0,
                Some(60.0),
            );
        });
        // 1: a boost of 4 cancelled by a scheduled antiviral.
        context.add_plan(65.0, move |ctx| {
            InfectionManager::<Counting>::schedule_infectiousness_modifier::<Antiviral>(
                ctx, person, 0.25, 70.0, None,
            );
        });
        context.add_plan(70.0, move |ctx| {
            InfectionManager::<Counting>::set_infectiousness_modifier::<Boost>(ctx, person, 4.0);
        });
        // 4: reinfection drops the antiviral.
        context.add_plan(80.0, move |ctx| {
            TransmissionManager::<Counting>::seed_infection(ctx, person);
        });
        // 2: a mask again.
        context.add_plan(90.0, move |ctx| {
            InfectionManager::<Counting>::set_infectiousness_modifier::<Mask>(ctx, person, 0.5);
        });
        // 4: the mask removed, which raises the bound.
        context.add_plan(100.0, move |ctx| {
            InfectionManager::<Counting>::remove_infectiousness_modifier::<Mask>(ctx, person);
        });
        // 1 for five days then 3: a function that rises under its declared
        // maximum, with nothing to announce the rise.
        context.add_plan(110.0, move |ctx| {
            InfectionManager::<Counting>::remove_infectiousness_modifier::<Boost>(ctx, person);
            InfectionManager::<Counting>::set_infectiousness_modifier_fn_with_max::<Wave>(
                ctx,
                person,
                3.0,
                |_, _, elapsed| if elapsed < 5.0 { 1.0 } else { 3.0 },
            );
        });
        context.add_plan(120.0, Context::shutdown);
        context.execute();
        context.get_data(CountingPlugin).attempts.get()
    });
    assert_about(mean, 480.0, 12.0);
}

#[test]
#[should_panic(expected = "multiply to inf")]
fn infectiousness_modifiers_overflowing_panic() {
    struct First;
    struct Second;
    let (mut context, people) = sir_context(1, 1, 1.0, 5.0);
    let person = people[0];
    InfectionManager::<Sir>::assign_infectiousness_rate_fn(&mut context, person);
    InfectionManager::<Sir>::set_infectiousness_modifier::<First>(&mut context, person, 1e200);
    InfectionManager::<Sir>::set_infectiousness_modifier::<Second>(&mut context, person, 1e200);
}

#[test]
#[should_panic(expected = "must be finite and non-negative")]
fn a_negative_infectiousness_modifier_panics() {
    struct Boost;
    let mut context = Context::new();
    let person = context.add_entity(Person).unwrap();
    InfectionManager::<Sir>::set_infectiousness_modifier::<Boost>(&mut context, person, -0.5);
}

#[test]
fn infectiousness_modifier_fn_measures_time_since_it_was_set() {
    struct Mask;
    let (mut context, people) = sir_context(1, 1, 1.0, 10.0);
    let person = people[0];
    // Set a day before the infection: elapsed time runs from the set.
    context.add_plan(1.0, move |ctx| {
        InfectionManager::<Sir>::set_infectiousness_modifier_fn::<Mask>(ctx, person, |_, _, t| {
            1.0 - 0.25 * t.min(2.0)
        });
    });
    context.add_plan(2.0, move |ctx| {
        InfectionManager::<Sir>::assign_infectiousness_rate_fn(ctx, person);
        assert_eq!(
            InfectionManager::<Sir>::intrinsic_infectiousness(ctx, person),
            Some(0.75)
        );
    });
    context.add_plan(3.0, move |ctx| {
        assert_eq!(
            InfectionManager::<Sir>::intrinsic_infectiousness(ctx, person),
            Some(0.5)
        );
    });
    context.execute();
    assert_eq!(context.get_current_time(), 3.0);
}

#[test]
fn a_scheduled_infectiousness_modifier_ends_with_its_infection() {
    struct Mask;
    struct Antiviral;
    let (mut context, people) = sir_context(1, 1, 1.0, 10.0);
    let person = people[0];
    InfectionManager::<Sir>::assign_infectiousness_rate_fn(&mut context, person);
    InfectionManager::<Sir>::set_infectiousness_modifier::<Mask>(&mut context, person, 0.5);
    InfectionManager::<Sir>::schedule_infectiousness_modifier::<Antiviral>(
        &mut context,
        person,
        0.5,
        1.0,
        None,
    );
    context.add_plan(2.0, move |ctx| {
        assert_eq!(
            InfectionManager::<Sir>::infectiousness_multiplier(ctx, person),
            0.25
        );
        // Reinfection drops the scheduled modifier and keeps the person's.
        InfectionManager::<Sir>::assign_infectiousness_rate_fn(ctx, person);
        assert_eq!(
            InfectionManager::<Sir>::infectiousness_multiplier(ctx, person),
            0.5
        );
    });
    context.execute();
}

#[test]
#[should_panic(expected = "exceeds its forecast")]
fn loop_panics_when_the_forecast_is_not_a_bound() {
    struct Broken;
    impl InfectionManager<Broken> for Context {
        type Entity = Person;
        type Exposure = ();

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

        fn evaluate_forecast(&self, person: PersonId, forecast: f64) -> Option<(PersonId, ())> {
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
        type Exposure = ();

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

// One infector at rate 2 whose attempts are counted and never transmit, with a
// contact multiplier and an optional bound the test changes mid-run.
struct Counting;

struct CountingState {
    multiplier: Cell<f64>,
    /// `None` leaves the default bound: the multiplier now, so each rise
    /// needs a reforecast.
    bound: Cell<Option<f64>>,
    attempts: Cell<usize>,
    reforecast_on_infected: Cell<bool>,
}

impl Default for CountingState {
    fn default() -> Self {
        Self {
            multiplier: Cell::new(1.0),
            bound: Cell::new(None),
            attempts: Cell::new(0),
            reforecast_on_infected: Cell::new(false),
        }
    }
}
define_data_plugin!(CountingPlugin, CountingState, CountingState::default());

const COUNTING_RATE: f64 = 2.0;

impl InfectionManager<Counting> for Context {
    type Entity = Person;
    type Exposure = ();

    fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
        ConstantRateFn::new(COUNTING_RATE, 1000.0).into()
    }

    fn on_infected(&mut self, _infector: Option<PersonId>, infectee: PersonId, _exposure: ()) {
        if self.get_data(CountingPlugin).reforecast_on_infected.get() {
            TransmissionManager::<Counting>::reforecast_transmission(self, infectee);
        }
    }
}

impl TransmissionManager<Counting> for Context {
    fn contact_multiplier(&self, _person: PersonId) -> f64 {
        self.get_data(CountingPlugin).multiplier.get()
    }

    fn max_contact_multiplier(&self, _person: PersonId) -> f64 {
        let state = self.get_data(CountingPlugin);
        state.bound.get().unwrap_or(state.multiplier.get())
    }

    // Reached once per accepted proposal, i.e. per attempt.
    fn sample_contact(&self, _person: PersonId) -> Option<(PersonId, ())> {
        let state = self.get_data(CountingPlugin);
        state.attempts.set(state.attempts.get() + 1);
        None
    }
}

/// One infector whose factor is 0 until t = 10, 1 until t = 30, then 3, run
/// to t = 60; returns the number of attempts.
fn run_boosted(seed: u64, reforecast: bool) -> usize {
    let mut context = Context::new();
    context.init_random(seed);
    let person = context.add_entity(Person).unwrap();
    context.get_data(CountingPlugin).multiplier.set(0.0);
    TransmissionManager::<Counting>::setup_transmission_loop(&mut context);
    TransmissionManager::<Counting>::seed_infection(&mut context, person);
    for (time, factor) in [(10.0, 1.0), (30.0, 3.0)] {
        context.add_plan(time, move |context| {
            context.get_data(CountingPlugin).multiplier.set(factor);
            if reforecast {
                TransmissionManager::<Counting>::reforecast_transmission(context, person);
            }
        });
    }
    context.add_plan(60.0, Context::shutdown);
    context.execute();
    context.get_data(CountingPlugin).attempts.get()
}

#[test]
fn reforecast_restarts_an_ended_loop_and_follows_a_rise() {
    // The loop ends at once on a zero forecast; each reforecast picks the new
    // rate up. Expected attempts: 2 * 20 + 6 * 30 = 220 per run.
    let mean = mean_attempts(|seed| run_boosted(seed, true));
    assert_about(mean, 220.0, 8.0);
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
    TransmissionManager::<Counting>::setup_transmission_loop(&mut context);
    TransmissionManager::<Counting>::seed_infection(&mut context, person);
    context.add_plan(10.0, |context| {
        context.get_data(CountingPlugin).multiplier.set(3.0);
    });
    context.add_plan(60.0, Context::shutdown);
    context.execute();
}

#[test]
fn reinfection_and_reforecast_leave_one_loop_running() {
    // Fifty reinfections and fifty reforecasts at t = 0 must not leave a
    // hundred loops: expected attempts stay 2 * 10 = 20 per run.
    let mean = mean_attempts(|seed| {
        let mut context = Context::new();
        context.init_random(seed);
        let person = context.add_entity(Person).unwrap();
        TransmissionManager::<Counting>::setup_transmission_loop(&mut context);
        for _ in 0..50 {
            TransmissionManager::<Counting>::seed_infection(&mut context, person);
        }
        context.add_plan(0.0, move |context| {
            for _ in 0..50 {
                TransmissionManager::<Counting>::reforecast_transmission(context, person);
            }
        });
        context.add_plan(10.0, Context::shutdown);
        context.execute();
        context.get_data(CountingPlugin).attempts.get()
    });
    assert_about(mean, 20.0, 3.0);
}

#[test]
fn reforecast_is_a_no_op_for_someone_never_infected() {
    let mut context = Context::new();
    context.init_random(1);
    let person = context.add_entity(Person).unwrap();
    TransmissionManager::<Counting>::reforecast_transmission(&mut context, person);
    context.execute();
    assert_eq!(context.get_data(CountingPlugin).attempts.get(), 0);
}

/// Mean attempts per run over ten time units at factor 1 (expected 20), after
/// `infect` has infected the one person at time zero.
fn mean_boosted_attempts(infect: impl Fn(&mut Context, PersonId)) -> f64 {
    mean_attempts(|seed| {
        let mut context = Context::new();
        context.init_random(seed);
        let person = context.add_entity(Person).unwrap();
        TransmissionManager::<Counting>::setup_transmission_loop(&mut context);
        infect(&mut context, person);
        context.add_plan(10.0, Context::shutdown);
        context.execute();
        context.get_data(CountingPlugin).attempts.get()
    })
}

#[test]
fn repeated_infection_before_the_loop_starts_leaves_one_loop_running() {
    let mean = mean_boosted_attempts(|context, person| {
        for _ in 0..50 {
            TransmissionManager::<Counting>::seed_infection(context, person);
        }
    });
    assert_about(mean, 20.0, 3.0);
}

#[test]
fn reforecast_in_on_infected_leaves_one_loop_running() {
    let mean = mean_boosted_attempts(|context, person| {
        context
            .get_data(CountingPlugin)
            .reforecast_on_infected
            .set(true);
        TransmissionManager::<Counting>::seed_infection(context, person);
    });
    assert_about(mean, 20.0, 3.0);
}

#[test]
fn reforecast_after_infectiousness_ended_schedules_nothing() {
    let (mut context, people) = sir_context(4, 1, 1.0, 2.0);
    let person = people[0];
    InfectionManager::<Sir>::assign_infectiousness_rate_fn(&mut context, person);
    context.add_plan(5.0, move |context| {
        TransmissionManager::<Sir>::reforecast_transmission(context, person);
    });
    context.execute();
    // Nothing was scheduled past the reforecast itself.
    assert_eq!(context.get_current_time(), 5.0);
}

#[test]
fn infectiousness_duration_is_the_span_of_the_rate_function() {
    let mut context = Context::new();
    context.init_random(4);
    let person = context.add_entity(Person).unwrap();
    assert_eq!(
        InfectionManager::<Sir>::infectiousness_duration(&context, person),
        None
    );
    InfectionManager::<Sir>::load_infectiousness_rate_library(
        &mut context,
        vec![ConstantRateFn::new(1.0, 2.5).into()],
    );
    context.add_plan(3.0, move |context| {
        InfectionManager::<Sir>::assign_infectiousness_rate_fn(context, person);
    });
    context.execute();
    assert_eq!(
        InfectionManager::<Sir>::infectiousness_duration(&context, person),
        Some(2.5)
    );
    assert_eq!(
        InfectionManager::<Sir>::infectiousness_end_time(&context, person),
        Some(5.5)
    );
}

#[test]
fn a_rate_function_can_be_set_directly() {
    // No library and no draw: the infection's course is given.
    let mut context = Context::new();
    context.init_random(4);
    let person = context.add_entity(Person).unwrap();
    InfectionManager::<Sir>::set_infectiousness_rate_fn(
        &mut context,
        person,
        ConstantRateFn::new(0.5, 4.0),
    );
    assert!(InfectionManager::<Sir>::is_infectious(&context, person));
    assert_eq!(
        InfectionManager::<Sir>::infectiousness_duration(&context, person),
        Some(4.0)
    );
    assert_eq!(
        InfectionManager::<Sir>::intrinsic_infectiousness(&context, person),
        Some(0.5)
    );
}

#[test]
#[should_panic(expected = "is not in the library of 0")]
fn a_library_index_outside_the_library_panics_at_assignment() {
    let mut context = Context::new();
    context.init_random(4);
    let person = context.add_entity(Person).unwrap();
    // `Sir` draws library index 0, and no library was loaded.
    InfectionManager::<Sir>::assign_infectiousness_rate_fn(&mut context, person);
}

#[test]
fn two_models_on_one_population_keep_separate_infections() {
    use ixa::core_epi::InfectiousnessAssignedEvent;

    struct Flu;
    struct Rsv;
    impl InfectionManager<Flu> for Context {
        type Entity = Person;
        type Exposure = ();

        fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
            RateAssignment::Library(0)
        }
    }
    impl InfectionManager<Rsv> for Context {
        type Entity = Person;
        type Exposure = ();

        fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
            RateAssignment::Library(0)
        }
    }
    impl TransmissionManager<Flu> for Context {}
    impl TransmissionManager<Rsv> for Context {}

    #[derive(Default)]
    struct Assigned {
        flu: usize,
        rsv: usize,
    }
    define_data_plugin!(AssignedPlugin, Assigned, Assigned::default());

    let mut context = Context::new();
    context.init_random(8);
    let people: Vec<PersonId> = (0..200)
        .map(|_| context.add_entity(Person).unwrap())
        .collect();
    // Each model has its own library.
    InfectionManager::<Flu>::load_infectiousness_rate_library(
        &mut context,
        vec![ConstantRateFn::new(1.0, 3.0).into()],
    );
    InfectionManager::<Rsv>::load_infectiousness_rate_library(
        &mut context,
        vec![ConstantRateFn::new(0.0, 7.0).into()],
    );
    context.subscribe_to_event(|context, _event: InfectiousnessAssignedEvent<Flu>| {
        context.get_data_mut(AssignedPlugin).flu += 1;
    });
    context.subscribe_to_event(|context, _event: InfectiousnessAssignedEvent<Rsv>| {
        context.get_data_mut(AssignedPlugin).rsv += 1;
    });
    TransmissionManager::<Flu>::setup_transmission_loop(&mut context);
    TransmissionManager::<Rsv>::setup_transmission_loop(&mut context);

    TransmissionManager::<Flu>::seed_infection(&mut context, people[0]);
    TransmissionManager::<Rsv>::seed_infection(&mut context, people[1]);
    assert_eq!(
        InfectionManager::<Flu>::infectiousness_duration(&context, people[0]),
        Some(3.0)
    );
    assert_eq!(
        InfectionManager::<Rsv>::infectiousness_duration(&context, people[0]),
        None
    );
    assert_eq!(
        InfectionManager::<Rsv>::infectiousness_duration(&context, people[1]),
        Some(7.0)
    );
    // Immunity is per model: the RSV case is still susceptible to flu.
    assert_eq!(
        InfectionManager::<Flu>::susceptibility(&context, people[1]),
        1.0
    );
    InfectionManager::<Flu>::set_susceptibility_modifier::<Flu>(&mut context, people[2], 0.0);
    assert_eq!(
        InfectionManager::<Rsv>::susceptibility(&context, people[2]),
        1.0
    );

    context.execute();

    // Flu (R0 3) spreads; RSV has a rate of zero and stays with its seed.
    let mut flu_infected = 0;
    let mut rsv_infected = 0;
    for &person in &people {
        if InfectionManager::<Flu>::infection_time(&context, person).is_some() {
            flu_infected += 1;
        }
        if InfectionManager::<Rsv>::infection_time(&context, person).is_some() {
            rsv_infected += 1;
        }
    }
    assert!(flu_infected > 50, "flu infected {flu_infected}");
    assert_eq!(rsv_infected, 1);
    let assigned = context.get_data(AssignedPlugin);
    assert_eq!(assigned.flu, flu_infected);
    assert_eq!(assigned.rsv, 1);
}

// A model whose every attempt infects the other person, and whose
// `on_infected` reforecasts the infector from inside their own proposal.
struct Chained;

#[derive(Default)]
struct Chain {
    infector: Cell<Option<PersonId>>,
    contact: Cell<Option<PersonId>>,
    attempts: Cell<usize>,
}
define_data_plugin!(ChainPlugin, Chain, Chain::default());

impl InfectionManager<Chained> for Context {
    type Entity = Person;
    type Exposure = ();

    fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
        ConstantRateFn::new(COUNTING_RATE, 1000.0).into()
    }

    fn base_susceptibility(&self, _person: PersonId) -> f64 {
        1.0
    }

    fn on_infected(&mut self, infector: Option<PersonId>, _infectee: PersonId, _exposure: ()) {
        if let Some(infector) = infector {
            TransmissionManager::<Chained>::reforecast_transmission(self, infector);
        }
    }
}

impl TransmissionManager<Chained> for Context {
    fn evaluate_forecast(&self, person: PersonId, forecast: f64) -> Option<(PersonId, ())> {
        let chain = self.get_data(ChainPlugin);
        // Only the first infector attempts; the contact's own loop is idle.
        if chain.infector.get() != Some(person) {
            return None;
        }
        let total = InfectionManager::<Chained>::intrinsic_infectiousness(self, person)?;
        if !TransmissionManager::<Chained>::accept_proposal(self, person, total, forecast) {
            return None;
        }
        chain.attempts.set(chain.attempts.get() + 1);
        chain.contact.get().map(|contact| (contact, ()))
    }
}

#[test]
fn reforecasting_the_infector_from_on_infected_leaves_one_loop_running() {
    // Expected attempts: 2 * 10 = 20 per run, each one a transmission.
    let mean = mean_attempts(|seed| {
        let mut context = Context::new();
        context.init_random(seed);
        let infector = context.add_entity(Person).unwrap();
        let contact = context.add_entity(Person).unwrap();
        let chain = context.get_data(ChainPlugin);
        chain.infector.set(Some(infector));
        chain.contact.set(Some(contact));
        TransmissionManager::<Chained>::setup_transmission_loop(&mut context);
        TransmissionManager::<Chained>::seed_infection(&mut context, infector);
        context.add_plan(10.0, Context::shutdown);
        context.execute();
        context.get_data(ChainPlugin).attempts.get()
    });
    assert_about(mean, 20.0, 3.0);
}

// Settings through the loop: one infector, counting attempts per setting on
// the setting-first path and on the exact path.
ixa::define_settings!(Place for Person);

const PLACE_RATE: f64 = 0.5;
const PLACE_DURATION: f64 = 2000.0;

#[derive(Default)]
struct PlaceAttempts {
    by_kind: Cell<[usize; MAX_SETTINGS]>,
    /// Whether `Exact` forecasts with the bound that survives membership
    /// changes instead of the multiplier itself.
    bounded: Cell<bool>,
}
define_data_plugin!(PlaceAttemptsPlugin, PlaceAttempts, PlaceAttempts::default());

fn count_place_attempt(context: &Context, person: PersonId, place: Place) {
    if context.sample_setting_contact(person, place).is_some() {
        let attempts = &context.get_data(PlaceAttemptsPlugin).by_kind;
        let mut by_kind = attempts.get();
        by_kind[place.index()] += 1;
        attempts.set(by_kind);
    }
}

struct SettingFirst;
struct Exact;

impl InfectionManager<SettingFirst> for Context {
    type Entity = Person;
    type Exposure = ();

    fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
        ConstantRateFn::new(PLACE_RATE, PLACE_DURATION).into()
    }
}

impl TransmissionManager<SettingFirst> for Context {
    fn max_contact_multiplier(&self, person: PersonId) -> f64 {
        self.max_setting_multiplier::<Place>(person)
    }

    fn evaluate_forecast(&self, person: PersonId, forecast: f64) -> Option<(PersonId, ())> {
        let place = self.sample_setting::<Place>(person)?;
        let total = InfectionManager::<SettingFirst>::intrinsic_infectiousness(self, person)?
            * self.setting_multiplier(person, place);
        if TransmissionManager::<SettingFirst>::accept_proposal(self, person, total, forecast) {
            count_place_attempt(self, person, place);
        }
        None
    }
}

impl InfectionManager<Exact> for Context {
    type Entity = Person;
    type Exposure = ();

    fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
        ConstantRateFn::new(PLACE_RATE, PLACE_DURATION).into()
    }
}

impl TransmissionManager<Exact> for Context {
    fn contact_multiplier(&self, person: PersonId) -> f64 {
        self.current_setting_multiplier::<Place>(person)
    }

    // Unbounded, the forecast is the multiplier itself and no proposal is
    // rejected.
    fn max_contact_multiplier(&self, person: PersonId) -> f64 {
        if self.get_data(PlaceAttemptsPlugin).bounded.get() {
            self.max_setting_multiplier::<Place>(person)
        } else {
            self.current_setting_multiplier::<Place>(person)
        }
    }

    fn sample_contact(&self, person: PersonId) -> Option<(PersonId, ())> {
        let place = self.sample_setting_by_rate::<Place>(person)?;
        count_place_attempt(self, person, place);
        None
    }
}

/// Thirty people; the infector is at home with 3 others (multiplier 3), at
/// work with 16 others (alpha 0.5, multiplier 4), in the community
/// (multiplier 1) and has no school. Returns the infector.
fn place_population(context: &mut Context) -> PersonId {
    let home = context.add_setting_kind::<Place>("home", 1.0);
    let school = context.add_setting_kind::<Place>("school", 1.0);
    let work = context.add_setting_kind::<Place>("work", 0.5);
    let community = context.add_setting_kind::<Place>("community", 0.0);
    let people: Vec<PersonId> = (0..30)
        .map(|_| context.add_entity(Person).unwrap())
        .collect();
    let household = context.create_setting_group(home);
    for &person in &people[..4] {
        context.add_setting_member(household, person);
    }
    let workplace = context.create_setting_group(work);
    for &person in &people[..17] {
        context.add_setting_member(workplace, person);
    }
    let class = context.create_setting_group(school);
    for &person in &people[10..20] {
        context.add_setting_member(class, person);
    }
    context.create_population_setting_group(community);
    context.set_itinerary_assignment(Itinerary::new([
        (home, 0.4),
        (school, 0.2),
        (work, 0.3),
        (community, 0.1),
    ]));
    people[0]
}

#[test]
fn every_settings_path_attempts_in_each_setting_at_ratio_times_multiplier() {
    // The itinerary renormalizes over home, work and community: 0.5, 0.375,
    // 0.125. Expected attempts are rate * ratio * multiplier * duration.
    let per_unit = PLACE_RATE * PLACE_DURATION;
    let expected = [
        per_unit * 0.5 * 3.0,
        0.0,
        per_unit * 0.375 * 4.0,
        per_unit * 0.125,
    ];
    for path in ["setting first", "exact", "bounded exact"] {
        let mut context = Context::new();
        context.init_random(11);
        let infector = place_population(&mut context);
        if path == "setting first" {
            TransmissionManager::<SettingFirst>::setup_transmission_loop(&mut context);
            TransmissionManager::<SettingFirst>::seed_infection(&mut context, infector);
        } else {
            let bounded = path == "bounded exact";
            context.get_data(PlaceAttemptsPlugin).bounded.set(bounded);
            TransmissionManager::<Exact>::setup_transmission_loop(&mut context);
            TransmissionManager::<Exact>::seed_infection(&mut context, infector);
        }
        context.execute();
        let attempts = context.get_data(PlaceAttemptsPlugin).by_kind.get();
        for (place, &expected) in expected.iter().enumerate() {
            let observed = attempts[place] as f64;
            assert!(
                (observed - expected).abs() <= 4.0 * expected.sqrt(),
                "{path}, kind {place}: {observed} attempts, expected {expected}"
            );
        }
    }
}

// A model in the three-method form that records where each transmission
// happened.
struct Traced;

#[derive(Default)]
struct Trace(Vec<(Option<PersonId>, PersonId, Option<Place>)>);
define_data_plugin!(TracePlugin, Trace, Trace::default());

impl InfectionManager<Traced> for Context {
    type Entity = Person;
    type Exposure = Option<Place>;

    fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
        ConstantRateFn::new(0.5, 3.0).into()
    }

    fn on_infected(
        &mut self,
        infector: Option<PersonId>,
        infectee: PersonId,
        exposure: Option<Place>,
    ) {
        self.get_data_mut(TracePlugin)
            .0
            .push((infector, infectee, exposure));
    }
}

impl TransmissionManager<Traced> for Context {
    fn contact_multiplier(&self, person: PersonId) -> f64 {
        self.current_setting_multiplier::<Place>(person)
    }

    fn sample_contact(&self, person: PersonId) -> Option<(PersonId, Option<Place>)> {
        let place = self.sample_setting_by_rate::<Place>(person)?;
        let contact = self.sample_setting_contact(person, place)?;
        Some((contact, Some(place)))
    }
}

#[test]
fn on_infected_is_told_the_setting_the_contact_draw_returned() {
    let mut context = Context::new();
    context.init_random(5);
    let seed = place_population(&mut context);
    TransmissionManager::<Traced>::setup_transmission_loop(&mut context);
    TransmissionManager::<Traced>::seed_infection(&mut context, seed);
    context.execute();

    let trace = context.get_data(TracePlugin).0.clone();
    assert_eq!(trace[0], (None, seed, None));
    assert!(trace.len() > 5, "only {} infections", trace.len());
    for &(infector, infectee, exposure) in &trace[1..] {
        let infector = infector.expect("a transmission has an infector");
        let place = exposure.expect("a transmission has a setting");
        // They share the group the attempt happened in.
        assert_eq!(
            context.setting_group(infector, place),
            context.setting_group(infectee, place)
        );
        assert!(context.setting_group(infectee, place).is_some());
    }
}

#[test]
fn attempts_follow_the_multiplier_under_a_larger_bound() {
    // Bound 4 throughout; the multiplier is 1 until t = 10 and 3 until
    // t = 30, with no reforecast. Expected attempts: 2 * (10 + 3 * 20) = 140.
    let mean = mean_attempts(|seed| {
        let mut context = Context::new();
        context.init_random(seed);
        let person = context.add_entity(Person).unwrap();
        let state = context.get_data(CountingPlugin);
        state.bound.set(Some(4.0));
        TransmissionManager::<Counting>::setup_transmission_loop(&mut context);
        TransmissionManager::<Counting>::seed_infection(&mut context, person);
        context.add_plan(10.0, |context| {
            context.get_data(CountingPlugin).multiplier.set(3.0);
        });
        context.add_plan(30.0, Context::shutdown);
        context.execute();
        context.get_data(CountingPlugin).attempts.get()
    });
    assert_about(mean, 140.0, 6.0);
}

#[test]
#[should_panic(expected = "must be finite and non-negative")]
fn an_infinite_bound_panics_instead_of_proposing_forever() {
    let mut context = Context::new();
    context.init_random(1);
    let person = context.add_entity(Person).unwrap();
    let state = context.get_data(CountingPlugin);
    state.bound.set(Some(f64::INFINITY));
    TransmissionManager::<Counting>::setup_transmission_loop(&mut context);
    TransmissionManager::<Counting>::seed_infection(&mut context, person);
    context.execute();
}

#[test]
fn scheduling_an_infectiousness_modifier_for_someone_never_infected_does_nothing() {
    struct Mask;
    let mut context = Context::new();
    context.init_random(1);
    let person = context.add_entity(Person).unwrap();
    InfectionManager::<Sir>::schedule_infectiousness_modifier::<Mask>(
        &mut context,
        person,
        0.5,
        1.0,
        Some(2.0),
    );
    context.execute();
    // Nothing was scheduled.
    assert_eq!(context.get_current_time(), 0.0);
    assert_eq!(
        InfectionManager::<Sir>::infectiousness_multiplier(&context, person),
        1.0
    );
}
