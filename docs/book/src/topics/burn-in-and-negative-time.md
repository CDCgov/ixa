# Burn-in Periods and Negative Time

## **Syntax and Summary**

To implement burn-in using negative time:

1. Choose a burn-in duration `d`.
2. Call `context.set_start_time(-d)` before execution.
3. Schedule burn-in logic at negative times.
4. Schedule transition logic at `0.0` if needed.
5. Call `context.execute()` once.

Minimal example:

```rust
use ixa::prelude::*;

let mut context = Context::new();

// 1. Extend the timeline backward.
context.set_start_time(-180.0);

// 2. Burn-in logic.
context.add_plan(-180.0, |ctx| {
    // initialization or modified dynamics
});

// 3. Activate full dynamics at the analysis boundary.
context.add_plan_with_phase(
    0.0, 
    |ctx| {
    	  // Enable full dynamics, reporting, interventions, etc.
    },
  	// Optional: Run this plan before other plans scheduled at time 0.0
  	ExecutionPhase::First
);

context.execute();
```

## Introduction

In many epidemiological and agent-based models, the state you care about at time `0.0` does not arise instantaneously.

Populations need time to stabilize. Households and partnerships need to form. Immunity must reflect prior exposure rather than arbitrary assignment. Latent infections may need to be seeded and allowed to evolve into a realistic distribution. In short, the model often needs to *run* before it *begins*.

A common but fragile solution is to run a separate “initialization” simulation, snapshot its state, and then start a second simulation for analysis. This approach complicates reproducibility and splits what is conceptually one model into multiple executions.

Ixa provides a simpler mechanism: treat burn-in as part of the same execution. Instead of resetting time or stitching simulations together, you allow the timeline to extend into negative values. You then designate `0.0` as the beginning of your analysis window.

Negative time is not a special execution mode in Ixa. It is simply earlier simulation time. The event queue, scheduling rules, and execution semantics are identical before and after `0.0`. What may differ is your model logic. You may choose to disable transmission, suppress reporting, use alternate parameters, or run simplified dynamics during burn-in. These differences arise from your code, not from special treatment by the framework.

## The Core Pattern

Burn-in in Ixa is implemented by extending the simulation timeline into negative values and treating `0.0` as the beginning of the analysis window. There is only one execution and one event queue; burn-in and the main simulation evolve on the same continuous timeline.

The core pattern is:

1. Choose a burn-in duration (for example, 180 days).
2. Set the simulation start time to the negative of that duration.
3. Schedule burn-in and main-simulation logic on the same timeline.

The following example burns in for 180 days and enables full model dynamics at time `0.0`:

```rust
use ixa::prelude::*;

let mut context = Context::new();

// Burn in for 180 days before the "official" start.
context.set_start_time(-180.0);

// Burn-in setup and updates can run at negative times.
context.add_plan(-180.0, |ctx| {
    // Initialize or seed state here.
});

// Transition point: begin simulation proper at time 0.
context.add_plan_with_phase(
    0.0, 
    |ctx| {
    	  // Enable full dynamics, reporting, interventions, etc.
    },
  	// Run this plan before other plans scheduled at time 0.0
  	ExecutionPhase::First
);

context.execute();
```

The simulation does not restart at `0.0`. Time flows continuously from negative values through zero and onward. What changes at `0.0` is not how Ixa executes the model, but what behavior your model chooses to enable.

## Interpreting Time in Model Code

The `context.get_current_time()` method always returns the current simulation time:

- Negative during burn-in
- `0.0` at the beginning of the analysis window
- Positive thereafter

No special API is required to detect burn-in. Time itself provides the phase boundary. Model behavior can be partitioned directly by time:

```rust
fn transmission_step(context: &mut Context) {
    let t = context.get_current_time();

    if t < 0.0 {
        // Burn-in behavior (for example, transmission disabled)
        return;
    }

    // Main simulation behavior
}
```

This pattern makes the transition at `0.0` explicit and local to the model logic. Ixa does not switch modes automatically at `0.0`; any change in behavior must be encoded in your model.

In practice, models use one of two approaches:

- **Time-gated logic**, as shown above, where behavior depends directly on the current time.
- **Activation at** **`0.0`**, where a plan scheduled at time `0.0` enables or modifies model state (for example, by turning on transmission, enabling interventions, or beginning data collection).

Both approaches operate on the same continuous timeline. The choice depends on whether you prefer explicit time checks throughout the model or a single transition event that modifies state at the boundary.

## **Common Burn-in Designs**

Burn-in is not limited to simple state initialization. In practice, it is used to execute a specialized variant of the model prior to the analysis window. Several patterns recur frequently.

### Reduced or Modified Dynamics

During burn-in, some components of the model may operate differently:

- Transmission disabled while demography and recovery remain active.
- Alternate parameter values used to drive the system toward a desired state.
- Simplified dynamics used to establish equilibrium.

These differences are implemented through time-gated logic or by enabling full dynamics at `0.0`.

### Network or Structure Formation

In models with dynamic networks—households, partnerships, contact graphs—it is often desirable to allow the structure to stabilize before transmission begins. Burn-in provides a period during which relationships can form, dissolve, and equilibrate before infections are introduced or measurement begins.

### Seeding and Equilibrium Targeting

Rather than assigning initial states arbitrarily, models may:

- Introduce infections gradually.
- Allow immunity to accumulate through prior exposure.
- Run until summary metrics stabilize before beginning analysis.

Negative time allows this process to occur within the same execution, without splitting the model into separate runs.

### Delayed Reporting or Intervention

Often, the model’s full dynamics operate throughout burn-in, but outputs or interventions are suppressed until `t >= 0.0`. In this case, burn-in shapes the system state, while the analysis window determines what is recorded or evaluated.

Burn-in is therefore not a distinct modeling technique. It is a scheduling strategy that allows different behaviors to operate before and after a chosen time boundary on a single continuous timeline.

## Practical Considerations

Burn-in relies on the same scheduling rules as the rest of the simulation. The following constraints are important when working with negative time.

### Set the Start Time Before Execution

`context.set_start_time(...)` must be called before `context.execute()` and may only be called once. If you intend to use negative time, set the start time first, then schedule burn-in plans.

### Plans Cannot Be Scheduled Earlier Than the Effective Start Time

A plan cannot be scheduled earlier than the simulation’s effective current time. Before execution begins:

- If no start time is set, the earliest allowable plan time is `0.0`.
- If `start_time = s`, the earliest allowable plan time is `s`.

For burn-in, this means you must set a negative start time before scheduling any negative-time plans.

### Periodic Plans Begin at 0.0

`add_periodic_plan_with_phase(...)` schedules its first execution at `0.0`, not at the simulation start time. This is often desirable: reporting or intervention logic naturally begins at the analysis boundary. However, periodic behavior will not automatically run during negative-time burn-in.

If periodic activity is required during burn-in, schedule the first execution manually at the desired negative time and reschedule from there.

### Reports and Outputs Include Negative Timestamps

Outputs generated during burn-in will carry negative timestamps. This is usually intentional. If downstream analysis should begin at `0.0`, filter during post-processing or guard reporting logic within the model.

### `0.0` Convention, Not a Reset

Time does not reset at `0.0`. The event queue continues uninterrupted across the boundary. Any transition in behavior at `0.0` must be implemented explicitly in model logic or in a plan scheduled at that time.
