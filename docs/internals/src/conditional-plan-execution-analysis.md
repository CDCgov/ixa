# Conditional plan execution: three possible approaches

Here is my analysis of three different strategies for dealing with "canceling" in-flight plans associated with people who die. To understand _why_ the pros and cons are what they are, it helps to first explain how plans are stored internally in Ixa today.

## How plans work internally

A scheduled plan is currently split into two parts.

- The plan timing information - the `PlanId`, the execution time, and the execution phase - is stored in a priority queue called `queue`, implemented as a binary heap.
- The plan payload - at the moment, this is just the callback to run - is stored separately in a hash map called `data_map`, keyed by `PlanId`: `data_map: HashMap<PlanId, Payload>`.

This design matters because cancellation is already done lazily in a sense: When a plan is cancelled, Ixa removes the payload from `data_map`, but it does *not* remove the corresponding entry from `queue`. Later, when the scheduler looks for the next plan to execute, it may pop entries from `queue` that no longer have a payload in `data_map`, and simply discard them. The rationale for this design decision is:

- The removal operation for the `queue` involves a search (*O(log n)*) followed by a binary heap "fix up", and that operation is actually less efficient than just throwing out already cancelled plans that are popped from the top of the `queue` while fetching the next plan to execute.
- Said another way, removal from the `queue` is just ammortized over the life of the simulation rather than done eagerly.
- The assumption is that the memory cost of not eagerly removing items from `queue` is negligible, because the number of cancelled plans that are still "in flight" is expected to be small.

## 1. Bookkeeping in a separate "plan index", then bulk cancellation when a person dies

In this approach, client code keeps an additional index from each person to the plans associated with that person in a hash map `HashMap<PersonId, Vec<PlanId>>`. When the person dies, the model looks up all of those plans and cancels them. At first this sounds appealing because it gives a direct way to say "cancel all future plans for this person." But in practice the advantages don't really materialize.

The first problem is bookkeeping. Plans are added over time, so the index has to keep track of every plan that was ever associated with a person, for every single person. Removing plans from the index when they execute is awkward, so the simplest version is to leave executed plans in the index and only clear them out when the person dies. That means the index may hold a great deal of stale information over the course of a long simulation.

The second problem is that bulk cancellation is less valuable than it may sound. Cancelling a plan removes its payload from `data_map`, but it still does not remove the corresponding entry from `queue`. So even after bulk cancellation, the timing entries remain in the queue until they eventually reach the top and are discarded. In other words, this approach does not truly "remove all future plans for this person" from the scheduler. It only removes their payloads early.

That means the memory savings are limited. What we save is only the *payload* storage for plans that belong to already-dead people and have not yet been reached in simulated time. If that number is small, then the savings is small. Against that, we must pay the ongoing cost of maintaining a separate plan index for all people, across the whole simulation. On memory grounds, this is clearly a poor trade.

There is one real possible advantage: if plans are cancelled in advance, we do not have to do an "is this person still alive?" check later when those plans come due. That might save some runtime. But it is not obvious that this savings is large enough to justify the extra bookkeeping, especially since the alive check itself is simple (just an index into a vector and with a small method call overhead).

Overall, this approach seems unattractive. It adds substantial bookkeeping complexity, stores extra information for the whole simulation, and gives only limited benefit because cancelled plans still remain in the queue.

Other advantages:

- Implementable in client code; no modifications to ixa core necessary

## 2. Add an optional `RunCondition` to the plan payload

In this approach, a plan may optionally carry a `RunCondition` along with its callback. When the scheduler is choosing the next plan to execute, it checks the condition. If the condition does not hold, the plan is skipped.

This matches the current internal design much better than the plan-index approach. Ixa already uses lazy cancellation: plans can remain in the queue even after their payload has effectively been removed. A `RunCondition` is similar in spirit. Instead of eagerly trying to remove plans from the scheduler, we wait until a plan is about to run and decide then whether it is still valid.

This has several advantages.

First, it avoids the extra bookkeeping cost of a separate per-person plan index. We do not need to remember all plans associated with all people for the whole simulation on top of the plan storage subsystem that already exists.

Second, it is general. The condition does not have to be "person is alive." It could be any rule that can be checked from the current simulation state. That makes it useful for other cases too.

Third, it makes conditional execution a built-in feature of the scheduler itself. That means Ixa could later do more with it if desired, for example recording how many plans were skipped or supporting better debugging tools around skipped plans. We could have first-class support for a *semantics* of plan execution built into ixa core.

The main disadvantage is that this adds some complexity to the core plan system. Plans are no longer just callbacks; they may carry an additional condition. The scheduler also has to check that condition when deciding what to run next. This is not a huge conceptual change, but it is more machinery inside Ixa itself than the lightweight wrapper approach described below.

A second possible disadvantage is runtime cost. Every gated plan now requires checking its condition when it is reached. If most plans are gated, and if the condition is not trivial, that cost could matter. On the other hand, for the simple "person is alive" case, the check is likely small, and this cost may be entirely acceptable in practice.

