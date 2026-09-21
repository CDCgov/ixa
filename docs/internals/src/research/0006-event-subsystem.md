# Research-0006: Event subsystem research

**Examined revision date:** 2026-06-29 (best estimate from the source artifact)

> This is a historical snapshot of the event subsystem at the examined
> revision, not a maintained guide to the current implementation.

This note describes the existing event subsystem as implemented today. It intentionally excludes the `triggers` module, even though triggers are built on top of the same event APIs in places.

## Scope

In this note, "event" means an `IxaEvent` emitted through `Context::emit_event` and observed through `Context::subscribe_to_event`. This excludes plans, profiling events, reports, trigger-specific abstractions, and anything else that may colloquially be called an event but does not participate in `Context::event_handlers`.

Primary implementation points:

- `src/context.rs`: `IxaEvent`, `Context::event_handlers`, `Context::callback_queue`, `subscribe_to_event`, `emit_event`, `queue_callback`, and the execution loop.
- `src/entity/events.rs`: built-in `EntityCreatedEvent&lt;E&gt;` and `PropertyChangeEvent&lt;E, P&gt;`, plus partial property-change machinery.
- `src/entity/context_extension.rs`: emission sites for entity creation and property writes.
- `ixa-derive/src/lib.rs`: the `#[derive(IxaEvent)]` implementation.

## Public model

An event's identity is its concrete Rust type. Client code subscribes to a type `E: IxaEvent`, and it receives every emitted value of exactly that type.

The public API is implemented directly on `Context`:

```rust
pub fn subscribe_to_event<E: IxaEvent>(
    &mut self,
    handler: impl Fn(&mut Context, E) + 'static,
)

pub fn emit_event<E: IxaEvent>(&mut self, event: E)
```

There is no event name, string key, topic object, predicate, or dynamic runtime filter. `PropertyChangeEvent&lt;Person, Age&gt;` and `PropertyChangeEvent&lt;Person, InfectionStatus&gt;` are distinct event types because their generic parameters are part of their concrete Rust type. Likewise, `EntityCreatedEvent&lt;Person&gt;` and `EntityCreatedEvent&lt;Animal&gt;` are distinct event types.

The event system is asynchronous with respect to the call to `emit_event`: emitting an event does not call handlers immediately. Instead, `emit_event` creates callbacks and pushes them onto `Context::callback_queue`. Those callbacks later invoke the handlers.

## Storage layout

`Context` stores event subscriptions in:

```rust
event_handlers: HashMap<TypeId, Box<dyn Any>>
```

For a particular event type `E`, the value stored under `TypeId::of::&lt;E&gt;()` is a type-erased:

```rust
Vec<Rc<EventHandler<E>>>
```

where:

```rust
type EventHandler<E> = dyn Fn(&mut Context, E);
```

The `Box&lt;dyn Any&gt;` is the type-erasure layer that allows one map to hold handler vectors for many different event types. The `TypeId` key and the erased value must agree. `subscribe_to_event::&lt;E&gt;` inserts or retrieves the value for `TypeId::of::&lt;E&gt;()` and downcasts it to `Vec&lt;Rc&lt;EventHandler&lt;E&gt;&gt;&gt;`. `emit_event::&lt;E&gt;` looks up the same key and downcasts the stored value back to `Vec&lt;Rc&lt;EventHandler&lt;E&gt;&gt;&gt;`.

This is type-safe by construction as long as the only code writing `event_handlers` uses the same `TypeId::of::&lt;E&gt;()` key and `Vec&lt;Rc&lt;EventHandler&lt;E&gt;&gt;&gt;` value convention. The downcasts use `unwrap()`, so violating that internal invariant would panic.

Handlers are stored in subscription order. When an event is emitted, `emit_event` iterates the vector in order and pushes one callback per handler. Therefore handlers for a single emitted event run in subscription order, subject to normal callback queue FIFO behavior.

Subscribing after an event has already been emitted does not observe that prior event. `emit_event` snapshots the currently registered handler list only by immediately queueing callbacks for the handlers present at emission time.

## Emission path

