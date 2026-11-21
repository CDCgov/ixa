# `core_epi`

The `core_epi` module provides reusable building blocks for epidemiological
models: infectiousness rate functions, a continuous-time transmission loop,
modifiers, and contact settings, with more features planned over time.

The module is gated behind the `core_epi` Cargo feature, which is enabled by
default. The examples on this page also use the `rand_distr` crate to draw
durations.

## A minimal model

A model implements two traits on `Context` (`InfectionManager` and
`TransmissionManager`), parameterized by a marker type of its own. Defining a
second marker type lets you model a [second pathogen](#more-than-one-pathogen).

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

/// Marker type for this pathogen.
struct Flu;

impl InfectionManager<Flu> for Context {
    type Entity = Person;
    // No extra per-infection data; see "Infection details".
    type Details = ();

    // How infectious this person is over the course of their infection.
    fn draw_infectiousness_rate_fn(&mut self, _person: PersonId, _details: &()) -> RateAssignment {
        let infectious_period = self.sample_distr(InfectiousPeriodRng, Exp::new(1.0 / 5.0).unwrap());
        ConstantRateFn::new(0.4, infectious_period).into()
    }

    // The model's own bookkeeping.
    fn on_infected(&mut self, _infector: Option<PersonId>, infectee: PersonId, _details: ()) {
        self.set_property(infectee, InfectionStatus::Infected);
    }
}

// Defaults implement homogeneous mixing.
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

Each infected person makes transmission attempts at rate 0.4 per day for an
exponentially distributed period with a mean of 5 days (R0 = 2). Each attempt
targets another person uniformly at random, and each person can be infected at
most once. See `examples/core-epi` for the complete model.

## How the loop works

Each infected person runs a loop of transmission attempt _proposals_. One
iteration, simplified:

```rust
// 1. Forecast: an upper bound on the person's total infectiousness as a rate
//    function of time from now. Inverting an Exp(1) draw gives the delay to
//    the next proposal; `None` means the forecast has ended, so the loop stops.
let forecast_fn = context.forecast_total_infectiousness(person);
let delay = forecast_fn.inverse_cum_rate(exp_draw)?;
let forecast = forecast_fn.rate(delay);

// ... `delay` later ...

// 2. Evaluate: thin the proposal to an attempt and draw the contact.
if let Some((contact, details)) = context.evaluate_forecast(person, forecast) {
    // 3. The attempt transmits with probability equal to the contact's susceptibility.
    let susceptibility = context.susceptibility(contact, Some(person), &details);
    if context.sample_bool(CoreEpiTransmissionRng, susceptibility) {
        context.transmit(Some(person), contact, details);
    }
}
// Back to 1.
```

Steps 1 and 2 are `TransmissionManager` methods, with these defaults:

```rust
fn forecast_total_infectiousness(&self, person: PersonId) -> impl InvertibleRate {
    // The person's rate function shifted so that t = 0 is now. Modifiers below
    // 1 are omitted so this remains an upper bound; modifiers whose product
    // exceeds 1 scale it up.
    self.majorizing_infectiousness_rate_fn(person)
        .scaled(self.max_contact_multiplier(person)) // default: contact_multiplier(person)
}

fn evaluate_forecast(&self, person: PersonId, forecast: f64) -> Option<(PersonId, ())> {
    // The rate function now, times the infectiousness modifiers, times the
    // contact multiplier (default 1).
    let total = self.intrinsic_infectiousness(person)? * self.contact_multiplier(person);
    if !self.accept_proposal(person, total, forecast) {
        return None;
    }
    self.sample_contact(person) // default: anyone else, uniformly
}

fn accept_proposal(&self, person: PersonId, total: f64, forecast: f64) -> bool {
    assert!(total <= forecast * (1.0 + BOUND_TOLERANCE));
    total > 0.0
        && (total >= forecast || self.sample_bool(CoreEpiTransmissionRng, total / forecast))
}
```

With the defaults, a person proposes attempts at their majorizing rate,
proposals are thinned to their current infectiousness (applying any modifiers),
and contacts are chosen uniformly from everyone else. The loop stops when the
rate function ends; rejected proposals and failed transmission attempts return
to step 1.

The forecast must be an upper bound. Step 2 thins away the difference, so
anything that lowers infectiousness after a proposal is scheduled (such as a
treatment or mask) needs no rescheduling. A forecast that is too low causes
`accept_proposal` to panic; see [When rates rise](#when-rates-rise).

## Contact structure

Most models describe contact structure with these `TransmissionManager`
methods:

| Method                           | Returns                                                   | Default                                         |
| -------------------------------- | --------------------------------------------------------- | ----------------------------------------------- |
| `contact_multiplier(person)`     | Current factor applied to intrinsic infectiousness        | `1.0`                                           |
| `max_contact_multiplier(person)` | Upper bound on `contact_multiplier` until next proposal   | `contact_multiplier(person)`                    |
| `sample_contact(person)`         | Target contact and infection details for an attempt       | Uniformly random other person, with `details_from` |
| `details_from(person)`           | Infection details passed on by `person`                   | `Details::default()`                            |

The loop forecasts at the intrinsic bound times `max_contact_multiplier`, thins
by `contact_multiplier`, and calls `sample_contact` for each attempt.

```rust
impl TransmissionManager<Flu> for Context {
    fn contact_multiplier(&self, person: PersonId) -> f64 {
        /* for example, their number of neighbors in a contact network */
    }

    fn sample_contact(&self, person: PersonId) -> Option<(PersonId, ())> {
        /* one of those neighbors */
    }
}
```

`contact_multiplier` determines the actual attempt rate. Leave
`max_contact_multiplier` at its default when the multiplier is fixed during an
infection or every increase is followed by a [reforecast](#when-rates-rise);
otherwise return a finite constant it cannot exceed. A loose bound causes more
rejected proposals (extra work) but does not change the effective transmission
rate.

The helper `sample_anyone_else(person)` implements the default uniform contact
draw.

### Infection details

`sample_contact` also returns the _details_ of the infection the attempt would
cause: any extra metadata the model tracks beyond who and when, such as the
setting or [variant](#variants). The type is `InfectionManager::Details`; use
`()` if none is needed. The details are passed to:

- `susceptibility(contact, Some(infector), &details)`, which decides whether the
  attempt infects;
- `draw_infectiousness_rate_fn(contact, &details)`;
- `on_infected(infector, infectee, details)`.

Details are stored on the infection and can be read with
`infection_details(person)`.

The default `sample_contact` obtains details from `details_from(infector)`,
which returns `Details::default()` unless overridden.

```rust
impl InfectionManager<Flu> for Context {
    type Entity = Person;
    type Details = Option<Setting>;

    fn on_infected(&mut self, infector: Option<PersonId>, infectee: PersonId, details: Option<Setting>) {
        /* for example, write a row of a transmission report */
    }
}
```

`seed_infection` and `seed_infections` use `Details::default()`; use
`seed_infections_with(query, n, details)` to specify details explicitly.

### Overriding the forecast and the evaluation

Override `forecast_total_infectiousness` and `evaluate_forecast` directly when:

- the upper bound is not a constant multiple of the intrinsic rate (for example,
  when it follows a schedule of restrictions); or
- the rate and the contact depend on the same random draw, as in
  [Changing itineraries or groups](#using-settings-in-the-loop).

Start from the defaults in [How the loop works](#how-the-loop-works) and keep the
call to `accept_proposal`, which thins and validates the bound. `InvertibleRate`
is exported from `ixa::core_epi`.

## Infectiousness

A person's infectiousness is a rate function of time since infection, fixed
when they are infected. `draw_infectiousness_rate_fn` returns a `RateAssignment`:

- an individual rate function (built-in rate types convert with `.into()`), or
- `RateAssignment::Library(index)`, an index into shared curves loaded at setup
  with `load_infectiousness_rate_library`.

The built-in rate functions are `ConstantRateFn::new(rate, duration)` and
`EmpiricalRateFn::new(points, scale)` (piecewise linear through `[time, rate]`
points).

```rust
context.load_infectiousness_rate_library(vec![
    EmpiricalRateFn::new(vec![[0.0, 0.0], [2.0, 1.0], [6.0, 0.0]], 1.0).into(),
    EmpiricalRateFn::new(vec![[0.0, 0.0], [4.0, 0.5], [12.0, 0.0]], 1.0).into(),
]);
```

Methods for inspecting a person's infectiousness:

| Method                             | Returns                                            |
| ---------------------------------- | -------------------------------------------------- |
| `infection_time(person)`           | Start time of the current infection                |
| `infectiousness_duration(person)`  | Duration of the current infection's rate function  |
| `infectiousness_end_time(person)`  | Time when the rate function ends                   |
| `is_infectious(person)`            | Whether the rate function has not yet ended        |
| `intrinsic_infectiousness(person)` | Current infectiousness rate, including modifiers   |

All methods except `is_infectious` return `None` if the person has never been
infected.

`core_epi` has no disease states or recovery; the model tracks them. For
example, with a `Recovered` status:

```rust
fn on_infected(&mut self, _infector: Option<PersonId>, infectee: PersonId, _details: ()) {
    self.set_property(infectee, InfectionStatus::Infected);
    let recovery_time = self.infectiousness_end_time(infectee).unwrap();
    self.add_plan(recovery_time, move |context| {
        context.set_property(infectee, InfectionStatus::Recovered);
    });
}
```

To infect someone with a specific rate function rather than drawing one (for
example, for an imported case or in a test), pass the rate function and
[details](#infection-details) directly:

```rust
context.set_infectiousness_rate_fn(person, ConstantRateFn::new(0.5, 4.0), ());
```

This starts their transmission loop without calling `on_infected`.

## Susceptibility

`susceptibility(person, infector, &details)` returns the probability that a
transmission attempt infects `person`, computed as `base_susceptibility`
multiplied by any susceptibility [modifiers](#modifiers). Both `susceptibility`
and `base_susceptibility` receive the infector and attempt
[details](#infection-details), allowing susceptibility to depend on the infector
or [variant](#variants). The transmission loop always passes `Some(infector)`;
pass `None` when querying susceptibility without a specific infector.

The default `base_susceptibility` gives permanent immunity:

```rust
fn base_susceptibility(&self, person: PersonId, _infector: Option<PersonId>, _details: &()) -> f64 {
    if self.infection_time(person).is_none() {
        1.0
    } else {
        0.0
    }
}
```

Susceptibility is the only check the loop makes on a contact. If it is positive
for someone already infected, they can be reinfected, replacing their previous
rate function.

Susceptibility must be a valid probability in `[0, 1]`; values outside that
range cause the loop to panic. To model relative susceptibility, set the most
susceptible group to `1.0` and absorb the overall scale into the infectiousness
rate.

Override `base_susceptibility` to customize this behavior. For example, to make
children half as susceptible as adults:

```rust
define_property!(
    enum AgeGroup {
        Child,
        Adult,
    },
    Person,
    default_const = AgeGroup::Adult
);

fn base_susceptibility(&self, person: PersonId, _infector: Option<PersonId>, _details: &()) -> f64 {
    // Keep the default's permanent immunity.
    if self.infection_time(person).is_some() {
        return 0.0;
    }
    match self.get_property::<_, AgeGroup>(person) {
        AgeGroup::Adult => 1.0,
        AgeGroup::Child => 0.5,
    }
}
```

## Modifiers

Modifiers scale a person's infectiousness or susceptibility. Each modifier is
keyed by a marker type you define: setting a modifier of the same type replaces
the previous value, while modifiers of different types multiply. See
`examples/core-epi-antiviral` for an antiviral intervention example.

### Setting a modifier now

`set_infectiousness_modifier` and `set_susceptibility_modifier` take effect
immediately and persist until the matching `remove_` call, across any number of
infections. They can be set before the person is ever infected.

```rust
struct Mask;
struct Vaccine;

// Halves this person's infectiousness until removed.
context.set_infectiousness_modifier::<Mask>(person, 0.5);

// Halves this person's susceptibility until removed.
context.set_susceptibility_modifier::<Vaccine>(person, 0.5);

context.remove_infectiousness_modifier::<Mask>(person);
```

A factor can be any finite non-negative number, provided the resulting
[susceptibility](#susceptibility) does not exceed `1.0`.

### Scheduling a modifier

The `schedule_` methods apply a modifier at a start time and optionally remove
it at an end time.

```rust
struct Antiviral;
struct Vaccine;

// Scales infectiousness to 25% from one day in, for the rest of the infection.
let now = context.get_current_time();
context.schedule_infectiousness_modifier::<Antiviral>(person, 0.25, now + 1.0, None);

// Halves susceptibility from day 14 to day 194.
context.schedule_susceptibility_modifier::<Vaccine>(person, 0.5, 14.0, Some(194.0));
```

A scheduled infectiousness modifier is scoped to the person's current infection
(like a treatment):

- It starts only if they are still infectious with that infection, and ends if
  they are infected again.
- For someone never infected, it does nothing.
- If two scheduled modifiers of the same type overlap, the more recently
  scheduled one takes precedence.
- It multiplies with a modifier of the same type set directly;
  `remove_infectiousness_modifier` removes both.

A scheduled susceptibility modifier is scoped to the person (surviving
reinfection). If two schedules of the same type overlap, the more recently
scheduled one takes precedence, and a manual `set_susceptibility_modifier`
call is not removed by an earlier schedule's end time.

### Raising infectiousness

An infectiousness factor can be above `1.0`. The transmission loop automatically
reforecasts when a person's modifier increases.

```rust
struct Coughing;

// Triples this person's infectiousness while they have a cough.
context.set_infectiousness_modifier::<Coughing>(person, 3.0);
```

If your model does not use `setup_transmission_loop`, you must
[reforecast](#when-rates-rise) manually.

### Factors that change over time

The `set_*_fn` methods accept a closure evaluated on every read with the
context, the person, and the time elapsed since the modifier was set.
Susceptibility closures also receive the attempt's
[details](#infection-details):

```rust
// Protection that wanes linearly over 180 days.
context.set_susceptibility_modifier_fn::<Vaccine>(person, |_context, _person, elapsed, _details| {
    (0.5 + 0.5 * elapsed / 180.0).min(1.0)
});
```

`StepFn` represents a piecewise-constant factor:

```rust
use ixa::core_epi::StepFn;

// 80% protection for 90 days, then 50%.
let protection = StepFn::new(0.2).step_at(90.0, 0.5);
context.set_susceptibility_modifier_fn::<Vaccine>(person, move |_context, _person, elapsed, _details| {
    protection.at(elapsed)
});
```

For a factor that follows the course of the infection, read
`time_since_infection(person)` in the closure (`None` if never infected).

An infectiousness closure must return values in `[0, 1]` because the loop cannot
detect when it increases. If it can exceed `1.0`, declare an upper bound
instead:

```rust
// A cough that worsens over two days, to three times the usual rate.
context.set_infectiousness_modifier_fn_with_max::<Coughing>(person, 3.0, |_context, _person, elapsed| {
    (1.0 + elapsed).min(3.0)
});
```

The person is forecast at that maximum throughout, so a loose bound causes more
rejected proposals. If changes occur at known times, scheduling plain
`set_infectiousness_modifier` calls is more efficient.

### Population modifiers

A population modifier is registered once and applies to everyone. Its closure is
called on every read with the context and the person, so it can depend on a
property:

```rust
struct Masking;

// Anyone wearing a mask is half as infectious.
context.set_population_infectiousness_modifier::<Masking>(|context, person| {
    if context.get_property::<_, Masked>(person).0 { 0.5 } else { 1.0 }
});
```

Or it can depend on simulation time, for a population-wide intervention:

```rust
struct Lockdown;

// Contact cut to a quarter from day 20 to day 35.
let contact = StepFn::new(1.0).step_at(20.0, 0.25).step_at(35.0, 1.0);
context.set_population_infectiousness_modifier::<Lockdown>(move |context, _person| {
    contact.at(context.get_current_time())
});
```

Population modifiers multiply with per-person modifiers. Setting the same marker
type again replaces the previous closure, and
`remove_population_infectiousness_modifier` removes it.

As with per-person closures, a population infectiousness modifier must stay in
`[0, 1]` so properties or time can change without reforecasting. If it can
exceed `1.0`, use `set_population_infectiousness_modifier_with_max` to declare
an upper bound; everyone is then forecast at that maximum.

`set_population_susceptibility_modifier` works the same way for susceptibility,
and its closure also receives the attempt's [details](#infection-details):

```rust
// Masks protect the wearer less against the escape variant.
context.set_population_susceptibility_modifier::<Masking>(|context, person, variant| {
    if !context.get_property::<_, Masked>(person).0 {
        return 1.0;
    }
    match variant {
        Variant::Original => 0.6,
        Variant::Escape => 0.8,
    }
});
```

## Settings

Settings organize people into contact contexts using three concepts: _kinds_ of
places (such as household or school), _groups_ that are specific instances of a
kind (such as a particular household), and a per-person _itinerary_ that divides
their time across kinds.

`define_settings!` defines the setting-kind type for an entity. Kinds are
registered at runtime so they can be loaded from a parameters file:

```rust
use ixa::core_epi::{Itinerary, SettingsExt};
use ixa::define_settings;

define_settings!(pub Setting for Person);

let household = context.add_setting_kind::<Setting>("household", 1.0);
let community = context.add_setting_kind::<Setting>("community", 0.0);
```

The second argument is `alpha`, the density exponent: in a group of `n` people,
a member's infectiousness is multiplied by `(n - 1)^alpha`. With `alpha = 1`,
attempts scale linearly with group size; with `alpha = 0`, they are independent
of group size. For a group of size 1, the multiplier is 0.

### Groups and members

```rust
// One group at a time.
let home = context.create_setting_group(household);
context.add_setting_member(home, person);

// Or by an external ID, when loading a line list. The group is created the
// first time its ID appears.
let home = context.setting_group_for_key(household, "HH001");
context.add_setting_member(home, person);

// A group everyone is in, with no member list to store.
context.create_population_setting_group(community);
```

A person can belong to at most one group of each kind. Use
`deactivate_setting_member` and `activate_setting_member` to mark a member
inactive and active again (for example, during a school closure).

### Itineraries

```rust
context.set_itinerary_assignment(Itinerary::new([(household, 0.6), (community, 0.4)]));
```

An itinerary determines where a person's attempts occur, not how many they make:
shares are normalized across the settings the person belongs to. If someone has
no group of a given kind, that share is redistributed across their remaining
settings.

`set_itinerary_assignment` accepts a single itinerary for everyone, an
`ItineraryAssignment::by_property` mapping from a property value, or a closure
taking `(context, person)`. `override_itinerary` replaces the itinerary for one
person, and `clear_itinerary_override` undoes it:

```rust
// Isolate at home, then return to the usual itinerary.
context.override_itinerary(person, Itinerary::new([(household, 1.0)]));
context.clear_itinerary_override::<Setting>(person);
```

An override also marks the person inactive in any list group whose kind has zero
share in the override (except population groups, where members cannot be
deactivated), so they cannot be drawn as a contact there and do not count toward
its size. Clearing the override reactivates them. Explicit deactivations are
tracked separately: calling `deactivate_setting_member` during an override keeps
the person inactive after the override is cleared, and calling
`activate_setting_member` for an excluded kind takes effect once the override is
cleared.

Unlike overrides, `set_itinerary_assignment` does not change active state. If an
assignment closure omits a kind, it only shifts that person's own attempts, so
pair it with `deactivate_setting_member` if they should also be inactive in the
group.

Because overrides change group sizes, models that use them should use the
"Changing itineraries or groups" approach below.

### Using settings in the loop

Either approach below produces attempts in each setting at the rate
`intrinsic * share * multiplier`. Which to use depends on whether itineraries
and groups change while people are infectious.

**Fixed itineraries and groups.** The contact multiplier is
`current_setting_multiplier`: the average of the person's group multipliers,
weighted by their itinerary. The forecast uses that multiplier directly, so no
proposals are rejected. Each attempt's setting is chosen in proportion to its
contribution (`share * multiplier`).

For example, with the kinds and itinerary above, someone in a household of 4:

| Setting   | Share | Multiplier      | Share x multiplier |
| --------- | ----- | --------------- | ------------------ |
| household | 0.6   | `(4 - 1)^1` = 3 | 1.8                |
| community | 0.4   | `(n - 1)^0` = 1 | 0.4                |

Their `current_setting_multiplier` is 1.8 + 0.4 = 2.2, so they make attempts at
2.2 times their intrinsic rate. Each attempt happens in the household with
probability 1.8 / 2.2 and in the community with probability 0.4 / 2.2.

```rust
impl TransmissionManager<Flu> for Context {
    fn contact_multiplier(&self, person: PersonId) -> f64 {
        self.current_setting_multiplier::<Setting>(person)
    }

    fn sample_contact(&self, person: PersonId) -> Option<(PersonId, Option<Setting>)> {
        let setting = self.sample_setting_by_rate::<Setting>(person)?;
        let contact = self.sample_setting_contact(person, setting)?;
        Some((contact, Some(setting)))
    }
}
```

**Changing itineraries or groups.** When members are deactivated and reactivated
or itineraries are overridden, forecast at `max_setting_multiplier`: the largest
multiplier across all of the person's groups, regardless of itinerary. Because a
weighted average cannot exceed its maximum term, itinerary changes and member
deactivations or reactivations stay within this bound. (Groups that gain new
members are the exception; see [When rates rise](#when-rates-rise).)

For the person above, `max_setting_multiplier` is 3 (the household multiplier).
If they isolate at home, their itinerary becomes 100% household and their rate
rises from 2.2x to 3x intrinsic, which the forecast already covers.

Each proposal draws a setting according to itinerary share and thins by
`setting_multiplier / max_setting_multiplier` (3/3 for household, 1/3 for
community). Because thinning and contact sampling both use the drawn setting,
this approach overrides `evaluate_forecast`.

```rust
impl TransmissionManager<Flu> for Context {
    fn max_contact_multiplier(&self, person: PersonId) -> f64 {
        self.max_setting_multiplier::<Setting>(person)
    }

    fn evaluate_forecast(
        &self,
        person: PersonId,
        forecast: f64,
    ) -> Option<(PersonId, Option<Setting>)> {
        let setting = self.sample_setting::<Setting>(person)?;
        let total = self.intrinsic_infectiousness(person)? * self.setting_multiplier(person, setting);
        if !self.accept_proposal(person, total, forecast) {
            return None;
        }
        let contact = self.sample_setting_contact(person, setting)?;
        Some((contact, Some(setting)))
    }
}
```

This proposes at the rate of the person's largest setting and rejects the
difference, so it is slower than the fixed form. Alternatively, you can use the
fixed form and override `max_contact_multiplier` to return
`max_setting_multiplier` if you prefer to keep `sample_contact`.

`examples/core-epi-settings` loads a line list, uses the second form, and counts
infections by setting.

## When rates rise

After a change that can raise a person's rate, reforecast their transmission
loop:

```rust
context.reforecast_transmission(person);
```

This is always safe to call and is a no-op for uninfected people. Call it when:

- a contact restriction ends, or anything else in your `contact_multiplier` or
  forecast increases;
- you use settings without `max_setting_multiplier` and a person's itinerary or
  group membership changes;
- you use `max_setting_multiplier` and groups gain new members:
  - a person joins a group: reforecast that person;
  - a group exceeds its previous maximum size: reforecast its current members
    (`setting_group_members`);
  - a person becomes active again in a group via `activate_setting_member` or
    `clear_itinerary_override`: reforecast that person, since the group may have
    exceeded its previous maximum size while they were inactive (and
    `setting_group_members` excludes inactive members).

  A population group requires reforecasting whenever it grows with `alpha > 0`,
  or grows from 1 to 2 members with any `alpha`. If groups only deactivate and
  reactivate existing members, none of these calls are needed.

Modifiers do not require manual reforecasting (`setup_transmission_loop` handles
increases automatically), and changes that only lower a rate never require
reforecasting.

## Variants

To model multiple variants of a single pathogen, use the variant as the model's
[infection details](#infection-details):

```rust
#[derive(Clone, Copy, Default, PartialEq)]
enum Variant {
    #[default]
    Original,
    Escape,
}

impl InfectionManager<Flu> for Context {
    type Entity = Person;
    type Details = Variant;

    // Each variant has its own course of infection.
    fn draw_infectiousness_rate_fn(&mut self, _person: PersonId, variant: &Variant) -> RateAssignment {
        match variant {
            Variant::Original => ConstantRateFn::new(0.5, 5.0).into(),
            Variant::Escape => ConstantRateFn::new(0.8, 5.0).into(),
        }
    }

    // An infection with the original variant protects only partly against
    // the escape variant.
    fn base_susceptibility(&self, person: PersonId, _infector: Option<PersonId>, variant: &Variant) -> f64 {
        match (self.infection_details(person), variant) {
            (None, _) => 1.0,
            (Some(Variant::Original), Variant::Escape) => 0.5,
            (Some(_), _) => 0.0,
        }
    }
}

impl TransmissionManager<Flu> for Context {
    // An infection carries its infector's variant.
    fn details_from(&self, infector: PersonId) -> Variant {
        *self.infection_details(infector).expect("an infector is infected")
    }
}
```

`details_from` propagates the variant to new infections; without it, every
transmission carries `Details::default()` (here, `Variant::Original`). Mutation
logic belongs here as well. If you override `sample_contact` or
`evaluate_forecast`, construct the details there instead.

Introduce a variant by seeding it:

```rust
context.seed_infections_with(Person, 5, Variant::Escape);
```

A susceptibility modifier closure can also depend on the variant:

```rust
// A vaccine that protects better against the original variant.
context.set_susceptibility_modifier_fn::<Vaccine>(person, |_context, _person, _elapsed, variant| {
    match variant {
        Variant::Original => 0.2,
        Variant::Escape => 0.7,
    }
});
```

For an infectiousness modifier, the infector's variant is
`infection_details(person)`.

`core_epi` stores only the current infection's details. That suffices above
because `Escape` protects against both variants; if immunity depends on full
infection history, record past infections in a property inside `on_infected`.

Model a more transmissible variant with a higher infectiousness rate function
rather than a susceptibility above `1.0`.

## More than one pathogen

Implementing the traits for a second marker type adds a second pathogen on the
same population, with its own rate library, infections, modifiers, and immunity:

```rust
struct Flu;
struct Rsv;

impl InfectionManager<Flu> for Context { /* ... */ }
impl InfectionManager<Rsv> for Context { /* ... */ }
impl TransmissionManager<Flu> for Context {}
impl TransmissionManager<Rsv> for Context {}
```

Settings belong to the entity, so both pathogens share them.

When multiple pathogens are in scope, unqualified method calls are ambiguous;
use fully qualified syntax with the trait and marker type:

```rust
InfectionManager::<Flu>::infection_time(&context, person);
TransmissionManager::<Rsv>::seed_infections(&mut context, Person, 5);
```

The same syntax resolves name collisions with methods on your own traits.

## Examples

| Example                       | Shows                                                    |
| ----------------------------- | -------------------------------------------------------- |
| `examples/core-epi`           | Minimal model: homogeneous mixing with the defaults      |
| `examples/core-epi-antiviral` | Scheduled infectiousness modifier                        |
| `examples/core-epi-settings`  | Settings loaded from a line list, infections by setting  |
| `examples/core-epi-variants`  | Two variants as infection details, with a vaccine        |
