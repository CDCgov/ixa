# Research-0005: Plan queue benchmarks

**Investigation date:** 2026-05-21 (best estimate from the source artifact)

> This is a historical benchmark snapshot. It records why a `VecDeque` plan
> queue was not adopted despite performing well under nearly ordered synthetic
> workloads.

**Hypothesis:** Plans tend to be scheduled in "almost linear order" in the sense that new plans tend to be scheduled at or near the end of the queue. If we want to assume this is typical, we can use a deque (a `VecDeque`) instead of a binary heap, and insert new plans in sorted order. Basically you'd walk the deque from the rear to find the right insertion slot, shifting each existing plan down as you go to make room for the new item. Insertion isn't constant, but you're only shifting the tail of the deque to make room for the new element, and the tail is assumed to be small. You would pop from the head of the deque as usual.

**Empirical Evidence:** The criterion benchmarks confirm that a deque is indeed faster under the "almost linear order" assumption. However, that assumption does not hold for `ixa-epi-covid`, and the negative consequences scale superlinearly with population size.

## Criterion

Targeted criterion benchmarks comparing the original `BinaryHeap` implementation to the `VecDeque` implementation.

**`window`:** How many plans are in flight.

**`jitter`:** the maximum distance a new plan may be scheduled before the current tail

A `jitter = 0` is fully ordered insertion. Larger jitter values model increasing disorder. Values at or above `window` allow insertion anywhere in the active window, which is the benchmark’s maximally disordered case.

| Benchmark Name                                 | `BinaryHeap` (ms) | `VecDeque` (ms) | Relative Improvement |
| ---------------------------------------------- | ----------------- | --------------- | -------------------- |
| plan_queue/steady_state/window_128_jitter_0    | 1.1901            | 0.56237         | 52.75%               |
| plan_queue/steady_state/window_128_jitter_8    | 1.7736            | 1.0526          | 40.65%               |
| plan_queue/steady_state/window_128_jitter_64   | 1.7399            | 1.3869          | 20.29%               |
| plan_queue/steady_state/window_128_jitter_512  | 1.7858            | 1.7294          | 3.16%                |
| plan_queue/steady_state/window_1024_jitter_0   | 1.4973            | 0.57047         | 61.90%               |
| plan_queue/steady_state/window_1024_jitter_8   | 2.0999            | 1.0448          | 50.25%               |
| plan_queue/steady_state/window_1024_jitter_64  | 2.3207            | 1.3586          | 41.46%               |
| plan_queue/steady_state/window_1024_jitter_512 | 2.3559            | 3.5008          | -48.60%              |
| plan_queue/steady_state/window_8192_jitter_0   | 2.1235            | 0.54151         | 74.50%               |
| plan_queue/steady_state/window_8192_jitter_8   | 2.4272            | 1.0401          | 57.15%               |
| plan_queue/steady_state/window_8192_jitter_64  | 2.6310            | 1.3503          | 48.68%               |
| plan_queue/steady_state/window_8192_jitter_512 | 2.8647            | 3.4349          | -19.90%              |

## `ixa-epi-covid` Benchmarks

The `ixa-epi-covid` model is very plan-heavy, with maximum in-flight plans of ~1.87 * population size.

| Population size | `BinaryHeap` | `VecDeque`  |
| --------------- | ------------ | ----------- |
| 1,000           | 28ms 258us   | 22ms 912us  |
| 10,000          | 60ms 421us   | 244ms 962us |
| 100,000         | 443ms 127us  | 21s 502ms   |
| 1,000,000       | 7s 373ms     | timeout     |
