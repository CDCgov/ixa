# ADR-0008: Define shutdown and shutdown-time plan semantics

| Field | Value |
| --- | --- |
| Decision date | 2026-07-07 (merge of PR #976) |
| Recorded | 2026-07-29 |
| Status | Accepted |
| GitHub issue | [#926](https://github.com/CDCgov/ixa/issues/926) (inferred from the feature-branch name) |
| Pull request | [#976](https://github.com/CDCgov/ixa/pull/976) |
| Feature branch | `RobertJacobsonCDC_926_shutdown_semantics` |

## Summary

Ixa changed `Context::shutdown()` from an immediate event-loop stop into a
normal shutdown request. Normal shutdown stopped simulation time from
advancing, drained callbacks and regular plans at the current simulation time,
then ran a distinct queue of shutdown-time plans before returning. The former
immediate-stop behavior remained available as `Context::abort()`.

The event loop and public single-step operation were defined by the same
private shutdown state machine. Regular and shutdown-time plans were held by
one `PlanQueue`, with shared plan identifiers and cancellation, but
shutdown-time plans had no numeric simulation time and formed a final,
one-way phase of an execution pass.

## Context

Before this decision, `Context` represented shutdown with one
`shutdown_requested: bool`. `Context::shutdown()` set the flag, and
`Context::execute()` checked it before each call to `execute_single_step()`.
Once observed, the loop cleared the flag and returned without processing
another callback or plan. Natural exhaustion used the same flag to end the
loop. There was no separate cleanup phase and no API distinction between a
normal shutdown and an immediate abort.

That behavior could leave callbacks queued by the plan that requested shutdown
and could skip other regular plans scheduled for the same simulation time.
Ixa also needed a way to register ordered work that would run after the regular
timeline completed, whether completion was natural or explicitly requested.

Normal shutdown could not simply pop the next regular plan and decide
afterward whether to run it. A future plan had to remain scheduled so a later
call to `execute()` could resume the timeline. The existing generic `Queue`
offered destructive retrieval, while its `peek()` searched by iterating a
`BinaryHeap`; heap iteration did not guarantee that the first live entry found
was the earliest plan.

The design also had to preserve existing scheduling properties:

- queued callbacks ran before the next plan;
- plans at the same time were ordered by `ExecutionPhase` and insertion order;
- one `PlanId` and one cancellation API covered scheduled work; and
- `execute_single_step()` remained usable by manual stepping and visualization
  drivers.

## Decision

### Separate normal shutdown from immediate abort

`Context::shutdown()` requested normal shutdown. Once requested, the current
execution pass:

1. continued to drain callbacks;
2. ran every regular plan scheduled exactly at `Context::current_time`,
   including all execution phases;
3. transitioned to shutdown-time plans without advancing simulation time; and
4. returned after callbacks and shutdown-time plans were exhausted.

Future regular plans were not removed. A later `execute()` call could continue
with them.

`Context::abort()` provided the former immediate-stop behavior. An abort
requested while `execute()` was running stopped that event loop before another
callback or plan was selected. The stopped state was then cleared, so queued
work could continue in a later execution pass. An abort set before entering
`execute()` likewise did not poison that later pass; `execute()` cleared the
pre-existing stopped state on entry.

Calling `shutdown()` after shutdown was already in progress did not change the
current phase. Calling it during shutdown-time execution therefore could not
return the event loop to regular current-time plans. `abort()` could stop any
phase immediately.

### Add a final shutdown-time plan phase

Ixa added `Context::add_shutdown_plan()` and
`Context::add_shutdown_plan_with_phase()`. Shutdown-time plans:

- had no numeric simulation time;
- were ordered by `ExecutionPhase` and insertion order;
- ran after regular current-time work during normal shutdown;
- also ran after natural exhaustion of the regular timeline; and
- could be canceled through the existing `Context::cancel_plan()` API.

Callbacks retained priority over plan selection. A callback queued by a
shutdown-time plan ran before the next shutdown-time plan.

Transition into shutdown-time execution was one-way for that execution pass.
If a shutdown-time plan scheduled a regular plan, even at the current
simulation time, that plan stayed on the regular timeline for a later
`execute()` call. Shutdown-time work was therefore a final ordered phase, not
another source interleaved with regular scheduling.

### Use one plan-queue owner and one shutdown state machine

The generic `Queue` implementation in `plan.rs` became an internal
`PlanQueue` specialized for context callbacks. It owned separate regular and
shutdown-time heaps together with one callback map and one `PlanId` allocator.
This kept identifiers globally unique across both kinds of plan and made the
existing cancellation operation unambiguous. The public
`ixa::plan::PlanId` path was preserved through a compatibility re-export.

The queue added non-destructive regular-plan inspection and separate retrieval
operations for:

- the next regular plan;
- a regular plan only when its time exactly equaled the current time; and
- the next shutdown-time plan.

Canceled heap entries were removed from the heap root before inspection rather
than found through unordered heap iteration.

A private `ShutdownStatus` state machine selected among normal execution,
current-time draining, shutdown-time draining, and stopping.
`execute_single_step()` advanced that state machine by at most one callback,
one plan, or one status transition. `execute()` became a loop around the
single-step primitive and stopped when the state machine reached its stopped
state.

## Rationale

Distinguishing normal shutdown from abort allowed cleanup and same-time work to
complete without removing the ability to stop a faulty or unwanted execution
pass promptly. Defining normal shutdown in terms of the current simulation time
avoided advancing the model merely to shut it down and preserved future work
for an intentional later resume.

A distinguished shutdown-time queue expressed lifecycle work directly.
Encoding that work as a special numeric time would have exposed a sentinel
through the ordinary scheduling API and given shutdown work misleading
simulation-time semantics.

Making shutdown-time execution a final, one-way phase produced a stable
ordering boundary. Otherwise, shutdown-time plans that scheduled current-time
regular work could repeatedly move execution between the two queues and make
the meaning of "shutdown-time" dependent on callback behavior.

One queue owner avoided collisions that two independent queues could create by
both allocating the same `PlanId`. It also kept scheduling, cancellation, lazy
removal of canceled entries, and profiling in one abstraction.

Finally, placing scheduler selection in `execute_single_step()` kept manual
stepping consistent with the full event loop. A caller stepping the simulation
could observe the same transition from regular exhaustion to shutdown-time
work rather than bypassing lifecycle semantics implemented only in
`execute()`.

## Consequences

- `shutdown()` became graceful rather than immediate. Existing callers that
  required the old stop-now behavior had to use `abort()`.
- Callbacks and all regular plans at the shutdown request's simulation time
  completed before shutdown-time work.
- Normal shutdown did not advance time or discard future regular plans, so a
  later `execute()` could resume them.
- Shutdown-time plans gained phase ordering, FIFO ordering within a phase,
  ordinary cancellation, and callback-before-plan behavior.
- Shutdown-time work observed the last regular simulation time; it did not have
  a distinct time value of its own.
- Regular plans created during shutdown-time execution were deliberately
  deferred, even when scheduled for the current time.
- The scheduler gained a private multi-state lifecycle and two internal heaps,
  increasing implementation complexity in exchange for explicit ordering and
  resumability.
- `execute_single_step()` could spend a call performing only a state
  transition, such as entering shutdown-time execution after regular
  exhaustion.
- The internal plan module was renamed and specialized, while a compatibility
  module preserved the existing public `PlanId` import path.

The later
[passive-plan decision](0009-distinguish-passive-plans.md) refined when regular
execution was considered exhausted. It did not remove the distinction among
normal shutdown, abort, and shutdown-time plans established here.

## Alternatives considered

### Keep `shutdown()` as the immediate-stop operation

This would have preserved existing behavior but would still have left no
normal-shutdown lifecycle in which callbacks, same-time work, and cleanup plans
could finish. The immediate behavior was retained under the more explicit
`abort()` name instead.

### Represent shutdown-time work with a special simulation time

A sentinel could have reused the regular plan heap. It was not selected because
shutdown-time work had ordering but no meaningful numeric time. Exposing a
sentinel through `add_plan()` would also have complicated time validation and
the guarantee that normal shutdown did not advance the simulation clock.

### Let shutdown-time plans re-enter regular current-time execution

The event loop could have returned to the regular heap whenever shutdown-time
work scheduled a plan at the current time. This was rejected because it blurred
the final shutdown phase and could create repeated alternation between regular
and shutdown-time work. Such regular plans were instead preserved for the next
execution pass.

### Give regular and shutdown-time queues independent ownership

Two queue instances would have been mechanically simpler, but their independent
identifier allocators could issue the same `PlanId`, making cancellation
ambiguous or requiring queue-kind information in the public identifier. One
`PlanQueue` therefore owned both heaps and the shared identifier namespace.

### Inspect live plans by iterating the existing heap

The existing `Queue::peek()` skipped canceled entries by scanning
`BinaryHeap` iteration order. That order was not sorted, so it could not
reliably decide whether the earliest live plan was at the current time.
Root-based cleanup and non-destructive root inspection were adopted instead.

## References

- [Issue #926](https://github.com/CDCgov/ixa/issues/926) (association
  inferred from the feature-branch name)
- [PR #976: Shutdown semantics and shutdown-time plans](https://github.com/CDCgov/ixa/pull/976)
- [Feature-branch commit `9305174`](https://github.com/CDCgov/ixa/commit/93051742ac47963c2bac7116b401a7a4ea3338c1)
- [Adopted commit `380103a`](https://github.com/CDCgov/ixa/commit/380103a89f4178750e2e1d2d8e79c6418ce5fe7b)

The retained feature-branch commit and the merged commit have identical trees.
The implementation plan in `Notes/plan-shutdown-semantics.md`, the adopted
state machine and queue implementation, and their focused unit tests supplied
the reconstruction evidence for this record.
