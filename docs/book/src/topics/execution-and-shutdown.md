# Execution and Shutdown

This chapter details everything you might need to know about how simulations start, run, and stop.

## Syntax Summary

Running a manually constructed `Context` instance:

```rust
let mut context = Context::new();
// Initialize the model before execution.

// Start the event loop...
context.execute();
// ...or single-step by executing one callback or plan:
context.execute_single_step();
```

Shut down a running simulation from within model code:

```rust
// Trigger normal shutdown cycle and exit the event loop
context.shutdown();
// Skip normal shutdown cycle and exit the event loop immediately
context.abort();
```

## The Execution Lifecycle

A single simulation in Ixa is encapsulated by a `Context` instance. At the highest level, the lifecycle of a simulation
is:

1. **Construct a new `Context` instance.** Let's call it `context`.
2. **Initialize `context`.** Configure model behavior and initial state, load any initial entity data, register event
   subscribers, schedule initial plans, etc.
3. **Start the event loop.** This "runs" the simulation, performing work scheduled on the timeline or queued on the job
   queue, evolving the simulation state and driving the simulation forward.
4. **Exit the event loop.** Either the work queue is exhausted or client code manually triggers a stop.

### Beginning Execution

Executing a simulation, that is, starting the event loop, is as simple as calling
`context.execute()`:

```rust
let mut context = Context::new();
// Perform initialization steps here, before execution.
context.execute();
```

The only important thing to know is that `context.execute()` does not return until the event loop exits.

The preferred pattern is to use `run_with_args` (or `run_with_custom_args`) to configure and run your `Context` instance
using command-line options. In that case, you don't even have to construct a new `Context` instance or call
`context.execute()` yourself. The `run_with_args` function:

1. creates a new `Context` instance;
2. initializes the new `Context` instance according to the command line parameters;
3. calls the initialization function / closure you provided it, passing in the partially initialized `context`;
4. if your closure returns `Ok(())`, calls `context.execute()`; otherwise returns your error.

In code:

```rust
// Constructs, initializes, and (if there are no errors) executes a `Context`:
let result = run_with_args(|context: &mut Context, _args, _| {
    // Any additional custom initialization of `context` goes here.
    Ok(())
});
// If initialization failed, an error is returned.
// Otherwise, execution is run to completion.
```

See the chapter [Your First Model](../first_model/your-first-model.md) for a complete example.

### During Normal Event-loop Execution

Once `context.execute()` starts, Ixa repeatedly performs the next available unit
of work. During ordinary execution, this means:

1. If a callback is queued, run the next callback.
2. Otherwise, run the next plan from the future event list.
3. If there is no remaining active plan to keep the event loop alive, begin the
   shutdown lifecycle.

This callback-before-plan ordering is important. A callback represents work
that should happen as soon as control returns to the event loop, before the next
scheduled plan is chosen. This is also how event handlers run: when code emits
an event, Ixa queues the matching handlers as callbacks rather than running them
inline inside the code that emitted the event.

Plans are different from callbacks because they are scheduled at a particular simulation time. When the event loop
chooses a plan, Ixa advances the current simulation time to that plan's scheduled time and then runs the plan. Callbacks
do not advance simulation time; they run at whatever the current simulation time already is.

When multiple plans are scheduled for the same time, Ixa uses execution phases to order them. Plans in
`ExecutionPhase::First` run before plans in `ExecutionPhase::Normal`, and plans in `ExecutionPhase::Last` run after
normal plans. Plans with the same time and same phase run in the order they were scheduled. Model code should avoid
abusing `ExecutionPhase` to force ordering of otherwise normal simulation behavior.

> [!TIP] First and Last Execution Phases
>
> As a general rule, `ExecutionPhase::First` and `ExecutionPhase::Last` should be used for out-of-simulation
> administrative tasks, such as collecting statistics or reporting tasks. If normal in-simulation behavior needs to have a
> strict ordering of tasks scheduled at the same simulation time, the model should be structured to ensure that ordering
> in another way, for example by executing those tasks from within a single plan instead of in separate plans scheduled
> for the same time.

