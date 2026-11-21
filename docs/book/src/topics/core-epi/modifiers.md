# Modifiers

Modifiers scale a person's infectiousness or susceptibility. Each modifier is
keyed by a marker type you define: setting a modifier of the same type replaces
the previous value, while modifiers of different types multiply. See
`examples/core-epi-antiviral` for an antiviral intervention example.

## Setting a modifier now

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
[susceptibility](infections.md#susceptibility) does not exceed `1.0`.

## Scheduling a modifier

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

For both kinds:

- If two scheduled modifiers of the same type overlap, the more recently
  scheduled one takes precedence.
- A scheduled modifier multiplies with a modifier of the same type set directly.
- The matching `remove_` call removes both, and cancels any schedule of that
  type that has not started yet.

A scheduled infectiousness modifier is scoped to the person's current infection
(like a treatment): it starts only if they are still infectious with that
infection, ends if they are infected again, and does nothing for someone never
infected. A scheduled susceptibility modifier is scoped to the person, so it
survives reinfection.

## Raising infectiousness

An infectiousness factor can be above `1.0`. The transmission loop automatically
reforecasts when a person's modifier increases.

```rust
struct Coughing;

// Triples this person's infectiousness while they have a cough.
context.set_infectiousness_modifier::<Coughing>(person, 3.0);
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

## Population modifiers

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
and its closure also receives the attempt's
[details](infections.md#infection-details), here a
[variant](infections.md#variants):

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