For `emit_event::&lt;E&gt;(event)`:

1. `Context` is destructured to borrow `event_handlers` and `callback_queue` as disjoint fields.
2. The implementation looks up `event_handlers.get(&amp;TypeId::of::&lt;E&gt;())`.
3. If there is no entry, nothing is queued.
4. If an entry exists, it is downcast to `&amp;Vec&lt;Rc&lt;EventHandler&lt;E&gt;&gt;&gt;`.
5. For each handler, the `Rc` is cloned.
6. A boxed `FnOnce(&amp;mut Context)` callback is pushed to the back of `callback_queue`.
7. That queued callback later calls `handler_clone(context, event)`.

Because `E: Copy`, the same event value can be captured into every queued callback. Each callback receives a copy of the event value.

## Subscription path

For `subscribe_to_event::&lt;E&gt;(handler)`:

1. The implementation finds the entry for `TypeId::of::&lt;E&gt;()`.
2. If no entry exists, it inserts an empty `Vec&lt;Rc&lt;EventHandler&lt;E&gt;&gt;&gt;` inside `Box&lt;dyn Any&gt;`.
3. It downcasts the erased entry to `&amp;mut Vec&lt;Rc&lt;EventHandler&lt;E&gt;&gt;&gt;`.
4. It wraps the handler in `Rc` and pushes it onto the vector.
5. It calls `E::on_subscribe(self)`.

`IxaEvent::on_subscribe` defaults to a no-op. It gives an event type a hook that runs every time a subscription is registered. The built-in event types currently use the derived default implementation.

## Why handlers are stored in `Rc`

The handler itself has type `dyn Fn(&amp;mut Context, E)`, not `FnOnce`. A subscription is persistent: the same handler must be callable for every future emission of that event type.

However, the callback queue stores boxed `FnOnce(&amp;mut Context)` callbacks:

```rust
type Callback = dyn FnOnce(&mut Context);
```

Each queued event callback must own everything it will need later, because it may execute after `emit_event` returns. It cannot borrow the handler vector while waiting in the queue. It also cannot move the subscribed handler out of the vector, because the handler must remain registered for later events.

`Rc` is the mechanism that satisfies both requirements:

- The handler vector keeps one owned `Rc` for the durable subscription.
- Each emitted event callback receives an owned clone of that `Rc`.
- Cloning an `Rc` is cheap and does not clone the closure itself.
- The queued `FnOnce` can move its `Rc` into the callback and invoke the underlying `Fn`.
- The same handler remains available for subsequent event emissions.

`Rc`, rather than `Arc`, matches the rest of `Context`: the simulation context is single-threaded and callbacks take `&amp;mut Context`. There is no cross-thread callback execution model in this subsystem, so atomic reference counting would add cost without adding useful capability.

## Why `IxaEvent: Copy + 'static`

`IxaEvent` is defined as:

```rust
pub trait IxaEvent: Copy + 'static {
    fn on_subscribe(_context: &mut Context) {}
}
```

`Copy` is required by the current queuing design. One emitted event value fans out to zero or more handlers, and each handler gets a by-value `E`. Since callbacks are queued independently, each callback must own its event payload. `Copy` lets `emit_event` capture the same event value into every queued callback without cloning explicitly, sharing ownership, borrowing from the caller, or consuming the event for only the first handler.

The `Copy` bound also shapes event payload design. Events are expected to be small, value-like notifications: IDs, enum values, small property values, and other cheap data. The built-in derive macro enforces this model by implementing `Copy` and `Clone` for the event type.

`'static` is needed because event handlers and queued callbacks are stored inside `Context` without an external lifetime parameter. A queued callback may outlive the stack frame that emitted the event. The callback type is effectively `Box&lt;dyn FnOnce(&amp;mut Context) + 'static&gt;` because `queue_callback`, plans, and event subscriptions all accept `'static` closures. Therefore the event value captured into a queued callback cannot contain non-static references.

The derive macro implements:

```rust
impl IxaEvent for MyEvent where MyEvent: 'static {}
```