The event loop is not just burning through a fixed list of work created during initialization. Most models schedule some
initial plans after constructing `context` and before calling `context.execute()`, so execution has somewhere to begin. Once
the event loop is running, the queue is usually dynamic: plans and event subscribers run model code, that model code
changes simulation state, and those changes often schedule more plans or emit events. Emitted events queue callbacks for
subscribed handlers, and those callbacks can themselves schedule plans, emit more events, or queue additional callbacks.
In this way, the model continually creates the future work that drives the simulation forward.

This process is usually probabilistic. For example, a model might randomly sample how long an infection lasts and then
schedule a future plan that changes the infected person to recovered at the sampled time. In that sense, the future plan
queue is often generated probabilistically as the simulation runs. Even so, a simulation should be exactly reproducible
when run with the same random seed; see the [`random` module](random-module.md) chapter for details.

The event loop keeps repeating this process until execution starts to end. That
can happen automatically when Ixa detects that no active plans remain, or manually when model code calls
`context.shutdown()` or `context.abort()`. The following sections describe those ending behaviors in more detail.

### Ending Execution

Execution ends when the event loop enters a shutdown path. Ixa can do this
automatically when active work is exhausted, or model code can request it
explicitly.

#### Automatic Shutdown

The built-in automatic shutdown condition is active-plan exhaustion. During
normal event-loop execution, Ixa only advances simulation time while at least
one active plan remains scheduled. When there are no active plans left, Ixa
stops advancing simulation time and begins the normal shutdown lifecycle.

This means that not every scheduled plan is responsible for keeping the
simulation alive. Ixa distinguishes active plans from passive plans. Active
plans represent simulation-driving work: they evolve state, create future
simulation work, or otherwise indicate that the model still has forward
progress to make. Plans scheduled with `context.add_plan()` and
`context.add_plan_with_phase()` are active.

Passive plans are scheduled on the same future event list, use the same
simulation-time and execution-phase ordering rules, and run like other plans
while active work remains. The difference is that passive plans do not keep
execution alive by themselves. They are intended for observational or
administrative work: reporting, statistics collection, and other tasks that can
be skipped if the simulation has otherwise finished.

Periodic plans are passive. This is important because a periodic plan
reschedules itself every time it runs. If periodic plans were active, a periodic
reporter or statistics collector could keep the simulation alive forever even
after all simulation-driving work had finished. Because periodic plans are
passive, they can run while active work is keeping the timeline alive without
preventing automatic shutdown.

Passive plans can remain queued after automatic shutdown begins. A passive plan
scheduled at the final current simulation time can still run as part of the
normal shutdown lifecycle, but a passive plan scheduled for a later time will
not cause Ixa to advance time just to run it. That future passive plan remains
queued and may run later if model code schedules new active work and calls
`context.execute()` again.

#### Normal Shutdown

Normal shutdown is the orderly way for execution to end. It happens automatically when active plans are exhausted, and
it can also be requested manually by calling `context.shutdown()` from model code.

Calling `context.shutdown()` does not stop execution at that exact line of code. Instead, it tells the event loop to
finish the normal shutdown cycle and then return from `context.execute()`. The key rule is that normal shutdown stops
simulation time from advancing. Ixa may still run work that is already due at the current simulation time, but it will
not move forward to a later simulation time during that execution pass.

##### What Runs During Normal Shutdown

Normal shutdown preserves the usual callback-before-plan rule. If callbacks are already queued, or if shutdown work
queues new callbacks, those callbacks run before the next plan is selected.

After callbacks are drained, Ixa runs regular plans scheduled exactly at `context.get_current_time()`. These are plans
that are due at the current simulation time, not future plans. Once no more current-time regular plans are available,
Ixa runs shutdown-time plans. After shutdown-time plans and any callbacks they queue are exhausted, `context.execute()`
returns.

The overall order is:

