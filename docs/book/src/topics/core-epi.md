# `core_epi`

The `core_epi` module provides reusable building blocks for epidemiological
models: infectiousness rate functions, a continuous-time transmission loop,
modifiers, and contact settings, with more features planned over time.

The module is gated behind the `core_epi` Cargo feature, which is enabled by
default. The examples in this section also use the `rand_distr` crate to draw
durations.

This page builds a minimal model. The rest of the section covers:

- [Transmission](core-epi/transmission.md): how the loop works, contact
  structure, and when to reforecast.
- [Infections](core-epi/infections.md): infectiousness, infection details,
  susceptibility, variants, and infection history.
- [Modifiers](core-epi/modifiers.md): scaling infectiousness or susceptibility
  for a person or for everyone.
- [Settings](core-epi/settings.md): households, schools and other places
  where contact happens.

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

[Settings](core-epi/settings.md) belong to the entity, so both pathogens share
them.

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
