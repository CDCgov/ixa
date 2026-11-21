//! Whole outbreaks on the `core_epi` transmission loop: homogeneous mixing,
//! and households plus a community on each of the two settings forms.
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use ixa::core_epi::{
    ConstantRateFn, InfectionManager, Itinerary, RateAssignment, SettingsExt, TransmissionManager,
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

/// Defines `fn $name(context) -> usize`: run an outbreak of `$model` from
/// `SEEDS` infections at `rate` and return how many were infected.
macro_rules! define_outbreak {
    ($name:ident, $model:ty) => {
        fn $name(mut context: Context, rate: f64) -> usize {
            InfectionManager::<$model>::load_infectiousness_rate_library(
                &mut context,
                vec![ConstantRateFn::new(rate, INFECTIOUS_PERIOD).into()],
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

// R0 of 2 under homogeneous mixing. Under settings the contact multiplier is 2,
// so the same intrinsic rate drives a comparable outbreak despite repeat
// household contacts.
const HOMOGENEOUS_RATE: f64 = 0.4;
const SETTINGS_RATE: f64 = 0.4;

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
            homogeneous_outbreak(population(1).0, HOMOGENEOUS_RATE),
        ),
        (
            "fixed settings",
            fixed_settings_outbreak(with_settings(1), SETTINGS_RATE),
        ),
        (
            "changing settings",
            changing_settings_outbreak(with_settings(1), SETTINGS_RATE),
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
            |context| black_box(homogeneous_outbreak(context, HOMOGENEOUS_RATE)),
            BatchSize::PerIteration,
        );
    });
    group.bench_function("fixed_settings_100k", |b| {
        b.iter_batched(
            || with_settings(1),
            |context| black_box(fixed_settings_outbreak(context, SETTINGS_RATE)),
            BatchSize::PerIteration,
        );
    });
    group.bench_function("changing_settings_100k", |b| {
        b.iter_batched(
            || with_settings(1),
            |context| black_box(changing_settings_outbreak(context, SETTINGS_RATE)),
            BatchSize::PerIteration,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_core_epi);
criterion_main!(benches);