1. queued callbacks;
2. regular plans at the current simulation time;
3. shutdown-time plans, with callbacks still drained before the next shutdown-time plan;
4. return from `context.execute()`.

##### Current-Time Plans Across Phases

During normal shutdown, Ixa drains all regular plans scheduled at the current simulation time, across execution phases.
This matters when `context.shutdown()` is called by one plan while other plans are scheduled for the same time.

For example, suppose a normal-phase plan at time `10.0` calls `context.shutdown()`. Ixa will not advance to time `11.0`,
but it will still run the remaining plans scheduled for time `10.0`, including plans in `ExecutionPhase::Last`. The
callback queue is drained between execution of plans as usual.

##### Shutdown-Time Plans

Shutdown-time plans are plans that are meant to run at the end of execution rather than at a specific simulation time.
They are scheduled with `context.add_shutdown_plan()` or `context.add_shutdown_plan_with_phase()`.

Shutdown-time plans run after current-time regular plans have been exhausted. They do not advance simulation
time; during a shutdown-time plan, `context.get_current_time()` still returns the last regular simulation time.

Ixa orders shutdown-time plans the same way it orders regular plans at a shared simulation time: by execution phase,
then by scheduling order.

Once Ixa has begun running shutdown-time plans, it does not return to the regular plan queue during that same execution
pass. If a shutdown-time plan schedules a regular plan, even at the current simulation time, that regular plan remains
unexecuted in the queue until `context.execute()` is called again.

Shutdown-time plans are useful for finalization work that should happen after ordinary current-time simulation work has
finished but before `context.execute()` returns. Common use cases include:

- Writing a final "totals" row to a summary report, such as total infections, peak prevalence, or final population size.
- Emitting a final partial-period report when shutdown happens between regular periodic report times. If shutdown occurs
  partway through a reporting period, the next periodic report time will never come. A shutdown-time plan can write this
  final partial-period report.
- Flushing model-owned buffers or cached output that has not yet been written through Ixa's
  [reporting system](reports.md).
- Running final consistency checks, such as verifying conservation relationships or checking that no model-specific
  invariants were violated.
- Recording model-specific shutdown metadata, such as whether the run ended by max time, extinction, or another
  model-defined condition.
- Finalizing accumulators whose meaning depends on the full run, such as time-at-risk totals, person-time denominators,
  or cumulative exposure measures.

##### What Remains Queued After Shutdown

Normal shutdown does not clear plans scheduled in the future, that is, plans scheduled after the current simulation
time. Also, if during shutdown-time execution a plan is scheduled for the current simulation time, that plan will remain
queued without being executed as well, because once shutdown-time plans start executing, the event loop never returns
to the regular plan queue before exiting.

This is intentional. A `Context` can be executed again after `context.execute()` returns. If model code later schedules
new active work and calls `context.execute()` again, queued future work can still run according to the usual event-loop
rules. However, this would be unusual. The more typical case is for `context.execute()` to only ever be called once.

#### Aborting Execution

Calling `context.abort()` stops the current `context.execute()` event loop immediately. Unlike normal shutdown, aborting
does not drain queued callbacks, does not run remaining plans at the current simulation time, and does not run
shutdown-time plans. It is the escape hatch for cases where the model should stop now rather than complete the normal
shutdown cycle.

An abort only affects the current execution pass. It does not clear the future event list or permanently poison the
`Context`. If queued work remains, a later call to `context.execute()` can continue from that state. In typical model
code, however, `abort()` should be reserved for exceptional cases where skipping normal shutdown work is intentional.
If `context.shutdown()` is normal shutdown, then `context.abort()` is abnormal shutdown.

#### Common Shutdown Patterns

Even though Ixa automatically begins shutdown when no active plans remain, most models should still define an explicit
stop condition. An explicit stop condition makes the intended run horizon clear and protects the model from running
longer than intended if some part of the simulation keeps producing active work.

A recommended best practice is to define a fixed end time scheduled during initialization:

```rust
const MAX_TIME: f64 = 365.0;

context.add_passive_plan(MAX_TIME, |context| {
    context.shutdown();
});
```