Overall, this is a good fit if we think conditional plan execution is something Ixa itself should support as a first-class feature, not just a one-off convenience for a single model. Really its main selling point is that it provides a clear path forward for future enrichment of plan execution semantics.

But we can get the same functionality with far less infrastructure by using strategy 3 of the next section.

Advantages:

- first-class support for "execution semantics" of plans
- simple to understand and reason about
- provides a more generic feature ("here is a condition to determine if a plan should execute") that can be used in other use cases.

Disadvantages:

- Requires support in ixa core, not just implementation in client code.
- We'd still probably want a convenience method in client code of the form `add_plan_for_person` that is implemented in terms of `RunCondition` anyway. But this is easy to do.



## 3. A lightweight wrapper in client code: `add_plan_for_person`

The simplest approach is to keep Ixa unchanged and handle the issue in model code. A helper like `add_plan_for_person` can wrap the user's callback in another callback that first checks whether the person is alive, and only then runs the original handler.

```rust
/// Adds a plan for the given person if and only if that person is 
/// alive when the plan comes due.
fn add_plan_for_person(
    &mut self,
    person_id: PersonId,
    time: f64,
    callback: impl FnOnce(&mut Context) + 'static,
) -> PlanId {
    self.add_plan(
        time,
        |context| {
            // Only execute callback if the person is still alive.
            let Alive(is_alive) = context.get_property::<Alive>(person_id);
            if is_alive {
              	callback(context)
          	}
        }
    )
}
```

This is attractive because it is so simple. It does not change Ixa internals, does not require new bookkeeping, and is easy for model authors to understand. For the concrete case of person-associated plans, it expresses exactly what we want: "run this only if the person is still alive."

Client code can still schedule plans unconditionally by using the existing `Context::add_plan` API. In other words, client code uses `add_plan_for_person` *if and only if* client code wants the plan execution gated on an "is alive" check.

This approach can also be generalized to arbitrary conditions. Instead of hard-coding an alive check, the helper can take a `RunCondition` argument and apply it inside the wrapper callback. That makes it much closer in spirit to the built-in `RunCondition` approach *but without needing any support in ixa core.*

```rust
/// Adds a plan for the given entity to be executed only if the `RunCondition` holds.
fn add_plan_for_person(
    &mut self,
    person_id: PersonId,
    time: f64,
    callback: impl FnOnce(&mut Context) + 'static,
  	run_condition: impl RunCondition
) -> PlanId {
    self.add_plan(
        time,
        |context| {
            // Only execute callback if the run condition holds
            if run_condition.should_run(context) {
              	callback(context)
          	}
        }
    )
}
```

Compared with the plan-index approach, the wrapper is clearly simpler and likely more memory-efficient. There is no extra global index to maintain, and no need to keep track of every plan ever associated with every person. The cost is just the condition check at execution time. This cost isn't zero, but it's likely small. We need to measure it.

Compared with the built-in `RunCondition` approach, the lightweight wrapper contributes less to a future execution-semantics framework, but it remains forward-compatible with one. The scheduler itself does not know that a plan is conditional. From Ixa's point of view, it is just an ordinary callback that sometimes returns immediately without doing anything.

One consequence of this is, in the generalized wrapper design, the condition cannot naturally receive the `PlanId` unless the underlying scheduling API changes, whereas if we had full in-built support in ixa for `RunCondition`, we could include both `context: &Context` and `plan_id: PlanId` parameters to the `RunCondition::should_run` method. Still, for the immediate use case, that difference may not matter much. If all we need is "do nothing if the person is no longer alive," then the wrapper behaves almost the same as a built-in condition check.



## Overall comparison

The plan-index strategy is the weakest of the three. It adds the most bookkeeping, stores extra information for the whole simulation, and gets less benefit than one might expect because cancelled plans still remain in the queue.

The built-in `RunCondition` strategy is the most powerful and the cleanest if we want conditional execution to be a first-class concept within Ixa. It fits naturally with the current lazy approach to plan cancellation, avoids the cost of a separate index, and provides a scaffolding for richer plan execution semantics, introspection on execution conditions, statistics collection, and so forth, that we might want to conceptually attach to conditional execution. 

The lightweight wrapper strategy is the simplest. It solves the immediate problem with very little machinery, keeps the core plan system unchanged, and can be generalized enough to cover many practical cases. Its main limitation is that it remains agnostic about architecting richer support for inspecting or analyzing skipped plans.

My personal choice: lightweight wrappers give us the biggest payout for the lowest cost and is pretty low-stakes.

## Shared API over all three strategies

Under all three strategies you would have an `add_plan_for_person` helper method as the primary access point to the functionality for client code. In the jargon of software engineering, the `add_plan_for_person` helper provides an "abstraction boundary" that prevents us from having to change every single call site in client code in the event, for example, that we do decide to have first-class support for `RunCondition` execution gates in ixa core's plan execution subsystem. We would only have to change the implementation of `add_plan_for_person`. This reduces [coupling](https://en.wikipedia.org/wiki/Coupling_(computer_programming)) between client code implementation and implementation of ixa core.



