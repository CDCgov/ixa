# Settings

Settings organize people into contact contexts using three concepts: _kinds_ of
places (such as household or school), _groups_ that are specific instances of a
kind (such as a particular household), and a per-person _itinerary_ that divides
their time across kinds.

The snippets use the `Person` entity and `Flu` marker from the
[minimal model](../core-epi.md#a-minimal-model).

`define_settings!` defines the setting-kind type for an entity, and the kinds
the model names in code:

```rust
use ixa::core_epi::{Itinerary, SettingsExt};
use ixa::define_settings;

define_settings!(pub Setting for Person {
    Household,
    Community,
});

context.set_setting_alpha(Setting::Household, 1.0);
context.set_setting_alpha(Setting::Community, 0.0);
```

Each declared kind is a constant of the type, such as `Setting::Household`, that
can be used anywhere in the model, including in a `match`. Its name is the
identifier unless one is given (`Household = "household"`).

`alpha` is the density exponent: in a group of `n` people, a member's
infectiousness is multiplied by `(n - 1)^alpha`. With `alpha = 1`, attempts
scale linearly with group size; with `alpha = 0`, they are independent of group
size. For a group of size 1, the multiplier is 0. A declared kind needs its
`alpha` set before its first group is created.

Kinds can also be added at runtime, for example from a parameters file. They
come after the declared ones, and `define_settings!(pub Setting for Person)`
with no list leaves all of them to runtime:

```rust
let church = context.add_setting_kind::<Setting>("church", 0.5);
assert_eq!(context.setting_kind_by_name::<Setting>("church"), Some(church));
```

## Groups and members

```rust
// One group at a time.
let home = context.create_setting_group(Setting::Household);
context.add_setting_member(home, person);

// Or by an external ID, when loading a line list. The group is created the
// first time its ID appears.
let home = context.setting_group_for_key(Setting::Household, "HH001");
context.add_setting_member(home, person);

// A group everyone is in, with no member list to store.
context.create_population_setting_group(Setting::Community);
```

A person can belong to at most one group of each kind. Use
`deactivate_setting_member` and `activate_setting_member` to mark a member
inactive and active again (for example, during a school closure).

## Itineraries

```rust
context.set_itinerary_assignment(Itinerary::new([
    (Setting::Household, 0.6),
    (Setting::Community, 0.4),
]));
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
context.override_itinerary(person, Itinerary::new([(Setting::Household, 1.0)]));
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

## Using settings in the loop

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
members are the exception; see
[When rates rise](transmission.md#when-rates-rise).)

For the person above, `max_setting_multiplier` is 3 (the household multiplier).
If they isolate at home, their itinerary becomes 100% household and their rate
rises from 2.2x to 3x intrinsic, which the forecast already covers.

Each proposal draws a setting according to itinerary share and thins by
`setting_multiplier / max_setting_multiplier` (3/3 for household, 1/3 for
community). Because thinning and contact sampling both use the drawn setting,
this approach
[overrides `evaluate_forecast`](transmission.md#overriding-the-forecast-and-the-evaluation).

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
