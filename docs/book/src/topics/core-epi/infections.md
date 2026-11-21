# Infections

This page covers a person's infection and who can be infected: infectiousness,
infection details, susceptibility, variants and infection history.

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
example, adding a `Recovered` status to the
[minimal model](../core-epi.md#a-minimal-model):

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

## Infection details

Besides the contact, [`sample_contact`](transmission.md#contact-structure)
returns the _details_ of the infection the attempt would cause: any extra
metadata the model tracks beyond who and when, such as the setting or
[variant](#variants). The type is `InfectionManager::Details`; use `()` if none
is needed. The details are passed to:

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

## Susceptibility

`susceptibility(person, infector, &details)` returns the probability that a
transmission attempt infects `person`, computed as `base_susceptibility`
multiplied by any susceptibility [modifiers](modifiers.md). Both `susceptibility`
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

A [susceptibility modifier](modifiers.md#factors-that-change-over-time)
closure can also depend on the variant:

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

`infection_details(person)` is the current infection only. That suffices above
because `Escape` protects against both variants; if immunity depends on earlier
infections too, read the [infection history](#infection-history).

Model a more transmissible variant with a higher infectiousness rate function
rather than a susceptibility above `1.0`.

## Infection history

A reinfection replaces a person's infection, and the replaced one is kept as a
past infection. `infection_count(person)` is how many infections the person has
had, and `past_infections(person)` lists the ones before the current one:

```rust
impl InfectionManager<Flu> for Context {
    type Entity = Person;
    type Details = Variant;

    // Immune to every variant the person has had.
    fn base_susceptibility(&self, person: PersonId, _infector: Option<PersonId>, variant: &Variant) -> f64 {
        let has_it = self.infection_details(person) == Some(variant);
        let had_it = self
            .past_infections(person)
            .iter()
            .any(|past| past.details == *variant);
        if has_it || had_it { 0.0 } else { 1.0 }
    }

    // ...
}
```

The list is oldest first. Each `PastInfection` has its `time`, its `details` and
its `infectiousness_end`: when its rate function ran out, or the time of the
reinfection if that came first. The current infection is not in the list; read
it with `infection_time(person)` and `infection_details(person)`.

History costs nothing for a person infected once. Each reinfection adds one
record, and records are never removed.
