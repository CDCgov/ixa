# ADR-0007: Introduce composable triggers

| Field | Value |
| --- | --- |
| Decision date | 2026-06-26 (merge of PR #962) |
| Recorded | 2026-07-29 |
| Status | Accepted |
| GitHub issue | [#944](https://github.com/CDCgov/ixa/issues/944) (inferred from the feature branch) |
| Pull request | [#962](https://github.com/CDCgov/ixa/pull/962) |
| Feature branch | `RobertJacobsonCDC_944_triggers` |

## Summary

Ixa introduced a trigger subsystem that separated monitored criteria from the
user-defined events emitted when those criteria matched. A
`TriggerCriterion` owned criterion-specific monitoring and produced a typed
observation. Generic trigger specifications adapted observations into ordinary
`IxaEvent`s and were installed through one context extension API.

The subsystem was designed for composition rather than only one-condition
callbacks. `TriggerSpec` admitted both ordinary criterion-to-event adapters and
a stateful `TogglingTrigger` composed from activation and deactivation
criteria. The same architecture included built-in criteria for entity counts,
property changes, property-value counts, one-shot times, and periodic times.

## Context

Ixa already had the low-level mechanisms needed to detect many model
conditions. Code could subscribe to typed events such as
`EntityCreatedEvent&lt;E&gt;` and `PropertyChangeEvent&lt;E, P&gt;`, or schedule plans at a
simulation time and execution phase. Event handlers were queued in subscription
order rather than run inline.

Using those mechanisms directly made each model reimplement several concerns:

- criterion-specific subscription or scheduling;
- local state for thresholds and one-time behavior;
- the shape of data observed when the condition matched;
- construction and emission of the model's event; and
- composition when one logical behavior depended on multiple conditions.

The first prototype represented every condition with one generic
`TriggerCriteria&lt;P&gt;` enum and every observation with one
`TriggerEvent&lt;E, P&gt;` enum. `Context::register_trigger` accepted a criterion plus
an event-construction callback. This proved the monitoring behavior, but it
carried entity and property type parameters even for time-only or
entity-count-only cases. Event constructors also had to match an observation
enum variant that was already determined by the chosen criterion. Adding new
criteria expanded the central enums and registration match.

The desired API needed to let different criteria emit the same model event,
let one criterion feed different event types, and keep criterion-specific type
dependencies local. It also needed an extension point for composite triggers,
especially an on/off trigger for hysteresis, without exposing internal marker
events as part of the model's event stream.

The underlying event system imposed important constraints:

- entity creation emitted `EntityCreatedEvent` after initial properties were
  available but did not emit `PropertyChangeEvent`;
- property writes emitted `PropertyChangeEvent` even when the value did not
  change;
- event subscriptions could not be removed; and
- handlers were single-threaded, so shared trigger state could use
  `Rc&lt;Cell&lt;_&gt;&gt;` rather than synchronization primitives.

## Decision

### Separate criteria, observations, and installable specifications

Ixa defined `TriggerCriterion` as the interface for a bare monitored
condition. Each criterion had an associated `Observation` type and installed
its monitoring logic with a callback shaped like:

```rust
Fn(&mut Context, Observation)
```

The criterion decided when a match occurred by subscribing to lower-level
events, scheduling plans, and maintaining any required local state. It did not
decide which user event to emit.

`Trigger&lt;C, Ev, F&gt;` bound one criterion to a function from its observation to a
concrete `IxaEvent`. The `emit_with`, `emit_value`, and `emit_default` helpers
on `TriggerCriterion` constructed this complete ordinary trigger. Its
`TriggerSpec` implementation installed the criterion and emitted the
constructed event on every accepted match.

`TriggerSpec` was the common interface for complete installable trigger-like
values. `ContextTriggersExt::register_trigger` accepted any `TriggerSpec`
rather than only `Trigger&lt;C, Ev, F&gt;`. Client code normally used the built-in
criteria and adapters rather than implementing these traits itself.

### Provide typed built-in criteria

The merged subsystem supplied these criterion families:

- `EntityCountTrigger&lt;E&gt;` observed creation of the entity whose ID made the
  population reach an exact positive threshold.
- `PropertyChangeTrigger&lt;E, P&gt;` matched a `from` value, a `to` value, or both.
  It defaulted to repeating and could be made one-shot.
- `PropertyValueCountTrigger&lt;E, P&gt;` detected arrival at an exact count in the
  increasing direction, decreasing direction, or either direction. It
  defaulted to repeating and could be made one-shot.
- `TimeTrigger` scheduled one match at an absolute simulation time, using
  `ExecutionPhase::Normal` unless another phase was selected.
- `PeriodicTimeTrigger` matched repeatedly at a positive finite period. Its
  first match could be at registration time, after a delay, or at an absolute
  time, and it also supported execution phases.

Every criterion exposed a dedicated observation type containing only the data
relevant to that condition. For example, a property-change observation carried
the entity ID and previous and current values, while a time observation carried
the simulation time and phase.

Property-value-count criteria initialized a trigger-local count with
`query_entity_count`, then subscribed to both entity creation and property
changes. This covered initial property values on newly created entities as well
as later transitions. Counts changed one entity at a time, so threshold
semantics were exact arrival rather than a persistent greater-than or
less-than predicate.

Periodic time criteria did not call the public periodic-plan helper for their
first occurrence. That helper always seeded a plan at absolute time zero and
could not implement delayed, absolute, or post-start registration safely.
Instead, the criterion scheduled its first occurrence explicitly with
`add_plan_with_phase`, then reused the context's internal periodic
rescheduling helper.

### Compose criteria with a toggling trigger

`TogglingTrigger` implemented `TriggerSpec` directly. It combined:

- an activation criterion and activation-event constructor;
- a deactivation criterion and deactivation-event constructor; and
- shared active and enabled state.

The trigger started inactive and repeating by default. It accepted activation
matches only while inactive and deactivation matches only while active. It
updated its state before emitting the corresponding event. In one-shot mode,
the trigger disabled itself after completing one active period; an initially
active one-shot trigger completed that period on its first accepted
deactivation.

`TogglingTriggerCriteria` provided the ordinary builder flow for a pair of bare
criteria. Its `emit_with`, `emit_values`, and `emit_defaults` methods produced a
complete `TogglingTrigger`. The all-at-once `TogglingTrigger::new` constructor
remained available. Component criteria were normally expected to repeat,
because a one-shot component could consume itself on a match that the
toggling state rejected.

## Rationale

The criterion/observation split encoded each condition's data dependencies in
the type system. A time criterion no longer needed placeholder entity and
property types, and an event constructor no longer had to unpack a central
observation enum. The same criterion could be adapted to different model
events, and several criteria could intentionally emit the same event type.

Having criteria report matches to context-aware handlers, rather than emit
events themselves, made them reusable building blocks. The ordinary `Trigger`
adapter used the handler for unconditional event emission. `TogglingTrigger`
used the same interface to gate matches and manage state without routing
through library-defined marker events.

Keeping `TriggerSpec` separate from the concrete `Trigger` wrapper added a
public abstraction, but it preserved a single registration API as composite
specifications were added. The toggling implementation exercised this
extension point in the same feature rather than leaving its value hypothetical.

The built-in criteria reused established event and plan behavior. This kept
trigger installation additive: criteria subscribed or scheduled work without
adding parallel mutation hooks to the entity or execution subsystems.

## Consequences

- Models could express conditions independently of the events and handlers that
  responded to them.
- Typed observations removed irrelevant generic parameters and enum-variant
  matching from ordinary trigger construction.
- Custom model events remained ordinary `IxaEvent`s, preserving existing event
  subscription and queue-ordering behavior.
- New criterion types and new composite `TriggerSpec` implementations could be
  added without changing `ContextTriggersExt::register_trigger`.
- Stateful hysteresis and other on/off behavior could be built by composing
  arbitrary criterion types, including criteria with different observation and
  emitted event types.
- The public API gained several layers—criterion, observation, adapter,
  specification, and registration—that maintainers had to distinguish.

Trigger installation also inherited costs and limitations from the low-level
mechanisms:

- Installing a property-change-based criterion created a subscriber and
  disabled the `set_property` fast path for properties with no change-event
  subscribers.
- There was no subscriber-removal API. One-shot triggers stopped producing
  matches but left inactive handlers installed, so dynamically accumulating
  triggers could accumulate subscription overhead.
- Each property-value-count trigger maintained its own count. Initial
  registration could require a linear query; indexed properties duplicated
  count maintenance; and separate triggers did not share state.
- The local count avoided requiring `IndexableProperty` and tracked only the
  selected value rather than every value, trading shared indexing efficiency
  for a simpler and more general criterion.
- Periodic triggers followed the existing periodic-plan lifetime behavior:
  rescheduling continued only while the execution queue still contained work.
- A toggling trigger could ignore a component match because of its current
  state, but a one-shot component criterion could still be consumed by that
  match.

## Alternatives considered

### Retain the monolithic criterion and observation enums

The prototype was direct and exhaustive, but unrelated criteria inherited the
same generic parameters and event constructors had to unpack a shared
observation enum. Every new criterion expanded central matching logic. Separate
criterion types with associated observations kept dependencies and extension
points local.

### Make `register_trigger` accept only `Trigger&lt;C, Ev, F&gt;`

This would have removed the `TriggerSpec` abstraction and covered ordinary
one-criterion triggers. It could not represent the adopted toggling trigger
without adding a separate registration method or forcing the composite into
the shape of one criterion and one event. A common specification trait kept
registration independent of concrete wrapper shape.

### Let criteria construct and emit events directly

The initial typed design considered passing an event constructor into
`TriggerCriterion::install`. That coupled criterion installation to
unconditional event emission. Changing installation to report matches through
a context-aware handler allowed ordinary and composite specifications to reuse
the same criteria.

### Compose toggling behavior through internal events

Activation and deactivation criteria could have emitted library marker events,
with another subscriber maintaining toggle state. This would have introduced
extra event identities and queue steps and made criteria less directly
composable. Handler-based installation kept the gating state inside one
`TriggerSpec`.

### Reuse property indexes for value-count criteria

An index could share counts across consumers and avoid repeated scans when
already present. It would also track all values, require `Eq + Hash`, and still
need trigger-specific crossing and mode state. The adopted local counter worked
for every `Property` and stored only the tracked value's count.

### Seed periodic triggers with `add_periodic_plan_with_phase`

That public helper scheduled its first occurrence at absolute time zero. It
could not safely register after time had advanced or provide delayed and
absolute first occurrences. Explicitly scheduling the first match and then
using the existing rescheduling helper preserved periodic behavior without
changing the public plan API.

## References

- [Issue #944](https://github.com/CDCgov/ixa/issues/944)
- [PR #962: Triggers](https://github.com/CDCgov/ixa/pull/962)
- [Initial prototype commit `803e666`](https://github.com/CDCgov/ixa/commit/803e666a0a4c7119bd6d0dc6bcd7c793bef1b135)
- [Criterion/specification split commit `c8c91df`](https://github.com/CDCgov/ixa/commit/c8c91df65bfb290f352e3c52f85c429c6329a2ed)
- [Toggling trigger commit `97fc3f7`](https://github.com/CDCgov/ixa/commit/97fc3f7dd15b0353e6b3384cc95a7cc3cbfffae7)
- [Aligned builder commit `932066b`](https://github.com/CDCgov/ixa/commit/932066bb7a13aa60272000d50f1e2ac1c32f3566)
- [Periodic time criterion commit `bf31a24`](https://github.com/CDCgov/ixa/commit/bf31a24ae765ca77c0a069378dd8670f859c7298)
- [Retained aggregate feature commit `8331ecc`](https://github.com/CDCgov/ixa/commit/8331eccaf82d181b1dd27296b282c644771162c5)
- [Merged commit `e7bddf0`](https://github.com/CDCgov/ixa/commit/e7bddf055f422469b9087a8547e5cda8c0144b3f)

The trigger files in `8331ecc` and `e7bddf0` are identical; their complete
trees differ because their respective bases contain unrelated context changes.
The later commits `a422158` and `d5644de` correspond to the same documentation
fix, merged separately as PR #988, and do not change the architectural
decision.