This does not prevent the model from ending earlier by active-plan exhaustion. It simply guarantees that, if the
simulation is still running at `MAX_TIME`, normal shutdown begins then. The end time should usually be far enough in the
future to cover the intended simulation horizon.

Models can also define their own shutdown triggers in ordinary model code. For example, a model might shut down once a
target condition has been reached:

```rust
fn maybe_stop(context: &mut Context) {
    if no_infections_remain(context) {
        context.shutdown();
    }
}
```

That trigger can be called from whichever callbacks or plans observe a relevant state change. Once `context.shutdown()`
is called, Ixa observes the normal shutdown cycle, exiting the event loop before simulation time can progress.

## Execution Outside of the Event-loop

Most models call `context.execute()` once and let the event loop run to completion. Ixa also exposes the lower-level
`context.execute_single_step()` method for less common cases where model code needs to step through work manually.
Never call these methods from in-simulation code, that is, code that executes from within an active event loop. They
should only be called outside of a running event loop.

### Executing Again After `execute()` Returns

A `Context` still exists after `context.execute()` returns. Its model state remains in place, its current simulation
time remains where execution stopped, and any queued work that was not run during the previous execution pass remains
queued.

That means it is possible to call `context.execute()` again. This is not the typical way to structure a model run, but
it can be useful for interactive workflows, tests, debugging, or specialized control code that intentionally runs a
simulation in stages.

The main thing to remember is that a later execution pass continues from the current state. It does not reset the
`Context`, clear queued work, or move simulation time backward. For example, if normal shutdown stops at time `10.0`
and leaves an active plan queued for time `12.0`, a later call to `context.execute()` can continue to that future plan.
Similarly, if `context.abort()` stops execution while callbacks or plans remain queued, a later call to
`context.execute()` can continue with that queued work.

Passive plans require special care. A future passive plan can remain queued after execution stops, but a passive plan
does not keep the event loop alive. If only passive future work remains, calling `context.execute()` again will not make
Ixa advance time just to run it. To reach that passive work in a later execution pass, the model must also schedule
active work that keeps the timeline alive until the relevant time.

[Negative-time burn-in](burn-in-and-negative-time.md) and restarting the event loop are not interchangeable. Burn-in
runs as a single, uninterrupted `context.execute()` call that simply starts before `0.0` and crosses into the analysis
window without resetting state. This is what you want whenever you need an initialization period for things like
population structure, immunity, or infection states to stabilize before measurement begins. Calling `context.execute()`
a second time is a separate pattern that only makes sense when the event loop must stop and hand control back to your
driver code, typically for tests, debugging, interactive workflows, or staged runs that inspect or modify the `Context`
between passes. In short: reach for burn-in for initialization, and use repeated `execute()` calls only when you
specifically need out-of-simulation low-level control.

### Single-Step Execution

The `context.execute_single_step()` method exposes one iteration of the event-loop state machine. One call runs at most
one queued callback, one plan, or one shutdown status transition. This is the primitive that `context.execute()` uses
internally: `execute()` repeatedly calls `execute_single_step()` until the event loop reaches a stopped state.

Single stepping is most useful for tests, debugging, visualization, and interactive control code. It lets external code
observe the `Context` between units of event-loop work instead of waiting for the whole simulation to finish.

The same ordering rules still apply. If a callback is queued, a single step runs that callback before selecting a plan.
If no callback is queued, a single step may run the next regular plan, move from active-plan exhaustion into normal
shutdown, run a current-time shutdown plan, move into shutdown-time execution, run a shutdown-time plan, or complete the
stopped-state transition.

The main subtlety is that not every single step runs user model code. Some calls only advance the execution state
machine. For example, if there is no active work left but a shutdown-time plan is queued, one call may enter normal
shutdown, another may move from current-time regular plans to shutdown-time plans, and a later call may actually run the
shutdown-time plan. This is expected: single stepping exposes the intermediate lifecycle states that `context.execute()`
usually hides by looping until execution is complete.
