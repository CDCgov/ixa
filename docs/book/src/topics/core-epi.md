# Epidemic Models with `core_epi`

The `core_epi` module includes a set of epi features: so far, definitions of infectiousness rates and related utilities, a standard continuous-time transmission loop, and settings. We intend to expand the feature set over time.

The module is behind the `core_epi` cargo feature, which is on by default. The
examples on this page also use the `rand_distr` crate to draw durations.

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
    // Nothing to record about how someone was exposed: see "Contact structure".
    type Exposure = ();

    // How infectious this person is over the course of their infection.
    fn draw_infectiousness_rate_fn(&mut self, _person: PersonId) -> RateAssignment {
        let infectious_period = self.sample_distr(InfectiousPeriodRng, Exp::new(1.0 / 5.0).unwrap());
        ConstantRateFn::new(0.4, infectious_period).into()
    }

    // The model's own bookkeeping.
    fn on_infected(&mut self, _infector: Option<PersonId>, infectee: PersonId, _exposure: ()) {
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

Each infected person runs their own loop of attempt _proposals_. One turn of it,
simplified from the implementation:

```rust
// 1. Forecast: an upper bound on the person's total infectiousness, as a rate
//    function of time from now. Inverting an Exp(1) draw on it gives the delay
//    to the next proposal. No delay means the forecast ran out: the loop ends.
let forecast_fn = context.forecast_total_infectiousness(person);
let delay = forecast_fn.inverse_cum_rate(exp_draw)?;
let forecast = forecast_fn.rate(delay);

// ... `delay` later ...

// 2. Evaluate: thin the proposal to an attempt and draw the contact.
if let Some((contact, exposure)) = context.evaluate_forecast(person, forecast) {
    // 3. The attempt transmits with the contact's susceptibility.
    let susceptibility = context.susceptibility(contact);
    if context.sample_bool(CoreEpiTransmissionRng, susceptibility) {
        context.transmit(Some(person), contact, exposure);
    }
}
// Back to 1.
```

Steps 1 and 2 are `TransmissionManager` methods, and step 3 reads an
`InfectionManager` method. Their defaults are:

```rust
fn forecast_total_infectiousness(&self, person: PersonId) -> impl InvertibleRate {
    ScaledRateFn::new(
        // The person's rate function, shifted so that 0 is now. Modifiers
        // that lower the rate are left out, so this is a bound; modifiers that
        // multiply to more than 1 scale it up.
        self.majorizing_infectiousness_rate_fn(person),
        self.max_contact_multiplier(person), // default: contact_multiplier(person)
    )
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

So with nothing overridden, a person proposes at the rate of their own rate
function, a proposal is accepted with probability equal to the product of their
infectiousness modifiers, and the contact is anyone else in the population. The
loop ends when the rate function runs out.

A proposal that is not accepted, or an attempt that does not transmit, costs
nothing but the draw: the loop goes back to step 1 either way.

The forecast must be an upper bound. Thinning in step 2 corrects for the
difference, so anything that lowers infectiousness after the proposal was drawn
(a treatment, a mask, a contact restriction) is handled without rescheduling. A
forecast that is too low is an error, and `accept_proposal` panics rather than
under-count attempts. See [When rates rise](#when-rates-rise).

## Infectiousness

A person's infectiousness is a rate function of time since infection, assigned
when they are infected and fixed for that infection. `draw_infectiousness_rate_fn`
returns a `RateAssignment`, which is one of:

- an individually-assigned rate function, for example a constant rate over a
  period drawn for them. The built-in rate functions convert with `.into()`.
- `RateAssignment::Library(index)`, an index into a library of shared curves
  loaded once at setup with `load_infectiousness_rate_library`. Use this when
  many people share a few curves.

The built-in rate functions are `ConstantRateFn::new(rate, duration)` and
`EmpiricalRateFn::new(points, scale)`, a piecewise-linear curve through points
given as time and rate pairs.

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

`core_epi` has no notion of recovery or of disease states: you must track those yourself. For example, to implement a recovered state (a `Recovered` variant added to the status above), schedule it from `on_infected`:

```rust
fn on_infected(&mut self, _infector: Option<PersonId>, infectee: PersonId, _exposure: ()) {
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

This starts their transmission loop but does not call `on_infected`, so do
whatever the model does there (set a status, schedule recovery) yourself.

## Susceptibility

`base_susceptibility` is the probability that an attempt on a person succeeds without any modifiers, and `susceptibility(person)` is the realized probability including any modifiers.

The default `base_susceptibility` is permanent immunity: 1 for someone never
infected and 0 once they have been infected. If you define [more than one pathogen](#more-than-one-pathogen), immunity to each is separate.

```rust
fn base_susceptibility(&self, person: PersonId) -> f64 {
    if self.infection_time(person).is_none() {
        1.0
    } else {
        0.0
    }
}
```

`susceptibility` is the person's base susceptibilty times the product of the person's susceptibility
[modifiers](#modifiers) (their `susceptibility_multiplier`):

```rust
fn susceptibility(&self, person: PersonId) -> f64 {
    self.base_susceptibility(person) * self.susceptibility_multiplier(person)
}
```

Susceptibility is the only check the loop makes on a contact. It does not look at
whether they are currently infected, so a `base_susceptibility` that is positive
for someone already infected lets them be infected again: they get a new rate
function, which replaces the old one.

Susceptibility only decides the last step. The infector's proposal must first be
accepted as an attempt, and this person must be the contact drawn. A
susceptibility of 1 means that such an attempt always infects them.

Susceptibility is a probability: the loop panics on a value outside 0 to 1. For
relative susceptibility, give the most susceptible group 1 and put the scale in
the infectiousness rate.

Override `base_susceptibility` for anything else, for example children who are
half as susceptible as adults:

```rust
define_property!(
    enum AgeGroup {
        Child,
        Adult,
    },
    Person,
    default_const = AgeGroup::Adult
);

fn base_susceptibility(&self, person: PersonId) -> f64 {
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

Interventions change infectiousness and susceptibility through modifiers: factors
that multiply together. Each is identified by a type of the model's choosing, so
setting a modifier again replaces its factor and different modifiers compose.

```rust
struct Antiviral;
struct Vaccine;

struct Mask;

// Halves this person's infectiousness until removed.
context.set_infectiousness_modifier::<Mask>(person, 0.5);

// Halves this person's susceptibility until removed.
context.set_susceptibility_modifier::<Vaccine>(person, 0.5);

// Scales this infection's infectiousness to 25% from one day in, until it ends.
let start = context.get_current_time() + 1.0;
context.schedule_infectiousness_modifier::<Antiviral>(person, 0.25, start, None);
```

A modifier set with `set_infectiousness_modifier` or `set_susceptibility_modifier`
belongs to the person. It can be set before they are ever infected, stays through
any number of infections, and ends only when the model removes it. The two kinds
differ in the factors they take:

- An **infectiousness modifier** may be any finite non-negative factor. A
  factor above 1 raises the person's rate, and the forecast with it: when a
  change takes the product of their modifiers above 1, the loop forecasts again
  by itself.
- A **susceptibility modifier** may be any finite non-negative factor, as long
  as the person's [susceptibility](#susceptibility) stays at most 1.

`schedule_infectiousness_modifier` is the exception: it is for an effect on one
infection, such as a treatment. It applies only if the person is still infectious
with the same infection at the start time, and it is dropped if they are infected
again. Called for someone never infected, it does nothing.
`schedule_susceptibility_modifier` has no such tie: it sets a modifier on the
person at the start time and removes it at the end time.

Both kinds also have a `remove_` form and a `set_..._fn` form, whose factor is
computed on every read from the context, the person and the time elapsed since
the modifier was set. That is for effects that build up or wane:

```rust
// Protection that wanes linearly over 180 days.
context.set_susceptibility_modifier_fn::<Vaccine>(person, |_context, _person, elapsed| {
    (0.5 + 0.5 * elapsed / 180.0).min(1.0)
});
```

An infectiousness function must return values between 0 and 1, because nothing
tells the loop when a function rises. For one that goes higher, declare its
maximum:

```rust
// A surge that builds to three times the usual rate over two days.
context.set_infectiousness_modifier_fn_with_max::<Surge>(person, 3.0, |_context, _person, elapsed| {
    (1.0 + elapsed).min(3.0)
});
```

The person is then forecast at the maximum throughout and the excess is
rejected, so a maximum far above the usual value costs proposals. For a factor
that changes at known times, scheduling plain `set_infectiousness_modifier` calls
is cheaper: each rise is forecast exactly.

For a factor that follows the course of the infection instead, read
`time_since_infection(person)` inside the closure. It is `None` for someone never
infected.

`examples/core-epi-antiviral` runs the same outbreak with and without an
antiviral.

## Contact structure

`TransmissionManager` decides who is contacted and how contact structure changes
the rate. Most models describe it with three methods, each with a default:

| Method                           | Returns                                               | Default                |
| -------------------------------- | ----------------------------------------------------- | ---------------------- |
| `contact_multiplier(person)`     | the factor on their intrinsic infectiousness now      | 1                      |
| `max_contact_multiplier(person)` | an upper bound on that factor until their next proposal | the multiplier now   |
| `sample_contact(person)`         | who they contact in an attempt, and the exposure      | anyone else, uniformly |

The loop forecasts on the intrinsic bound times `max_contact_multiplier`, thins by
`contact_multiplier`, and asks `sample_contact` for each attempt.

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

The rate is set by `contact_multiplier` alone. `max_contact_multiplier` only says
how far it can rise before the person's next proposal, and must be finite: leave
it at its default when the multiplier does not change during an infection, or
when every rise is followed by a [reforecast](#when-rates-rise). Override it with
a larger constant when the multiplier can rise without one. A bound larger than
needed costs rejected proposals; it never raises the rate.

`sample_anyone_else(person)` is the default draw, for a model that falls back to
the whole population.

### Recording how someone was exposed

`sample_contact` returns the contact together with an _exposure_: whatever the
model wants to know in `on_infected` about how the infection happened, such as
the setting it happened in. Its type is `InfectionManager::Exposure`. A model
with nothing to record uses `()`. A seeded infection (`seed_infection`,
`seed_infections`) gets the type's default value.

```rust
impl InfectionManager<Flu> for Context {
    type Entity = Person;
    type Exposure = Option<Setting>;

    fn on_infected(&mut self, infector: Option<PersonId>, infectee: PersonId, exposure: Option<Setting>) {
        /* for example, write a row of a transmission report */
    }
}
```

### Overriding the forecast and the evaluation

The three methods cover a contact multiplier with a constant bound. Two cases
need the level below them, `forecast_total_infectiousness` and
`evaluate_forecast`:

- the bound is not a constant multiple of the intrinsic one, for example it
  follows a schedule of restrictions;
- the total and the contact must come from the same draw, as on the
  setting-first path [below](#using-settings-in-the-loop).

Start from the defaults in [How the loop works](#how-the-loop-works), with
`InvertibleRate` and `ScaledRateFn` imported from `ixa::core_epi`. Keep the call
to `accept_proposal`: it does the thinning and checks the bound.

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

**Exact.** The person's total over all their settings is their contact
multiplier, and the setting is drawn in proportion to its part of that total.
The forecast defaults to the total itself, so no proposal is rejected on account
of contact structure.

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

That forecast is only valid while the multiplier stays as it was when the
forecast was made. To keep the same draw when memberships and itineraries do
change, add the bound that survives those changes:

```rust
    fn max_contact_multiplier(&self, person: PersonId) -> f64 {
        self.max_setting_multiplier::<Setting>(person)
    }
```

**Setting first.** Draw where the attempt happens, then thin by that setting's
multiplier. The forecast is the largest multiplier any of the person's groups has
ever had, which stays valid when members are removed and restored and when
itineraries are overridden and cleared. The setting has to be shared between the
thinning and the contact draw, so this path overrides `evaluate_forecast`.

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

The exact path with its default forecast is the fastest, because it proposes at
the person's actual rate. With `max_setting_multiplier` as the bound, either
path proposes at the rate of the person's largest setting and rejects the
difference, and in exchange needs no care when members are removed and restored
or itineraries change.

`examples/core-epi-settings` loads households, schools and workplaces from a line
list, runs the setting-first path, and counts infections by the setting they
happened in.

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

- an infectiousness modifier rising above 1, but only in a model that does not
  call `setup_transmission_loop`: with it, the loop does this itself;
- on the exact settings path with its default forecast, any change to the
  person's itinerary or to who is in any of their groups;
- with `max_setting_multiplier` as the bound, a group growing past its largest
  size, or a person joining a group, for the newcomer and for the group's
  infectious members (`setting_group_members`);
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
