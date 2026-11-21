# Transmission

This page covers how infections spread: the transmission loop, contact
structure, and when a transmission loop needs a reforecast.

The snippets use the `Person` entity and `Flu` marker from the
[minimal model](../core-epi.md#a-minimal-model).

## How the loop works

Each infected person runs a loop of transmission attempt _proposals_, which are composed of a forecast and an evaluation step. Both are defined on `TransmissionManager` with these defaults:

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
to step 1. Step 3 reads the contact's
[susceptibility](infections.md#susceptibility).

The forecast _must_ be an upper bound on the rate that is eventually evaluated. A forecast that is too low causes
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

`sample_contact` also returns the
[details](infections.md#infection-details) of the infection the attempt
would cause, such as its setting or variant.

### Overriding the forecast and the evaluation

Override `forecast_total_infectiousness` and `evaluate_forecast` directly when:

- the upper bound is not a constant multiple of the intrinsic rate (for example,
  when it follows a schedule of restrictions); or
- the rate and the contact depend on the same random draw, as in
  [Changing itineraries or groups](settings.md#using-settings-in-the-loop).

Start from the defaults in [How the loop works](#how-the-loop-works) and keep the
call to `accept_proposal`, which thins and validates the bound. `InvertibleRate`
is exported from `ixa::core_epi`.

## When rates rise

After a change that can raise a person's rate, reforecast their transmission
loop:

```rust
context.reforecast_transmission(person);
```

This is always safe to call and is a no-op for uninfected people. Call it when:

- a contact restriction ends, or anything else in your `contact_multiplier` or
  forecast increases;
- you use [settings](settings.md) without `max_setting_multiplier` and a
  person's itinerary or group membership changes: reforecast that person and,
  when they join or become active in a group, its current members
  (`setting_group_members`), whose multipliers rise too;
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

[Modifiers](modifiers.md) do not require manual reforecasting (`setup_transmission_loop` handles
increases automatically), and changes that only lower a rate never require
reforecasting.
