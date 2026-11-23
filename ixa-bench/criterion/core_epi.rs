//! Whole outbreaks on the `core_epi` transmission loop: homogeneous mixing
//! (on a constant rate and on an empirical curve), and households plus a
//! community on each of the two settings forms. Also the three lookups of
//! `EmpiricalRateFn` on their own.
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use ixa::core_epi::{
    AnyInfectionRate, ConstantRateFn, EmpiricalRateFn, InfectionManager, InvertibleRate, Itinerary,
    RateAssignment, SettingsExt, TransmissionManager,
};
use ixa::define_settings;
use ixa::prelude::*;

define_entity!(Person);
define_settings!(Setting for Person);

const PEOPLE: usize = 100_000;
const HOUSEHOLD_SIZE: usize = 4;
const SEEDS: usize = 100;
const INFECTIOUS_PERIOD: f64 = 5.0;

struct Homogeneous;
struct FixedSettings;
struct ChangingSettings;

/// One shared constant rate function, permanent immunity, no details.
macro_rules! impl_infection_manager {
    ($model:ty) => {
        impl InfectionManager<$model> for Context {
            type Entity = Person;
            type Details = ();

            fn draw_infectiousness_rate_fn(
                &mut self,
                _person: PersonId,
                _details: &(),
            ) -> RateAssignment {
                RateAssignment::Library(0)
            }
        }
    };
}

impl_infection_manager!(Homogeneous);
impl_infection_manager!(FixedSettings);
impl_infection_manager!(ChangingSettings);

impl TransmissionManager<Homogeneous> for Context {}

impl TransmissionManager<FixedSettings> for Context {
    fn contact_multiplier(&self, person: PersonId) -> f64 {
        self.current_setting_multiplier::<Setting>(person)
    }

    fn sample_contact(&self, person: PersonId) -> Option<(PersonId, ())> {
        let setting = self.sample_setting_by_rate::<Setting>(person)?;
        Some((self.sample_setting_contact(person, setting)?, ()))
    }
}

impl TransmissionManager<ChangingSettings> for Context {
    fn max_contact_multiplier(&self, person: PersonId) -> f64 {
        self.max_setting_multiplier::<Setting>(person)
    }

    fn evaluate_forecast(&self, person: PersonId, forecast: f64) -> Option<(PersonId, ())> {
        let setting = self.sample_setting::<Setting>(person)?;
        let intrinsic =
            InfectionManager::<ChangingSettings>::intrinsic_infectiousness(self, person)?;
        let total = intrinsic * self.setting_multiplier(person, setting);
        if !TransmissionManager::<ChangingSettings>::accept_proposal(self, person, total, forecast)
        {
            return None;
        }
        Some((self.sample_setting_contact(person, setting)?, ()))
    }
}

fn population(seed: u64) -> (Context, Vec<PersonId>) {
    let mut context = Context::new();
    context.init_random(seed);
    let people = (0..PEOPLE)
        .map(|_| context.add_entity(Person).unwrap())
        .collect();
    (context, people)
}

/// Households of `HOUSEHOLD_SIZE` and one community, half of each person's
/// contact in each: a contact multiplier of 2.
fn add_settings(context: &mut Context, people: &[PersonId]) {
    let household = context.add_setting_kind::<Setting>("household", 1.0);
    let community = context.add_setting_kind::<Setting>("community", 0.0);
    context.reserve_setting_groups(household, PEOPLE / HOUSEHOLD_SIZE);
    for members in people.chunks(HOUSEHOLD_SIZE) {
        let group = context.create_setting_group(household);
        for &person in members {
            context.add_setting_member(group, person);
        }
    }
    context.create_population_setting_group(community);
    context.set_itinerary_assignment(Itinerary::new([(household, 0.5), (community, 0.5)]));
}

