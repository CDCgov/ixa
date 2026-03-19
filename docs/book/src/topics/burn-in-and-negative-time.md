# Burn-in Periods and Negative Time

## Syntax Summary

Burn-in is implemented by setting a negative start time and treating `0.0` as the beginning of the analysis window:

```rust
context.set_start_time(-d);
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
3. Differentiate burn-in behavior from main-simulation behavior using one of the two methods below.

There are two standard approaches:

- **Time-gated logic**: behavior depends directly on the current time with `context.get_current_time() >= 0.0` checks throughout the code.
- **Activation at `0.0`**: a plan scheduled at time `0.0` enables or modifies model state (for example, by turning on transmission, enabling interventions, or beginning data collection).

Both approaches operate on the same continuous timeline. The choice depends on whether you prefer localized time checks or a single activation event at `0.0`. There is no automatic transition at `0.0`. Any change in behavior must be implemented explicitly in your model.

### 1. Activation at `0.0`

Schedule a plan at `0.0` that enables or modifies model behavior. For example, transmission, reporting, or interventions may be turned on at the boundary. This approach centralizes the transition logic in a single plan.

The following example burns in for 180 days and enables full model dynamics at time `0.0`:

```rust
use ixa::prelude::*;

let mut context = Context::new();

// Burn in for 180 days before the "official" start.
context.set_start_time(-180.0);

// Optional: perform initialization at the start of burn-in.
context.add_plan(-180.0, |ctx| {
    // Initialize or seed state here.
});

// Method 1: Activation at 0.0
context.add_plan_with_phase(
    0.0, 
    |ctx| {
        // Enable full dynamics, reporting, interventions, etc.
    },
    ExecutionPhase::First
);

context.execute();
```

In this example we use `context.add_plan_with_phase` with `ExecutionPhase::First` instead of the usual `context.add_plan` so that the activation plan runs before any other plans that happen to be scheduled at time `0.0`.

### 2. Time-Gated Logic in Model Code

Partition behavior directly by checking the current time:

```rust
fn transmission_step(context: &mut Context) {
    if t < context.get_current_time() {
        // Burn-in behavior
        return;
    }
    // Main simulation behavior
}
```

This approach makes the phase boundary explicit in the code where behavior occurs. The downside is that you might need to do this check in many different disparate places within model code.

## Practical Considerations

Burn-in relies on the same scheduling rules as the rest of the simulation. The following constraints are important when working with negative time.

### Set the Start Time Before Execution

`context.set_start_time(...)` must be called before `context.execute()` and may only be called once.

The start time may be set to an arbitrarily low number. When execution begins, the event queue advances directly to the earliest scheduled plan. If burn-in plans are scheduled stochastically, it may be useful to choose a sufficiently low start time such that the probability of scheduling a plan earlier than the start time is effectively zero.

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

### Include Negative Time in Initialization Tests

If negative time is part of your model initialization strategy, unit tests that validate initialization behavior may also need to include negative-time execution. Tests that assume the model begins at `0.0` may otherwise miss burn-in effects.

### `0.0` Convention, Not a Reset

Time does not reset at `0.0`. The event queue continues uninterrupted across the boundary. Any transition in behavior at `0.0` must be implemented explicitly in model logic or in a plan scheduled at that time.

## Common Burn-in Designs

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
