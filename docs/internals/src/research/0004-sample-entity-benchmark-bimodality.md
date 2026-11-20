# Research-0004: `sample_entity_whole_population` benchmark bimodality

Date: 2026-05-07

> This is a historical benchmark investigation. Its measurements and source
> descriptions apply to the revisions and environments examined at that time.

## Context

The `sample_entity_whole_population` benchmark family in
`ixa-bench/criterion/sample_entity_scaling.rs` measures
`context.sample_entity(SampleScalingRng, Mosquito)` for population sizes 1,000,
10,000, and 100,000.

Historically these benchmarks measured around 10-12 ns. Later runs became
bimodal: all three population sizes tend to be either low, around 11 ns, or high,
around 35 ns.

The important observation is that the three population-size variants move
together. This strongly suggests a shared execution mode or shared external
condition rather than a transient machine-local condition.

## Explicitly Ruled Out

### Population size as the direct cause

The hot path for a whole-population query is the empty-query fast path in
`src/entity/context_extension.rs`. It:

1. Gets the entity count.
2. Borrows the RNG associated with `SampleScalingRng`.
3. Samples one integer in `0..population`.
4. Wraps the index in `EntityId`.

For the benchmarked sizes, all populations are below `u32::MAX`, so all three
sizes use the same `u32` range-sampling branch. The only meaningful difference is
the range bound. That does not explain all three variants switching together
between the same two performance bands.

### OS entropy exhaustion or hardware random-number entropy

This benchmark does not draw entropy from the OS or hardware in the measured hot
path.

`define_rng!` uses `rand::rngs::SmallRng`. Ixa initializes it through
`SeedableRng::seed_from_u64(base_seed + hash_str(R::get_name()))` in
`src/random/context_ext.rs`. `SmallRng` is a deterministic PRNG. With
`rand 0.9.2` on a 64-bit target, it is `Xoshiro256PlusPlus`.

Also, `setup_context` has already used `SampleScalingRng` while constructing the
population, so the RNG has already been created before the benchmarked operation.
The measured operation does not call `OsRng`, `getrandom`, `/dev/urandom`, or a
hardware entropy instruction.

### Allocator slow path in the measured operation

Allocation is a useful example of a runtime subsystem that can introduce
high-variance behavior, but it does not appear applicable to this measured hot
path.

After setup, the measured closure only samples an entity. It does not add
entities, build indexes, allocate collections, or construct distributions. The
RNG holder already exists, so `Context::sample` should only perform the
`RefCell` borrow, `HashMap` lookup, downcast, and RNG call. Those operations
should not allocate in steady state.

### A short transient interruption during one benchmark invocation

Criterion's default configuration warms up and then measures for about 5 seconds
with 100 samples. A brief interruption should usually be diluted by the
measurement window and should not consistently move the estimate from 11 ns to
35 ns.

This does not rule out a condition that persists for most or all of the
benchmark group. Criterion can measure a slow regime accurately if the process is
in that regime during the measurement.

### Range-sampling rejection behavior

`rand` integer range sampling can take an extra random word in rare cases. That
is not a plausible explanation for this bimodality.

For ranges like 1,000, 10,000, and 100,000, the extra-work probability is tiny
and should be range-dependent. It would not naturally make all three population
sizes track together in the same 11 ns vs 35 ns bands.

## Unlikely, But Not Proven Impossible

### A direct source-path change in `sample_entity_whole_population`

No source-path change has been identified that explains the bimodality. The
current path remains the empty-query fast path: entity count, RNG lookup, integer
range sample, `EntityId` construction.

One strong clue against a source-path explanation is PR #847. Two benchmark
history points on that PR showed low and high measurements for
`sample_entity_whole_population`, but the diff between those two commits only
touched `examples/network-hhmodel/*`, not `src`, `ixa-bench`, `Cargo.toml`, or
`Cargo.lock`.

This does not prove that no source-level high-variance mechanism exists, but it
makes a direct code-path regression less likely.

### "The whole machine was slower" as inferred from other benchmark groups

The benchmark workflow runs Criterion groups as a GitHub Actions matrix. Each
group can run on a different `ubuntu-latest` runner. Therefore, the absence of
slowdowns in `counts`, `indexing`, or `large_dataset` does not strongly constrain
the runner state for `sample_entity_scaling`.

Comparisons against other benchmark groups are still useful historically, but
they are not same-machine controls.

## Reasonable Hypotheses

### CPU-bound microbenchmark sensitivity to runner CPU regime

This is the leading hypothesis.

`sample_entity_whole_population` is unusually CPU-bound. It mostly measures a
small fixed sequence of integer operations, branches, and wrapper overhead. That
makes it sensitive to:

- CPU model and microarchitecture
- sustained clock frequency or turbo state
- VM steal time
- core placement
- scheduler behavior
- branch prediction and cache state
- compiler codegen interacting with the specific CPU

Mixed-mode or memory-bound benchmarks can be less sensitive to pure CPU speed.
If a benchmark spends meaningful time stalled on cache misses, memory latency,
pointer chasing, allocation, or larger data-structure traversal, then a slower
core does not necessarily translate into the same percentage slowdown. In
contrast, a tiny CPU-bound operation can show a large percentage swing from a
stable absolute per-iteration penalty.