/// Defines `fn $name(context, rate_fn) -> usize`: run an outbreak of `$model`
/// from `SEEDS` infections, everyone on `rate_fn`, and return how many were
/// infected.
macro_rules! define_outbreak {
    ($name:ident, $model:ty) => {
        fn $name(mut context: Context, rate_fn: AnyInfectionRate) -> usize {
            InfectionManager::<$model>::load_infectiousness_rate_library(
                &mut context,
                vec![rate_fn],
            );
            TransmissionManager::<$model>::setup_transmission_loop(&mut context);
            TransmissionManager::<$model>::seed_infections(&mut context, Person, SEEDS);
            context.execute();
            context
                .get_entity_iterator::<Person>()
                .filter(|&person| {
                    InfectionManager::<$model>::infection_time(&context, person).is_some()
                })
                .count()
        }
    };
}

define_outbreak!(homogeneous_outbreak, Homogeneous);
define_outbreak!(fixed_settings_outbreak, FixedSettings);
define_outbreak!(changing_settings_outbreak, ChangingSettings);

// R0 of 2 under homogeneous mixing. Under settings the contact multiplier is
// 2, which doubles the attempts; many of them repeat a household contact.
const HOMOGENEOUS_RATE: f64 = 0.4;
const SETTINGS_RATE: f64 = 0.4;

fn constant_rate(rate: f64) -> AnyInfectionRate {
    ConstantRateFn::new(rate, INFECTIOUS_PERIOD).into()
}

const EMPIRICAL_ANCHORS: usize = 600;
const EMPIRICAL_SPAN: f64 = 30.0;
const EMPIRICAL_R0: f64 = 2.0;

/// A gamma-shaped curve (shape 3, mean 4.5 days) of unit area through
/// `anchors` points over `EMPIRICAL_SPAN` days, spaced quadratically: close
/// together early, far apart in the tail.
fn empirical_rate(anchors: usize, scale: f64) -> EmpiricalRateFn {
    let mut points = Vec::with_capacity(anchors);
    for index in 0..anchors {
        let fraction = index as f64 / (anchors - 1) as f64;
        let time = EMPIRICAL_SPAN * fraction * fraction;
        points.push([time, time * time * (-time / 1.5).exp()]);
    }
    let mut area = 0.0;
    for pair in points.windows(2) {
        area += 0.5 * (pair[0][1] + pair[1][1]) * (pair[1][0] - pair[0][0]);
    }
    for point in &mut points {
        point[1] /= area;
    }
    EmpiricalRateFn::new(points, scale)
}

/// The curve of `constant_rate(HOMOGENEOUS_RATE)` as two anchors, the smallest
/// empirical curve.
fn flat_empirical_rate() -> EmpiricalRateFn {
    let points = vec![
        [0.0, HOMOGENEOUS_RATE],
        [INFECTIOUS_PERIOD, HOMOGENEOUS_RATE],
    ];
    EmpiricalRateFn::new(points, 1.0)
}

/// `count` values spread over `[0, upper)` in no particular order, so a
/// lookup's branches are not predictable from the one before.
fn scattered_inputs(count: usize, upper: f64) -> Vec<f64> {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut inputs = Vec::with_capacity(count);
    for _ in 0..count {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        inputs.push((state >> 11) as f64 / (1u64 << 53) as f64 * upper);
    }
    inputs
}

