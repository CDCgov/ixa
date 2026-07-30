# ADR-0009: Distinguish passive plans from liveness-sustaining plans

| Field | Value |
| --- | --- |
| Decision date | 2026-07-10 (merge of PR #981) |
| Recorded | 2026-07-29 |
| Status | Accepted |
| Related decision | [ADR-0008: Define shutdown and shutdown-time plan semantics](0008-define-shutdown-semantics.md) |
| GitHub issue | [#916](https://github.com/CDCgov/ixa/issues/916) (inferred from the feature-branch name) |
| Pull request | [#981](https://github.com/CDCgov/ixa/pull/981) |
| Feature branch | `RobertJacobsonCDC_916_passive_plans` |

## Summary

Ixa distinguished passive regular plans from the existing, liveness-sustaining
regular plans. Passive plans used the same simulation-time, execution-phase,
plan-ID, cancellation, and callback semantics as other regular plans, but they
did not by themselves keep the normal simulation timeline running. Exhaustion
therefore came to mean that no non-passive regular plans remained, rather than
that the regular plan queue was empty.

The existing one-time scheduling APIs remained non-passive, and new one-time
passive APIs were added. Periodic plans and library-owned, self-rescheduling
report handlers became passive because their observational work should not
prevent a simulation from completing.

## Context

[ADR-0008](0008-define-shutdown-semantics.md) had established a graceful
shutdown lifecycle. When the regular plan queue was exhausted, `Context`
stopped advancing simulation time, drained regular work at the current time,
and then ran shutdown-time plans. Future regular work was preserved when
normal shutdown had been requested explicitly.

Queue exhaustion was not a useful automatic stop condition once a model used
self-rescheduling observational work. Periodic reporting, statistics
collection, and periodic property-value change counts each scheduled a later
occurrence after running. Such work could leave the regular queue perpetually
nonempty even after all simulation-driving activity had finished.

The periodic-plan implementation tried to stop rescheduling when the queue was
otherwise empty. That local queue-length test did not express model liveness:
multiple periodic plans could keep one another scheduled, and each would
mistake the other's observational callback for simulation-driving work. The
periodic value-change-count handler used a similar remaining-plan-count check.
Consequently, adding reporters could change whether an otherwise finite model
terminated.

The design needed to preserve the ordering and final-time guarantees introduced
by ADR-0008. In particular, observational work scheduled before or at the last
simulation-driving time still needed to run in normal order, including across
execution phases, without allowing later observations to advance the clock.
Immediate abort and the distinguished shutdown-time queue also had to remain
unaffected.

## Decision

### Make passivity a liveness attribute of regular plans

Every live regular plan was classified internally as passive or non-passive.
The existing `Context::add_plan()` and `Context::add_plan_with_phase()` APIs
continued to create non-passive plans. Ixa added:

- `Context::add_passive_plan()`;
- `Context::add_passive_plan_with_phase()`; and
- matching `ContextBase` methods for plugin extension traits.

Passivity affected only whether a plan kept the normal timeline alive. It did
not restrict what the callback could do: passive callbacks still received
`&amp;mut Context` and could mutate state or schedule non-passive work. Treating
passive plans as observational and safe to skip after the final time was an API
convention and a responsibility of the caller, not an enforced effect system.

Passive and non-passive regular plans shared one heap and retained the existing
ordering by simulation time, `ExecutionPhase`, and plan ID. The passive flag
did not participate in ordering.

### Base normal-timeline liveness on non-passive plans

`PlanQueue` tracked the number of live non-passive regular plans. Adding,
canceling, popping, or clearing plans updated that count while retaining lazy
removal of canceled heap entries. Passive regular plans and shutdown-time plans
did not contribute to the count.

During ordinary execution, the queue returned its earliest regular plan only
while at least one non-passive regular plan remained somewhere in the queue.
The returned plan could itself be passive. This allowed passive work scheduled
before a later non-passive plan to execute in normal time and phase order.

Once no non-passive regular plan remained, `Context::execute_single_step()`
entered normal shutdown rather than jumping directly to shutdown-time plans.
The shutdown lifecycle from ADR-0008 then determined which remaining passive
plans could run:

- passive plans earlier than the final non-passive time had already run in
  ordinary order;
- passive plans at the final time ran while normal shutdown drained all regular
  plans at `Context::current_time`, regardless of execution phase; and
- passive plans after the final time remained queued because shutdown did not
  advance simulation time.

A later call to `execute()` could run those retained passive plans if new
non-passive work made the regular timeline live again. If a context started
with only passive plans, non-passive exhaustion was immediate; normal shutdown
still allowed passive plans at the initial current time to run.

Shutdown-time plans remained outside this distinction and never sustained the
normal timeline. `Context::abort()` remained immediate and did not drain
passive or other pending work after an abort request.

### Make indefinitely self-rescheduling library work passive

`Context::add_periodic_plan_with_phase()` became a passive-only periodic API.
Both its initial callback and every later occurrence were scheduled passively.
After running, a periodic callback scheduled its next occurrence
unconditionally; normal-timeline liveness, rather than a local queue-length
test, determined whether execution would reach that occurrence.

The self-rescheduling handlers used by periodic property-value change counts
were converted in the same way. Their one-time setup plan remained
non-passive, while the first and subsequent report handlers were passive.

The internal `remaining_plan_count` APIs were removed because queue length no
longer represented the condition for continued execution.

## Rationale

Liveness was a property of the role a plan played, not merely of whether any
callback remained scheduled. Marking observational work passive made
termination independent of the number of reporters and allowed perpetually
self-rescheduling services to coexist with a finite simulation.

Keeping both plan kinds in the regular queue preserved all existing temporal
ordering. A separate passive queue, or selecting non-passive plans ahead of
earlier passive plans, would have made reporting order depend on classification
rather than simulation time and execution phase.

Entering the normal shutdown state on non-passive exhaustion reused the
final-time boundary established by ADR-0008. It allowed passive plans at the
last simulation time to finish without advancing to later observations and
without weakening the one-way transition to shutdown-time work.

Making periodic plans passive by definition reflected their API contract.
They rescheduled indefinitely and had no public operation for disabling the
periodic behavior itself, so treating them as liveness-sustaining would still
make automatic exhaustion unavailable to models that used them.

The merged design expressed the distinction as passive versus ordinary plans,
not as two equally named public categories of “active” and “passive” plans.
The feature branch explicitly removed active-plan terminology for ordinary
plans before merge, keeping the existing scheduling API and vocabulary as the
default.

## Consequences

- Models could use periodic reporters and other observational loops without
  preventing automatic shutdown after simulation-driving work ended.
- Existing one-time plan calls retained their liveness-sustaining behavior;
  callers opted into passivity through new APIs.
- Periodic callbacks at or before the final non-passive time could run, while
  later occurrences remained queued and did not advance time.
- Multiple passive periodic callbacks no longer kept one another alive.
- Passive plans preserved ordinary plan ordering and cancellation semantics,
  avoiding a second scheduling domain.
- The scheduler acquired per-plan passivity metadata and an exact count of live
  non-passive regular plans. Every add, cancel, pop, and clear path had to
  preserve that accounting invariant.
- A passive callback was not prevented from mutating model state or creating
  non-passive work. Incorrect classification could therefore cause
  simulation-relevant work to be skipped when execution stopped before its
  scheduled time.
- Unexecuted passive plans could remain queued after `execute()` returned and
  become eligible during a later execution pass once non-passive work was
  added.
- Queue-length liveness helpers were removed, so library code could no longer
  infer continued execution from the mere presence of scheduled plans.

## Alternatives considered

### Continue treating an empty regular queue as exhaustion

This preserved the shutdown model from ADR-0008 but left automatic termination
unreliable in the presence of self-rescheduling observers. Local checks that
rescheduled a periodic callback only while some other plan remained did not
solve the problem because multiple observers could keep one another alive.

### Keep periodic plans liveness-sustaining

Periodic plans could have remained ordinary plans and required models to call
`shutdown()` explicitly. This was not selected because periodic work had no
built-in termination or disabling mechanism and was commonly used for
reporting. Its presence would therefore defeat plan-exhaustion shutdown in the
models that most needed recurring observations.

### Stop passive callbacks from rescheduling when other work disappears

The earlier implementation approximated this behavior with queue-length
checks. Those checks coupled each callback to incidental queue contents and
could not distinguish another observer from simulation-driving work. Passive
callbacks instead rescheduled unconditionally, while one centralized
non-passive count controlled event-loop liveness.

### Enforce that passive plans are observational

Ixa could have restricted passive callbacks to read-only access or prohibited
them from scheduling non-passive work. That would have required a different
callback and context capability model. The adopted API retained `&amp;mut Context`
and treated passivity as a shutdown classification only, accepting the risk of
caller misclassification.

### Call ordinary plans “active plans”

The initial feature implementation used active/passive terminology and an
active-plan count. Before merge, a follow-up feature-branch commit deliberately
removed “active plan” language for the default case. The public model therefore
added passive plans as an exception while existing plans remained ordinary,
non-passive plans.

## References

- [ADR-0008: Define shutdown and shutdown-time plan semantics](0008-define-shutdown-semantics.md)
- [Issue #916](https://github.com/CDCgov/ixa/issues/916) (association inferred
  from the feature-branch name)
- [PR #981: Passive plans](https://github.com/CDCgov/ixa/pull/981)
- [Feature implementation commit `67a4ccc`](https://github.com/CDCgov/ixa/commit/67a4ccca5212001a9382264ac5f6a993162d1551)
- [Feature terminology commit `1df0392`](https://github.com/CDCgov/ixa/commit/1df0392784353f1e0c5ba1d09dff9e529f687cda)
- [Adopted commit `a85f0fd`](https://github.com/CDCgov/ixa/commit/a85f0fd5d98c9114d74c2289a1ae81ba92d17a3c)

The implementation plan in `Notes/plan-passive-plans.md`, the retained feature
branch, the merged implementation, and focused `Context` and `PlanQueue` tests
supplied the reconstruction evidence for this record. The branch was based on
the adopted shutdown-semantics commit from ADR-0008; PR #981 was merged as a
squashed commit with the same substantive passive-plan behavior and the
feature branch's final non-active terminology.
