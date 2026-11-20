# `core_epi`

The `core_epi` module includes a set of epi features: so far, definitions of
infectiousness rates and related utilities, a standard continuous-time
transmission loop, and settings. We intend to expand the feature set over time.

The module is behind the `core_epi` cargo feature, which is on by default. The
examples on this page also use the `rand_distr` crate to draw durations.

## A minimal model

A model implements two traits for `Context`, on a marker type it defines. A
second marker is a [second pathogen](#more-than-one-pathogen).

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
    // Nothing more to know about an infection: see "Infection details".
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

Each infected person makes attempts at rate 0.4 per day for an exponential
period with mean 5 days (R0 of 2), each attempt contacts someone else uniformly
at random, and a person can be infected once. `examples/core-epi` is the
complete model.

## How the loop works

Each infected person runs a loop of attempt _proposals_. One turn, simplified:

```rust
// 1. Forecast: an upper bound on the person's total infectiousness, as a rate
//    function of time from now. Inverting an Exp(1) draw on it gives the delay
//    to the next proposal. No delay means the forecast ran out: the loop ends.
let forecast_fn = context.forecast_total_infectiousness(person);
let delay = forecast_fn.inverse_cum_rate(exp_draw)?;
let forecast = forecast_fn.rate(delay);

// ... `delay` later ...

// 2. Evaluate: thin the proposal to an attempt and draw the contact.
if let Some((contact, details)) = context.evaluate_forecast(person, forecast) {
    // 3. The attempt transmits with the contact's susceptibility.
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
    // The person's rate function, shifted so that 0 is now. Modifiers that
    // lower the rate are left out, so this is a bound; modifiers that
    // multiply to more than 1 scale it up.
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

With nothing overridden, a person proposes at their rate function's rate, a
proposal is accepted with the product of their infectiousness modifiers, and the
contact is anyone else. The loop ends when the rate function runs out. A rejected
proposal or failed attempt goes back to step 1.

The forecast must be an upper bound. Step 2 thins away the difference, so
anything that lowers infectiousness after a proposal was drawn (a treatment, a
mask) needs no rescheduling. A forecast that is too low makes `accept_proposal`
panic: see [When rates rise](#when-rates-rise).

## Contact structure

Most models describe contact structure with these `TransmissionManager`
methods:

| Method                           | Returns                                               | Default                |
| -------------------------------- | ----------------------------------------------------- | ---------------------- |
| `contact_multiplier(person)`     | the factor on their intrinsic infectiousness now      | 1                      |
| `max_contact_multiplier(person)` | an upper bound on that factor until their next proposal | the multiplier now   |
| `sample_contact(person)`         | who they contact in an attempt, and the details       | anyone else, uniformly |
| `details_from(person)`           | the details of an infection they pass on              | the default details    |

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

`contact_multiplier` alone sets the rate. Leave `max_contact_multiplier` at its
default when the multiplier is fixed during an infection or every rise is
followed by a [reforecast](#when-rates-rise); otherwise return a finite constant
it cannot exceed. A loose bound costs rejected proposals but never changes the
rate.

`sample_anyone_else(person)` is the default draw.

### Infection details

`sample_contact` also returns the _details_ of the infection the attempt would
cause: whatever the model tracks besides who and when, such as the setting or
the [variant](#variants). The type is `InfectionManager::Details`; use `()` for
none. The details are passed to:

- `susceptibility(contact, Some(infector), &details)`, which decides whether the
  attempt infects;
- `draw_infectiousness_rate_fn(contact, &details)`;
- `on_infected(infector, infectee, details)`.

They are stored with the infection: `infection_details(person)` reads those of
a person's current one.

The default `sample_contact` gets them from `details_from(infector)`, which
returns the type's default value unless overridden.

```rust
impl InfectionManager<Flu> for Context {
    type Entity = Person;
    type Details = Option<Setting>;

    fn on_infected(&mut self, infector: Option<PersonId>, infectee: PersonId, details: Option<Setting>) {
        /* for example, write a row of a transmission report */
    }
}
```

`seed_infection` and `seed_infections` use the type's default value;
`seed_infections_with(query, n, details)` takes one.

### Overriding the forecast and the evaluation

Override `forecast_total_infectiousness` and `evaluate_forecast` themselves
when:

- the bound is not a constant multiple of the intrinsic one, for example it
  follows a schedule of restrictions;
- the total and the contact come from the same draw, as on the
  [changing itineraries or groups](#using-settings-in-the-loop).

Start from the defaults in [How the loop works](#how-the-loop-works) and keep the
call to `accept_proposal`, which thins and checks the bound. `InvertibleRate` is
in `ixa::core_epi`.

## Infectiousness

A person's infectiousness is a rate function of time since infection, fixed
when they are infected. `draw_infectiousness_rate_fn` returns a `RateAssignment`:

- a rate function of their own (the built-in ones convert with `.into()`), or
- `RateAssignment::Library(index)`, an index into shared curves loaded at setup
  with `load_infectiousness_rate_library`.

The built-in rate functions are `ConstantRateFn::new(rate, duration)` and
`EmpiricalRateFn::new(points, scale)`, piecewise linear through `[time, rate]`
points.

```rust
context.load_infectiousness_rate_library(vec![
    EmpiricalRateFn::new(vec![[0.0, 0.0], [2.0, 1.0], [6.0, 0.0]], 1.0).into(),
    EmpiricalRateFn::new(vec![[0.0, 0.0], [4.0, 0.5], [12.0, 0.0]], 1.0).into(),
]);
```

For someone infected:

| Method                                | Returns                                                  |
| ------------------------------------- | -------------------------------------------------------- |
| `infection_time(person)`              | when their current infection began                       |
| `infectiousness_duration(person)`     | how long that infection is infectious for                |
| `infectiousness_end_time(person)`     | when their rate function runs out                        |
| `is_infectious(person)`               | whether it has not yet run out                           |
| `intrinsic_infectiousness(person)`    | their rate now, with modifiers applied                   |

All but `is_infectious` return `None` for someone never infected.

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

To infect someone with a given rate function instead of a drawn one (an imported
case, a test), assign it with the infection's [details](#infection-details):

```rust
context.set_infectiousness_rate_fn(person, ConstantRateFn::new(0.5, 4.0), ());
```

This starts their loop but does not call `on_infected`.

## Susceptibility

`susceptibility(person, infector, &details)` is the probability that an attempt
infects the person: `base_susceptibility` times their susceptibility
[modifiers](#modifiers). Both take the infector and the attempt's
[details](#infection-details), so they can depend on who is infecting and on the
[variant](#variants); the examples here ignore them. The loop always passes
`Some(infector)`; pass `None` to ask with no particular infector.

The default `base_susceptibility` is permanent immunity:

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
for someone already infected, they are infected again and the new rate function
replaces the old one.

It is a probability: the loop panics outside 0 to 1. For relative
susceptibility, give the most susceptible group 1 and put the scale in the
infectiousness rate.

Override `base_susceptibility` for anything else, for example children half as
susceptible as adults:

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

Modifiers are factors on a person's infectiousness or susceptibility. Each is
keyed by a type the model defines: setting one again replaces it, and different
ones multiply. `examples/core-epi-antiviral` uses one for an antiviral.

### Setting a modifier now

`set_infectiousness_modifier` and `set_susceptibility_modifier` take effect
immediately and last until the matching `remove_` call, through any number of
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

A factor is any finite non-negative number, as long as the person's
[susceptibility](#susceptibility) stays at most 1.

### Scheduling a modifier

The `schedule_` methods set a modifier at a start time and, if given, remove it
at an end time.

```rust
struct Antiviral;
struct Vaccine;

// Scales infectiousness to 25% from one day in, for the rest of the infection.
let now = context.get_current_time();
context.schedule_infectiousness_modifier::<Antiviral>(person, 0.25, now + 1.0, None);

// Halves susceptibility from day 14 to day 194.
context.schedule_susceptibility_modifier::<Vaccine>(person, 0.5, 14.0, Some(194.0));
```

A scheduled infectiousness modifier lasts no longer than the infection the
person has when it is scheduled, as a treatment would:

- It starts only if they are still infectious with that infection, and ends if
  they are infected again.
- For someone never infected, it does nothing.
- Where two schedules of one type overlap, the later call's applies.
- It multiplies with a modifier of the same type set directly;
  `remove_infectiousness_modifier` removes both.

A scheduled susceptibility modifier is a plain `set_` at the start time and
`remove_` at the end time.

### Raising infectiousness

An infectiousness factor can be above 1. The loop reforecasts by itself when a
person's modifiers rise.

```rust
struct Coughing;

// Triples this person's infectiousness while they have a cough.
context.set_infectiousness_modifier::<Coughing>(person, 3.0);
```

A model that does not call `setup_transmission_loop` must
[reforecast](#when-rates-rise) itself.

### Factors that change over time

The `set_..._fn` forms take a function, called on every read with the context,
the person and the time since the modifier was set. A susceptibility function
also gets the attempt's [details](#infection-details):

```rust
// Protection that wanes linearly over 180 days.
context.set_susceptibility_modifier_fn::<Vaccine>(person, |_context, _person, elapsed, _details| {
    (0.5 + 0.5 * elapsed / 180.0).min(1.0)
});
```

`StepFn` holds an effect that changes in steps:

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

An infectiousness function must stay between 0 and 1, because the loop cannot
see it rise. For one that goes higher, declare its maximum:

```rust
// A cough that worsens over two days, to three times the usual rate.
context.set_infectiousness_modifier_fn_with_max::<Coughing>(person, 3.0, |_context, _person, elapsed| {
    (1.0 + elapsed).min(3.0)
});
```

The person is forecast at the maximum throughout, so a loose maximum costs
rejected proposals. For changes at known times, scheduling plain
`set_infectiousness_modifier` calls is cheaper.

### Modifiers on everyone

A population modifier is registered once and applies to everyone. Its function
is called on every read with the context and the person, so it can depend on a
property:

```rust
struct Masking;

// Anyone wearing a mask is half as infectious.
context.set_population_infectiousness_modifier::<Masking>(|context, person| {
    if context.get_property::<_, Masked>(person).0 { 0.5 } else { 1.0 }
});
```

Or on the time, for an effect on the whole population:

```rust
struct Lockdown;

// Contact cut to a quarter from day 20 to day 35.
let contact = StepFn::new(1.0).step_at(20.0, 0.25).step_at(35.0, 1.0);
context.set_population_infectiousness_modifier::<Lockdown>(move |context, _person| {
    contact.at(context.get_current_time())
});
```

It multiplies with each person's own modifiers, setting the same type again
replaces it, and `remove_population_infectiousness_modifier` removes it.

An infectiousness function must stay between 0 and 1, so a property or the
time can change with no reforecast. For one that goes higher, declare its
maximum with `set_population_infectiousness_modifier_with_max`: everyone is
then forecast at the maximum throughout.

`set_population_susceptibility_modifier` is the same for susceptibility, and its
function also gets the attempt's [details](#infection-details):

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

Settings put people in places: _kinds_ of place (household, school), _groups_
that are instances of a kind (one household), and per person an _itinerary_
splitting their time across kinds.

`define_settings!` names the type of an entity's kinds. The kinds are added at
run time, so they can come from a parameters file:

```rust
use ixa::core_epi::{Itinerary, SettingsExt};
use ixa::define_settings;

define_settings!(pub Setting for Person);

let household = context.add_setting_kind::<Setting>("household", 1.0);
let community = context.add_setting_kind::<Setting>("community", 0.0);
```

The second argument is `alpha`, the density exponent: in a group of `n` people a
member's infectiousness is multiplied by `(n - 1)^alpha`. With `alpha = 1`
attempts scale with group size; with `alpha = 0` they do not. Alone in a group,
the multiplier is 0.

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

A person is in at most one group of each kind. `remove_setting_member` and
`restore_setting_member` take them out and put them back, for example for a
school closure.

### Itineraries

```rust
context.set_itinerary_assignment(Itinerary::new([(household, 0.6), (community, 0.4)]));
```

An itinerary says where a person's attempts happen, not how many: the
proportions are normalized. Someone with no group of a kind spends that share in
their other settings.

`set_itinerary_assignment` takes one itinerary for everyone, an
`ItineraryAssignment::by_property` closure from a property value, or a closure
from the context and a person. `override_itinerary` replaces it for one person
and `clear_itinerary_override` undoes that:

```rust
// Isolate at home, then return to the usual itinerary.
context.override_itinerary(person, Itinerary::new([(household, 1.0)]));
context.clear_itinerary_override::<Setting>(person);
```

An override also takes the person out of their group of every kind it gives no
share, so they are not drawn as a contact there and do not count towards its
size. Clearing it puts them back. A population group is the exception: nobody
leaves one. An assignment does not do this: a closure that drops a kind for
someone only moves their own attempts, so pair it with `remove_setting_member`.

Because an override changes group sizes, a model that uses one needs the
"Changing itineraries or groups" form below.

### Using settings in the loop

Either form below gives attempts in each setting at the rate
`intrinsic * share * multiplier`. Which to use depends on whether itineraries
and groups change while people are infectious.

**Fixed itineraries and groups.** The contact multiplier is
`current_setting_multiplier`: the average of the person's group multipliers,
weighted by their itinerary. The forecast is that multiplier, so no proposals
are rejected. Each attempt's setting is drawn by its contribution to it.

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

**Changing itineraries or groups.** When members are removed and restored or
itineraries are overridden, forecast at `max_setting_multiplier`: the largest
multiplier any one of the person's groups has had, whatever their itinerary. An
average of group multipliers cannot exceed the largest of them, so no such
change takes the person's rate above it.

For the person above it is 3, the household's multiplier. If they isolate at
home, their itinerary becomes household 1.0 and their rate rises from 2.2 to 3
times intrinsic, which the forecast already covers.

Each proposal draws a setting by itinerary share and is thinned by that
setting's multiplier over the maximum: 3 / 3 for the household, 1 / 3 for the
community. The thinning and the contact draw share the setting, so this form
overrides `evaluate_forecast`.

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
difference, so it is slower than the fixed form. Returning
`max_setting_multiplier` from `max_contact_multiplier` in the fixed form has the
same effect, if you prefer to keep its contact draw.

`examples/core-epi-settings` loads a line list, uses the second form and counts
infections by setting.

## When rates rise

After a change that can raise a person's rate, reforecast them:

```rust
context.reforecast_transmission(person);
```

It is always safe to call, and does nothing for someone never infected. Call it
when:

- a contact restriction ends, or anything else in your own
  `contact_multiplier` or forecast goes up;
- using settings without `max_setting_multiplier`, a person's itinerary or the
  membership of one of their groups changes;
- with `max_setting_multiplier` as the bound, a person joins a group or a group
  grows past its largest size: reforecast the newcomer and the group's members
  (`setting_group_members`). A population group with a positive `alpha` needs
  it whenever it grows.

Modifiers need no call: `setup_transmission_loop` reforecasts when one rises.
Changes that only lower a rate never need one.

## Variants

Variants of one pathogen are one model whose
[infection details](#infection-details) are the variant:

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

`details_from` is what passes the variant on: without it every transmission
carries the default details, here the original variant. Mutation goes in the
same place. A model that overrides `sample_contact` or `evaluate_forecast`
builds the details there instead.

Introduce a variant by seeding it:

```rust
context.seed_infections_with(Person, 5, Variant::Escape);
```

A susceptibility function modifier can depend on the variant:

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

`core_epi` keeps only the current infection's details. That is enough above,
because the escape variant protects against both; when immunity depends on every
past infection, record them in a property from `on_infected`.

A variant that spreads faster gets a higher rate function, not a susceptibility
above 1.

## More than one pathogen

Implementing the traits for a second marker gives a second pathogen on the same
population, with its own rate library, infections, modifiers and immunity:

```rust
struct Flu;
struct Rsv;

impl InfectionManager<Flu> for Context { /* ... */ }
impl InfectionManager<Rsv> for Context { /* ... */ }
impl TransmissionManager<Flu> for Context {}
impl TransmissionManager<Rsv> for Context {}
```

Settings belong to the entity, so both pathogens share them.

With more than one model in scope a bare method call is ambiguous. Name the
trait and marker:

```rust
InfectionManager::<Flu>::infection_time(&context, person);
TransmissionManager::<Rsv>::seed_infections(&mut context, Person, 5);
```

The same form resolves a clash with a method of the same name in one of the
model's own traits.

## Examples

| Example                       | Shows                                                       |
| ----------------------------- | ----------------------------------------------------------- |
| `examples/core-epi`           | the smallest model: homogeneous mixing with the defaults    |
| `examples/core-epi-antiviral` | a scheduled infectiousness modifier                         |
| `examples/core-epi-settings`  | settings loaded from a line list, with a school closure     |
| `examples/core-epi-variants`  | two variants as infection details, with a vaccine           |