/// `rate`, `cum_rate` and `inverse_cum_rate` on empirical curves of a few
/// sizes, each measured over 1024 scattered inputs.
fn bench_empirical_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("core_epi_empirical_rate");
    let times = scattered_inputs(1024, EMPIRICAL_SPAN);
    let counts = scattered_inputs(1024, EMPIRICAL_R0);

    for anchors in [2, 8, 15, 32, EMPIRICAL_ANCHORS, 5000] {
        // Read through `black_box` on every call, as the transmission loop
        // meets it: nothing about the rate function is known ahead of a call,
        // so no check on it can be lifted out of the loop.
        let rate_fn: AnyInfectionRate = empirical_rate(anchors, EMPIRICAL_R0).into();
        group.bench_function(format!("rate_{anchors}_anchors"), |b| {
            b.iter(|| {
                let mut sum = 0.0;
                for &time in &times {
                    sum += black_box(&rate_fn).rate(black_box(time));
                }
                sum
            });
        });
        group.bench_function(format!("cum_rate_{anchors}_anchors"), |b| {
            b.iter(|| {
                let mut sum = 0.0;
                for &time in &times {
                    sum += black_box(&rate_fn).cum_rate(black_box(time));
                }
                sum
            });
        });
        group.bench_function(format!("inverse_cum_rate_{anchors}_anchors"), |b| {
            b.iter(|| {
                let mut sum = 0.0;
                for &count in &counts {
                    sum += black_box(&rate_fn)
                        .inverse_cum_rate(black_box(count))
                        .unwrap_or(0.0);
                }
                sum
            });
        });
    }

    group.finish();
}

fn with_settings(seed: u64) -> Context {
    let (mut context, people) = population(seed);
    add_settings(&mut context, &people);
    context
}

fn bench_core_epi(c: &mut Criterion) {
    // A benchmark of an outbreak that fizzled would measure nothing.
    for (name, infected) in [
        (
            "homogeneous",
            homogeneous_outbreak(population(1).0, constant_rate(HOMOGENEOUS_RATE)),
        ),
        (
            "homogeneous empirical",
            homogeneous_outbreak(
                population(1).0,
                empirical_rate(EMPIRICAL_ANCHORS, EMPIRICAL_R0).into(),
            ),
        ),
        (
            "homogeneous empirical, 2 anchors",
            homogeneous_outbreak(population(1).0, flat_empirical_rate().into()),
        ),
        (
            "fixed settings",
            fixed_settings_outbreak(with_settings(1), constant_rate(SETTINGS_RATE)),
        ),
        (
            "changing settings",
            changing_settings_outbreak(with_settings(1), constant_rate(SETTINGS_RATE)),
        ),
    ] {
        assert!(
            infected > PEOPLE / 2,
            "{name}: only {infected} of {PEOPLE} infected"
        );
    }

    let mut group = c.benchmark_group("core_epi");
    group.sample_size(10);

    group.bench_function("homogeneous_100k", |b| {
        b.iter_batched(
            || population(1).0,
            |context| {
                black_box(homogeneous_outbreak(
                    context,
                    constant_rate(HOMOGENEOUS_RATE),
                ))
            },
            BatchSize::PerIteration,
        );
    });
    group.bench_function("homogeneous_empirical_600_anchors_100k", |b| {
        b.iter_batched(
            || population(1).0,
            |context| {
                let rate_fn = empirical_rate(EMPIRICAL_ANCHORS, EMPIRICAL_R0);
                black_box(homogeneous_outbreak(context, rate_fn.into()))
            },
            BatchSize::PerIteration,
        );
    });
    group.bench_function("homogeneous_empirical_2_anchors_100k", |b| {
        b.iter_batched(
            || population(1).0,
            |context| black_box(homogeneous_outbreak(context, flat_empirical_rate().into())),
            BatchSize::PerIteration,
        );
    });
    group.bench_function("fixed_settings_100k", |b| {
        b.iter_batched(
            || with_settings(1),
            |context| {
                black_box(fixed_settings_outbreak(
                    context,
                    constant_rate(SETTINGS_RATE),
                ))
            },
            BatchSize::PerIteration,
        );
    });
    group.bench_function("changing_settings_100k", |b| {
        b.iter_batched(
            || with_settings(1),
            |context| {
                black_box(changing_settings_outbreak(
                    context,
                    constant_rate(SETTINGS_RATE),
                ))
            },
            BatchSize::PerIteration,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_core_epi, bench_empirical_rate);
criterion_main!(benches);
