# Epidemic Models with `core_epi`

The `core_epi` module includes a set of epi features including (so far), including definition of infectiousness rates and related utilties, a standard continuous-time transmission loop, and settings. We intend to expand the feature set over time.

The module is behind the `core_epi` cargo feature, which is on by default.

## A minimal model

To use the transmission loop features, a model implements two traits for `Context`. You must supply a marker type (it is also possible to have more than one for multiple pathogens, see [below](#more-than-one-pathogen)).

```rust
use ixa::core_epi::{ConstantRateFn, InfectionManager, RateAssignment, TransmissionManager};
use ixa::prelude::*;
use rand_distr::Exp;

define_entity!(Person);
define_rng!(InfectiousPeriodRng);

define_property!(
    enum InfectionStatus {
        Susceptible,
        Infected,
    },
    Person,
    default_const = InfectionStatus::Susceptible
);

/// Marker type the `core_epi` traits are implemented for.
struct Flu;

impl InfectionManager<Flu> for Context {
    type Entity = Person;

    // How infectious this person is over the course of their infection.
    fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
        let infectious_period = self.sample_distr(InfectiousPeriodRng, Exp::new(1.0 / 5.0).unwrap());
        ConstantRateFn::new(0.4, infectious_period).into()
    }

    // The model's own bookkeeping.
    fn on_infected(&mut self, _infector: Option<PersonId>, infectee: PersonId) {
        self.set_property(infectee, InfectionStatus::Infected);
    }
}

// Every method has a default, and together they are homogeneous mixing.
impl TransmissionManager<Flu> for Context {}
```

At setup, start the loop and seed some infections:

```rust
let mut context = Context::new();
context.init_random(42);
for _ in 0..1000 {
    context.add_entity(Person).unwrap();
}
context.setup_transmission_loop();
context.seed_infections(Person, 5);
context.execute();
```

In this example, each infected person makes infection attempts
at rate 0.4 per day for an exponentially distributed period with mean 5 days
(R0 of 2), each attempt contacts someone else uniformly at random, and a person
can be infected once. See `examples/core-epi` for a complete working version of this model.

## How the loop works

Each infected person runs their own loop of attempt _proposals_:

1. The loop asks the model for a **forecast**: a rate function, in time from now,
   that is at least as large as the person's total infectiousness will be. It
   draws the time of the next proposal from it.
2. At that time the model **evaluates** the proposal. It computes the person's
   total infectiousness now and accepts the proposal with probability
   `total / forecast`. An accepted proposal is an _attempt_, and the model
   returns who is contacted.
3. The attempt becomes a transmission with probability equal to the contact's
   **susceptibility**.

Then the loop schedules the next proposal. It ends when the forecast runs out,
which for is the person's infectiousness duration by default.

The forecast must be an upper bound. Thinning in step 2 corrects for the
difference, so anything that lowers infectiousness after the proposal was drawn
(a treatment, a mask, a contact restriction) is handled without rescheduling. A
forecast that is too low is an error, and the loop panics rather
than under-count attempts. See [When rates rise](#when-rates-rise).

## Infectiousness

A person's infectiousness is a rate function of time since infection, assigned
when they are infected and fixed for that infection. `draw_infectiousness_rate_fn`
returns a `RateAssignment`, which is one of:

- an individually-assigned rate function, for example a constant rate over a
  period drawn for them. Any rate function converts with `.into()`.
- `RateAssignment::Library(index)`, an index into a library of shared curves
  loaded once at setup with `load_infectiousness_rate_library`. Use this when
  many people share a few curves.

The built-in rate functions are `ConstantRateFn::new(rate, duration)` and
`EmpiricalRateFn::new(points, scale)`, a piecewise-linear curve through
`[time, rate]` points.

```rust
context.load_infectiousness_rate_library(vec![
    EmpiricalRateFn::new(vec![[0.0, 0.0], [2.0, 1.0], [6.0, 0.0]], 1.0).into(),
    EmpiricalRateFn::new(vec![[0.0, 0.0], [4.0, 0.5], [12.0, 0.0]], 1.0).into(),
]);
```

Once someone is infected, the following utilities are available:

| Method                                | Returns                                                  |
| ------------------------------------- | -------------------------------------------------------- |
| `infection_time(person)`              | when their current infection began                       |
| `infectiousness_duration(person)`     | how long that infection is infectious for                |
| `infectiousness_end_time(person)`     | when their rate function runs out                        |
| `is_infectious(person)`               | whether it has not yet run out                           |
| `intrinsic_infectiousness(person)`    | their rate now, with modifiers applied                   |

All but `is_infectious` return `None` for someone never infected.

`core_epi` has no notion of recovery or of disease states: you must track those yourself. For example, to implement a recovered state, schedule it from `on_infected`:

```rust
fn on_infected(&mut self, _infector: Option<PersonId>, infectee: PersonId) {
    self.set_property(infectee, InfectionStatus::Infected);
    let recovery_time = self.infectiousness_end_time(infectee).unwrap();
    self.add_plan(recovery_time, move |context| {
        context.set_property(infectee, InfectionStatus::Recovered);
    });
}
```

To infect someone with a known course of infection instead of a drawn one (an
imported case, or in a test), assign the rate function
directly:

```rust
context.set_infectiousness_rate_fn(person, ConstantRateFn::new(0.5, 4.0));
```

## Susceptibility

`susceptibility(person)` is the probability that an attempt on them succeeds. It
is `base_susceptibility` times the person's susceptibility modifiers.

The default `base_susceptibility` is permanent immunity: 1 for someone never
infected and 0 afterwards. Override it for anything else, for example immunity
that follows a status the model can reset:

```rust
fn base_susceptibility(&self, person: PersonId) -> f64 {
    if self.get_property::<_, InfectionStatus>(person) == InfectionStatus::Susceptible {
        1.0
    } else {
        0.0
    }
}
```

## Modifiers

Interventions change infectiousness and susceptibility through modifiers: factors
that multiply together. Each is identified by a type of the model's choosing, so
setting a modifier again replaces its factor and different modifiers compose.

```rust
struct Antiviral;
struct Vaccine;

// Scales this infection's infectiousness to 25% from one day in, until it ends.
let start = context.get_current_time() + 1.0;
context.schedule_infectiousness_modifier::<Antiviral>(person, 0.25, start, None);

// Halves this person's susceptibility until removed.
context.set_susceptibility_modifier::<Vaccine>(person, 0.5);
```

The two kinds differ in what they belong to:

- An **infectiousness modifier** belongs to the current infection. Its factor
  must be in `[0, 1]`, so the forecast stays an upper bound, and it is cleared
  when the person is infected again. Setting one on someone never infected does
  nothing, and a scheduled one applies only if they are still infectious with
  the same infection when it starts.
- A **susceptibility modifier** belongs to the person. It applies while they are
  uninfected, survives reinfection, and may be any finite non-negative factor.

Both have `set_`, `remove_` and `schedule_` forms, and a `set_..._fn` form whose
factor is computed on every read from the context, the person and the time
elapsed (since infection for infectiousness, since it was set for
susceptibility). That is for effects that build up or wane:

```rust
// Protection that wanes linearly over 180 days.
context.set_susceptibility_modifier_fn::<Vaccine>(person, |_context, _person, elapsed| {
    (0.5 + 0.5 * elapsed / 180.0).min(1.0)
});
```

`examples/core-epi-antiviral` runs the same outbreak with and without an
antiviral.

## Contact structure

`TransmissionManager` decides who is contacted and how contact structure changes
the rate. There are two levels of override.

To change only _who_ is contacted, at the intrinsic rate, override
`sample_contact`:

```rust
impl TransmissionManager<Flu> for Context {
    fn sample_contact(&self, person: PersonId) -> Option<PersonId> {
        /* for example, a neighbor in a contact network */
    }
}
```

When contact structure changes the _rate_ as well, override the forecast and the
evaluation together. The forecast scales the intrinsic bound by the largest
contact multiplier the person can have, and the evaluation thins by their
multiplier now. For example:

```rust
use ixa::core_epi::{InvertibleRate, ScaledRateFn};

impl TransmissionManager<Flu> for Context {
    fn forecast_total_infectiousness(&self, person: PersonId) -> impl InvertibleRate {
        ScaledRateFn::new(
            self.majorizing_infectiousness_rate_fn(person),
            max_contact_multiplier(self, person),
        )
    }

    fn evaluate_forecast(&self, person: PersonId, forecast: f64) -> Option<PersonId> {
        let total = self.intrinsic_infectiousness(person)? * contact_multiplier(self, person);
        if !self.accept_proposal(person, total, forecast) {
            return None;
        }
        self.sample_contact(person)
    }
}
```

`accept_proposal` does the thinning.

## Settings

Settings group people into places: named _kinds_ of place (household, school,
workplace), _groups_ that are instances of a kind (one particular household),
and per person an _itinerary_ saying how their time is split across kinds.

`define_settings!` names the type of an entity's setting kinds. The kinds
themselves are added at run time, so they can come from a parameters file:

```rust
use ixa::core_epi::{Itinerary, SettingsExt};
use ixa::define_settings;

define_settings!(pub Setting for Person);

let household = context.add_setting_kind::<Setting>("household", 1.0);
let community = context.add_setting_kind::<Setting>("community", 0.0);
```

The second argument is `alpha`, the kind's density exponent. In a group of `n`
people, a member's infectiousness there is multiplied by `(n - 1)^alpha`: with
`alpha = 1` attempts scale with the size of the group, and with `alpha = 0` they
do not. A person alone in a group has a multiplier of 0.

### Groups and members

```rust
// One group at a time.
let home = context.create_setting_group(household);
context.add_setting_member(home, person);

// Or by an external id, when loading a line list. The group is created the
// first time its id appears.
let home = context.setting_group_for_key(household, "HH001");
context.add_setting_member(home, person);

// A group everyone is in, with no member list to store.
context.create_population_setting_group(community);
```

A person is in at most one group of each kind. `remove_setting_member` takes them
out of their group and `restore_setting_member` puts them back, which is how an
intervention such as a school closure is expressed.

### Itineraries

```rust
context.set_itinerary_assignment(Itinerary::new([(household, 0.6), (community, 0.4)]));
```

An itinerary's proportions are shares of where a person's attempts happen, not a
scale on how many: they are normalized, so only their ratios matter. Someone with
no group of a kind spends that share in their other settings.

`set_itinerary_assignment` takes one itinerary for everyone, an
`ItineraryAssignment::by_property` closure from a property value to an itinerary
(by age group, say), or a closure from the context and a person. An individual
can be given an itinerary of their own with `set_itinerary`, and
`clear_itinerary` removes it again:

```rust
// Isolate at home, then return to the usual itinerary.
context.set_itinerary(person, Itinerary::new([(household, 1.0)]));
context.clear_itinerary::<Setting>(person);
```

### Using settings in the loop

There are two ways to connect settings to the loop. Both give attempts in each
setting at the rate `intrinsic * share * multiplier`.

**Setting first.** Draw where the attempt happens, then thin by that setting's
multiplier. The forecast is the largest multiplier any of the person's groups has
ever had, which stays valid when members are removed and restored and when
itineraries are overridden and cleared.

```rust
impl TransmissionManager<Flu> for Context {
    fn forecast_total_infectiousness(&self, person: PersonId) -> impl InvertibleRate {
        ScaledRateFn::new(
            self.majorizing_infectiousness_rate_fn(person),
            self.max_setting_multiplier::<Setting>(person),
        )
    }

    fn evaluate_forecast(&self, person: PersonId, forecast: f64) -> Option<PersonId> {
        let setting = self.sample_setting::<Setting>(person)?;
        let total = self.intrinsic_infectiousness(person)? * self.setting_multiplier(person, setting);
        if !self.accept_proposal(person, total, forecast) {
            return None;
        }
        self.sample_setting_contact(person, setting)
    }
}
```

**Exact.** Forecast on the person's total over all their settings and thin by the
same value, so that no proposal is rejected, then draw the setting in proportion
to its part of the total.

```rust
impl TransmissionManager<Flu> for Context {
    fn forecast_total_infectiousness(&self, person: PersonId) -> impl InvertibleRate {
        ScaledRateFn::new(
            self.majorizing_infectiousness_rate_fn(person),
            self.current_setting_multiplier::<Setting>(person),
        )
    }

    fn evaluate_forecast(&self, person: PersonId, forecast: f64) -> Option<PersonId> {
        let total =
            self.intrinsic_infectiousness(person)? * self.current_setting_multiplier::<Setting>(person);
        if !self.accept_proposal(person, total, forecast) {
            return None;
        }
        let setting = self.sample_setting_by_rate::<Setting>(person)?;
        self.sample_setting_contact(person, setting)
    }
}
```

The exact path is faster, because the setting-first path proposes at the rate of
the person's largest setting and rejects the difference. But its forecast is only
valid while the person's memberships and itinerary stay as they were when it was
made. Use it when they do not change during an infection, or pair every change
with a reforecast as described next. The setting-first path needs no such care
for removing and restoring members or changing itineraries.

`examples/core-epi-settings` loads households, schools and workplaces from a line
list and runs the setting-first path.

## When rates rise

A forecast is made when a proposal is scheduled. If something later raises a
person's total infectiousness above it, tell the loop:

```rust
context.reforecast_transmission(person);
```

This drops the person's pending proposal and draws a new one from now. Redrawing
is always statistically valid, so when in doubt, call it. It does nothing for
someone never infected, and it restarts a loop that ended because its forecast
was zero.

Changes that need it:

- on the exact settings path, any change to a person's memberships or itinerary;
- on either path, a group growing past its largest size, or a person joining a
  group, for the newcomer and for the group's infectious members
  (`setting_group_members`);
- anything in the model's own forecast that can go up, such as a contact
  restriction ending.

Changes that only lower a rate never need it.

## More than one pathogen

Infection state is kept per marker type. Implementing the traits for a second
marker gives a second pathogen on the same population, with its own rate
library, infections, modifiers and immunity:

```rust
struct Flu;
struct Cold;

impl InfectionManager<Flu> for Context { /* ... */ }
impl InfectionManager<Cold> for Context { /* ... */ }
impl TransmissionManager<Flu> for Context {}
impl TransmissionManager<Cold> for Context {}
```

Settings belong to the entity, so both pathogens share the same households and
itineraries.

With more than one model in scope a bare method call is ambiguous, and Rust asks
which is meant. Name the trait and marker:

```rust
InfectionManager::<Flu>::infection_time(&context, person);
TransmissionManager::<Cold>::seed_infections(&mut context, Person, 5);
```

The same form resolves a clash between a `core_epi` method and a method of the
same name in one of the model's own extension traits.

## Examples

| Example                       | Shows                                                       |
| ----------------------------- | ----------------------------------------------------------- |
| `examples/core-epi`           | the smallest model: homogeneous mixing with the defaults    |
| `examples/core-epi-antiviral` | a scheduled infectiousness modifier                         |
| `examples/core-epi-settings`  | settings loaded from a line list, on the setting-first path |