It also implements `Copy` and `Clone` manually. For generic event structs, that manual implementation avoids automatically imposing unnecessary `T: Copy` or `T: Clone` bounds when the generic parameter is only present through a marker such as `PhantomData&lt;T&gt;`.

## Why handlers have signature `Fn(&amp;mut Context, E)`

The handler signature is:

```rust
Fn(&mut Context, E)
```

The `&amp;mut Context` argument gives the handler full access to simulation state and APIs when the callback executes. This matches plans and manually queued callbacks, which also receive `&amp;mut Context`. Event handlers can inspect state, mutate state, schedule plans, queue callbacks, emit additional events, set properties, add entities, write reports, and so on.

The event payload is passed by value as `E`. Since `E: Copy`, passing by value is cheap and prevents lifetime coupling between the callback and the emitter. It also gives every subscriber its own event value, so subscribers cannot mutate a shared event object or affect what later subscribers see.

The handler is `Fn`, not `FnOnce`, because a subscription is reusable. A handler may be called for many emitted events over the lifetime of the context. `Fn` also permits handlers to be called through shared ownership inside `Rc`. If a handler needs mutable captured state, client code typically uses interior mutability such as `RefCell`, as the tests do.

The handler is not `FnMut`. Supporting `FnMut` would require mutable access to the stored closure for each invocation. That would complicate storage and dispatch, especially because each invocation is queued as an independent callback while the durable subscription remains in the handler vector. With `Fn`, the queued callback can hold an `Rc&lt;dyn Fn(...)&gt;` and invoke it through shared access.

The handler is `'static` because `Context` stores it for later use. The subscription API does not tie handler lifetime to any caller-owned stack frame.

## Callback queue relationship

Event callbacks are not the only use case for `Context::callback_queue`.

The queue is also exposed directly through:

```rust
pub fn queue_callback(&mut self, callback: impl FnOnce(&mut Context) + 'static)
```

That method pushes arbitrary callbacks onto the same `callback_queue`. Tests and plugin-context examples use `queue_callback` directly. Therefore, the callback queue is a general immediate-work queue, and event emission is one producer of that queue.

The queue type is:

```rust
callback_queue: VecDeque<Box<Callback>>
```

where:

```rust
type Callback = dyn FnOnce(&mut Context);
```

It is FIFO: callbacks are pushed with `push_back` and executed with `pop_front`.

## Execution timing

`Context::execute` repeatedly calls `execute_single_step` until shutdown is requested. `execute_single_step` prioritizes work as:

1. If `callback_queue` is nonempty, pop and execute one callback.
2. Otherwise, if `plan_queue` has a next plan, pop and execute one plan.
3. Otherwise, request shutdown so the event loop exits.

This means queued callbacks run before the next timed plan, even if that plan is scheduled for the current simulation time. It also means a plan that queues a callback will finish first; then the newly queued callback will run before the next plan is selected.

Important consequences:

- `emit_event` inside a plan does not interrupt the plan. Handlers run after the current plan returns.
- Event handlers are normal callbacks, so if a handler emits another event, those new handler callbacks are appended to the back of the callback queue.
- A callback queued before a plan at the same simulation time runs before that plan.
- A callback queued by a plan at time `t` runs at the current simulation time `t` before another plan at time `t` or later is executed.
- Callbacks do not advance simulation time. `current_time` changes when a plan is selected and executed.
- Before execution starts, `execute` initializes `current_time` to `start_time` if set, otherwise `0.0`. If only callbacks run, time remains at that initialized value.
- If `shutdown()` is called, `execute` stops before the next `execute_single_step`. Queued callbacks, including event callbacks, are left unexecuted for that execution pass.

Plans themselves are ordered by time, then `ExecutionPhase`, then scheduling order. That ordering only matters once the callback queue is empty.

## Built-in event types

### `EntityCreatedEvent&lt;E&gt;`

Definition:

```rust
pub struct EntityCreatedEvent<E: Entity> {
    pub entity_id: EntityId<E>,
}
```

It is emitted by `ContextEntitiesExt::add_entity` after:

1. The initialization property list is validated.
2. A new entity ID is allocated.
3. Initial property values are assigned to the new entity.
4. Enabled indexes are caught up for the new entity.

Only then does `add_entity` call:

```rust
self.emit_event(EntityCreatedEvent::<E>::new(new_entity_id));
```

The event identifies the newly created entity. Because the event callback is queued, subscribers run after `add_entity` returns and after the current plan or callback returns to the event loop.

Creating an entity does not emit `PropertyChangeEvent` for the initial property values. The code comments in `add_entity` explicitly state that assigning the initialization property list does not generate a property change event.

### `PropertyChangeEvent&lt;E, P&gt;`

Definition:

```rust
pub struct PropertyChangeEvent<E: Entity, P: Property<E>> {
    pub entity_id: EntityId<E>,
    pub current: P,
    pub previous: P,
}
```

It is emitted by `ContextEntitiesExt::set_property` through the partial property-change machinery.

For a non-derived property write `set_property::&lt;E, P&gt;(entity_id, property_value)`, the high-level algorithm is:

1. Before changing the stored value, decide which property-change records need to be created. This includes the written property `P` and each derived dependent of `P`, but only when that property needs partial change processing.
2. A property needs partial change processing when it has `PropertyChangeEvent&lt;E, ThatProperty&gt;` subscribers, value change counters, or an enabled index.
3. For each selected property, snapshot its previous value into a `PartialPropertyChangeEventCore&lt;E, ThatProperty&gt;`.
4. Set the new value for the written non-derived property.
5. For each partial event, compute the current value after the write, update value change counters, update any index, and emit `PropertyChangeEvent&lt;E, ThatProperty&gt;`.

This design covers both direct property changes and derived property changes. If `AgeGroup` is derived from `Age`, and `Age` is written, a subscriber to `PropertyChangeEvent&lt;Person, AgeGroup&gt;` can receive an event whose `previous` and `current` values are the derived values before and after the `Age` write.

`set_property` intentionally emits property-change events even when the written value equals the previous value, provided the partial change machinery is entered. The comments explicitly reject an early return for no-op writes, partly because clients may want to observe writes that do not change values. Value change counters are different: they update only when `current != previous`.

`PropertyChangeEvent` callbacks are queued through `emit_event`, so subscribers do not run inline during `set_property`. However, some non-event side effects in the partial property-change path happen immediately during `set_property`: current values are computed, value change counters may update, and indexes are updated before the event callbacks run.

## Performance-related fast paths and side effects

For property writes, event subscription status affects whether partial property-change objects are created. `PropertyValueStore::should_create_partial_change` returns true if any of these are true:

- There are handlers for `PropertyChangeEvent&lt;E, P&gt;`.
- The property has value change counters.
- The property has an index.

If none are true for a property and none are true for its dependents, `set_property` only updates the stored value and does not create or emit property-change events.

For entity creation, there is no analogous handler check before constructing `EntityCreatedEvent&lt;E&gt;`; `add_entity` always calls `emit_event`. If there are no handlers for that event type, `emit_event` performs the map lookup and queues nothing.

## Ordering examples

Given two handlers subscribed to `Event1`, one call to `emit_event(Event1 { ... })` queues two callbacks in subscription order.

Given:

```rust
context.emit_event(Event1 { data: 1 });
context.emit_event(Event1 { data: 2 });
```

with one subscribed handler, the callback for `data: 1` is queued before the callback for `data: 2`.

Given a plan at time `1.0` that emits an event and schedules another plan at time `1.0`, the event handler callbacks run before the second plan because callbacks have priority over plans.

Given a handler that subscribes after an event was emitted but before `execute` runs, that handler does not receive the already-emitted event because no callback was queued for it at emission time.

## Non-goals and explicit exclusions

This note does not describe the `triggers` module. Triggers may subscribe to built-in events or emit `IxaEvent`s, but their builder APIs and semantics are a separate layer and intentionally out of scope here.

This note also does not describe plans as events. Plans are scheduled callbacks in `Context::plan_queue`, not `IxaEvent` values in `Context::event_handlers`.
