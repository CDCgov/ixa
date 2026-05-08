On the `v2.0.0` branch:

### Setup
Make sure you have mise installed

```
curl https://mise.run | sh
```

And give permissions to mise.toml:

```
cd ixa
mise trust mise.toml
```

**You can always run mise run to get autocomplete for all the commands**:

<img width="571" height="469" alt="image" src="https://github.com/user-attachments/assets/6bfbde7b-4709-4113-a9be-b047bc1f16a7" />


## Running the reference SIR

The baseline version:
```
cargo run --bin run_bench -p ixa-bench --release -- --group large_sir --bench baseline
```

The the entities version:
```
cargo run --bin run_bench -p ixa-bench --release -- --group large_sir --bench entities
```


### Running the SIR reference benchmark

```
mise run bench:hyperfine
````

### Running the sample_entity benchmark

This is a scaling analysis on `sample_entity`

```
mise run bench:criterion sample_entity
```

At the end, you'll get a summary:

```
=== Scaling summary: sample_entity_whole_population ===
  baseline: n=1000, t=6.82 ns/sample
  ratios vs baseline:
    n=   1000:       6.82 ns/sample  (x1.000)
    n=  10000:       6.71 ns/sample  (x0.984)
    n= 100000:       6.78 ns/sample  (x0.995)

=== Scaling summary: sample_entity_single_property_indexed ===
  baseline: n=1000, t=51.15 ns/sample
  ratios vs baseline:
    n=   1000:      51.15 ns/sample  (x1.000)
    n=  10000:      51.23 ns/sample  (x1.002)
    n= 100000:      51.45 ns/sample  (x1.006)

=== Scaling summary: sample_entity_multi_property_indexed ===
  baseline: n=1000, t=102.70 ns/sample
  ratios vs baseline:
    n=   1000:     102.70 ns/sample  (x1.000)
    n=  10000:     102.07 ns/sample  (x0.994)
    n= 100000:     102.31 ns/sample  (x0.996)
```

