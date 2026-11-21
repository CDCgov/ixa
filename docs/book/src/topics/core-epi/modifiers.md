# Modifiers

Modifiers scale a person's infectiousness or susceptibility. Each modifier has
a _kind_: setting a modifier of the same kind replaces the previous value, while
modifiers of different kinds multiply. See `examples/core-epi-antiviral` for an
antiviral intervention example.

## Modifier kinds

`define_modifier!` declares a kind, which is passed by value to the methods
below. Each module of a model can declare its own:

```rust
use ixa::define_modifier;

define_modifier!(pub Mask);
```

A kind can also be added at run time, for an intervention named in a parameters
file. `add_modifier_kind` returns a `ModifierId`, which the same methods accept:

```rust
use ixa::core_epi::ModifiersExt;

// `interventions` holds a name and a factor each, e.g. ("ventilation", 0.8).
for (name, factor) in interventions {
    let kind = context.add_modifier_kind(&name);
    context.set_population_infectiousness_modifier(kind, move |_context, _person| factor);
}

// Later, by name.
let ventilation = context.modifier_kind_by_name("ventilation").unwrap();
context.remove_population_infectiousness_modifier(ventilation);
```

`modifier_kinds()` lists the kinds added at run time and `modifier_name(kind)`
gives the name of one. Their names are unique within a context; declared kinds
have no name to collide with. A kind is not tied to infectiousness or
susceptibility: the same kind can key one modifier of each.

## Setting a modifier

`set_infectiousness_modifier` and `set_susceptibility_modifier` take effect
immediately and persist until the matching `remove_` call, across any number of
infections. They can be set before the person is ever infected.

```rust
define_modifier!(Mask);
define_modifier!(Vaccine);

// Halves this person's infectiousness until removed.
context.set_infectiousness_modifier(person, Mask, 0.5);

// Halves this person's susceptibility until removed.
context.set_susceptibility_modifier(person, Vaccine, 0.5);

context.remove_infectiousness_modifier(person, Mask);
```

A factor can be any finite non-negative number, provided the resulting
[susceptibility](infections.md#susceptibility) does not exceed `1.0`.

## Scheduling a modifier

The `schedule_` methods apply a modifier at a start time and optionally remove
it at an end time. A schedule does not keep the simulation running: if nothing
else is planned before its start or end, the simulation ends first.

```rust
define_modifier!(Antiviral);
define_modifier!(Vaccine);

// Scales infectiousness to 25% from one day in, for the rest of the infection.
let now = context.get_current_time();
context.schedule_infectiousness_modifier(person, Antiviral, 0.25, now + 1.0, None);

// Halves susceptibility from day 14 to day 194.
context.schedule_susceptibility_modifier(person, Vaccine, 0.5, 14.0, Some(194.0));
```

For both infectiousness and susceptibility:

- If two scheduled modifiers of the same kind overlap, the more recently
  scheduled one takes precedence.
- A scheduled modifier multiplies with a modifier of the same kind set directly.
- The matching `remove_` call removes both, and cancels any schedule of that
  kind that has not started yet.

A scheduled infectiousness modifier is scoped to the person's current infection
(like a treatment): it starts only if they are still infectious with that
infection, ends if they are infected again, and does nothing for someone never
infected. A scheduled susceptibility modifier is scoped to the person, so it
survives reinfection.

## Raising infectiousness

An infectiousness factor can be above `1.0`. The transmission loop automatically
reforecasts when a person's modifier increases.

```rust
define_modifier!(Coughing);

// Triples this person's infectiousness while they have a cough.
context.set_infectiousness_modifier(person, Coughing, 3.0);
```

If your model does not use `setup_transmission_loop`, you must
[reforecast](transmission.md#when-rates-rise) manually.

## Factors that change over time

The `set_*_fn` methods accept a closure evaluated on every read with the
context, the person, and the time elapsed since the modifier was set.
Susceptibility closures also receive the attempt's
[details](infections.md#infection-details):

```rust
// Protection that wanes linearly over 180 days.
context.set_susceptibility_modifier_fn(person, Vaccine, |_context, _person, elapsed, _details| {
    (0.5 + 0.5 * elapsed / 180.0).min(1.0)
});
```

`StepFn` represents a piecewise-constant factor:

```rust
use ixa::core_epi::StepFn;

// 80% protection for 90 days, then 50%.
let protection = StepFn::new(0.2).step_at(90.0, 0.5);
context.set_susceptibility_modifier_fn(person, Vaccine, move |_context, _person, elapsed, _details| {
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
context.set_infectiousness_modifier_fn_with_max(person, Coughing, 3.0, |_context, _person, elapsed| {
    (1.0 + elapsed).min(3.0)
});
```

The person is forecast at that maximum throughout, so a loose bound causes more
rejected proposals. If changes occur at known times, scheduling plain
`set_infectiousness_modifier` calls is more efficient.

## Population modifiers

A population modifier is registered once and applies to everyone. Its closure is
called on every read with the context and the person, so it can depend on a
property:

```rust
define_modifier!(Masking);

// Anyone wearing a mask is half as infectious.
context.set_population_infectiousness_modifier(Masking, |context, person| {
    if context.get_property::<_, Masked>(person).0 { 0.5 } else { 1.0 }
});
```

Or it can depend on simulation time, for a population-wide intervention:

```rust
define_modifier!(Lockdown);

// Contact cut to a quarter from day 20 to day 35.
let contact = StepFn::new(1.0).step_at(20.0, 0.25).step_at(35.0, 1.0);
context.set_population_infectiousness_modifier(Lockdown, move |context, _person| {
    contact.at(context.get_current_time())
});
```

Population modifiers multiply with per-person modifiers. Setting the same kind
again replaces the previous closure, and
`remove_population_infectiousness_modifier` removes it.

As with per-person closures, a population infectiousness modifier must stay in
`[0, 1]` so properties or time can change without reforecasting. If it can
exceed `1.0`, use `set_population_infectiousness_modifier_with_max` to declare
an upper bound; everyone is then forecast at that maximum.

`set_population_susceptibility_modifier` works the same way for susceptibility,
and its closure also receives the attempt's
[details](infections.md#infection-details), here a
[variant](infections.md#variants):

```rust
// Masks protect the wearer less against the escape variant.
context.set_population_susceptibility_modifier(Masking, |context, person, variant| {
    if !context.get_property::<_, Masked>(person).0 {
        return 1.0;
    }
    match variant {
        Variant::Original => 0.6,
        Variant::Escape => 0.8,
    }
});
```