This explains why a ~20 ns per-call difference could dominate an 11 ns
microbenchmark while being much less visible in microsecond-scale benchmarks.

### Job-level rather than suite-level runner state

The fact that the three population sizes track together is consistent with a
condition affecting the whole `sample_entity_scaling` job: CPU frequency, VM
scheduling, noisy neighbor behavior, or runner hardware.

Criterion would not remove this. Criterion reduces random within-run noise, but
if the process is in a stable slow regime for the entire measurement window, the
reported estimate will reflect that regime.

### Ixa RNG wrapper overhead as an amplification point

The raw RNG is likely not the only measured work. `Context::sample` also performs
a `RefCell` borrow, looks up the RNG holder in a `HashMap&lt;TypeId, RngHolder&gt;`,
and downcasts it.

This path should be steady-state and allocation-free, but it may still be a
significant fraction of an 11 ns benchmark. If some part of this wrapper is
especially sensitive to CPU regime, it could amplify the bimodality.

The way to distinguish this from raw CPU/RNG sensitivity is to add sentinel
benchmarks that decompose the path.

## Additional Details

### Criterion still runs the benchmark for a substantial amount of time

The tiny per-operation time does not mean Criterion is only timing one tiny
operation. Criterion's default configuration uses a 3 second warmup, a 5 second
measurement target, and 100 samples.

The relevant limitation is not that Criterion under-measures the benchmark. The
limitation is that Criterion assumes the measured process is reasonably
stationary during the run. A stable runner regime shift is not a transient
outlier from Criterion's perspective.

### Latest local sanity check

A local run on 2026-05-07 measured the current benchmark around 4.2 ns on the
developer machine. Temporarily black-boxing the sampled `Option&lt;EntityId&lt;_&gt;&gt;`
moved it only modestly, around 4.5 ns in a short run. That makes complete
dead-code elimination an unlikely explanation for the GitHub 11 ns vs 35 ns
bimodality.

This local number is not directly comparable to GitHub Actions. Its value is
mainly diagnostic: the benchmark remains extremely small and CPU-bound.

### Benchmark history has PR-level replacement behavior

`scripts/bench_results.mjs` keeps a single entry per PR number when updating
`bench-history.json`. Reruns for a PR replace the prior entry for that PR in the
current history file.

The Git history of the `benchmarks-history` branch still contains prior points,
but the latest JSON is not a full append-only record of every rerun for each PR.
That matters when interpreting the visual time series.

## Ideas for More Robust and Interpretable Benchmarks

### Add sentinel benchmarks inside `sample_entity_scaling`

Add one or more same-job sentinel benchmarks to make the job's CPU regime visible.
These should run in the same Criterion group or at least the same matrix job as
`sample_entity_whole_population`.

Candidate sentinels:

- Raw `SmallRng::next_u64`.
- Raw `SmallRng::random_range(0..1000u32)`.
- `context.sample(SampleScalingRng, |rng| rng.random_range(0..1000u32))`.
- A tiny deterministic integer-arithmetic loop with no memory allocation.
- Possibly a memory-oriented sentinel, such as a simple pointer-chasing or
  cache-sensitive loop, to separate CPU-bound from memory-bound runner regimes.

Interpretation:

- If raw `SmallRng` sentinels go high with `sample_entity_whole_population`, the
  cause is likely runner CPU regime or raw integer-code throughput.
- If raw `SmallRng` stays flat but `context.sample` goes high, the interesting
  layer is Ixa's RNG-wrapper path.
- If both stay flat but `sample_entity_whole_population` goes high, the entity
  fast path needs deeper investigation.

### Normalize microbenchmarks against a sentinel

For charts and PR comparisons, show both absolute time and normalized time:

```text
normalized_sample_entity = sample_entity_whole_population / rng_or_cpu_sentinel
```

This would not make the absolute benchmark disappear. It would add a second view
that asks whether `sample_entity_whole_population` changed relative to the CPU
regime of that particular runner job.

The sentinel should be close enough to the benchmark to correct for the relevant
runner mode, but not so close that it hides real regressions. A small set of
sentinels may be better than one.

### Log runner CPU details

Record basic runner information in benchmark artifacts:

- `lscpu`
- selected `/proc/cpuinfo` fields
- reported CPU MHz, if available
- number of cores
- maybe kernel and runner image version

This would allow high/low points to be grouped by CPU model or obvious runtime
environment differences.

### Keep developer-machine local benchmark history

Maintain a local, append-only benchmark history local to the developer's
machine. This should not replace GitHub Actions, but it can answer a different
question: "What changed on *my* mostly stable machine?"

Useful properties:

- Append-only local history rather than one entry per PR.
- Include git SHA, branch, dirty state, rustc version, Cargo.lock hash, CPU model,
  and OS version.
- Track the same sentinel benchmarks as CI.
- Prefer repeated runs over time rather than relying on a single local run.

This would provide a stable baseline for tiny CPU-bound benchmarks and help
separate CI-runner noise from true code changes.

### Preserve rerun history

For investigating variance, it would be useful to preserve every benchmark run
rather than replacing prior runs for the same PR number in the current
`bench-history.json`.

One option is to keep the current PR-centered view for dashboards while also
writing an append-only raw history artifact or branch file. This would make it
easier to distinguish code changes from rerun variance.
