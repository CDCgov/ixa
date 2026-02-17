#!/usr/bin/env bash
set -euo pipefail

# Integration test for scripts/format_bench_pr_comment.mjs
#
# Usage:
#   bash scripts/test_format_bench_pr_comment.sh <group1> <group2>
#
# The script will create two temporary group files under a temp directory:
#   criterion-regressions-<group>.txt
# then run the formatter and compare the merged markdown to a golden expected output.

group1="${1:-}"
group2="${2:-}"

if [[ -z "$group1" || -z "$group2" ]]; then
  echo "Usage: bash scripts/test_format_bench_pr_comment.sh <group1> <group2>" >&2
  exit 2
fi

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT

# Group 1: full structured output (from user sample)
cat >"$workdir/criterion-regressions-${group1}.txt" <<'TXT'
Regressions:
  Group                                    Bench                                              Change  CI Lower  CI Upper
  ---------------------------------------  ------------------------------------------------  -------  --------  --------
  sample_entity_single_property_indexed    100000                                             3.806%    3.117%    4.483%
  sample_entity_single_property_indexed    1000                                               2.592%    1.861%    3.328%
  sample_entity_single_property_unindexed  10000                                             29.296%   27.345%   31.246%
  sample_entity_single_property_unindexed  1000                                              24.533%   22.965%   25.916%
  sample_entity_multi_property_indexed     1000                                               0.774%    0.193%    1.318%
  sampling                                 sampling_multiple_unindexed_entities               2.509%    1.929%    3.084%
  sample_entity_whole_population           100000                                             2.338%    1.628%    3.068%

Improvements:
  Group                                    Bench                                              Change  CI Lower  CI Upper
  ---------------------------------------  ------------------------------------------------  -------  --------  --------
  sample_entity_single_property_unindexed  100000                                            -1.609%   -2.199%   -1.030%
  sample_entity_multi_property_indexed     10000                                             -0.778%   -1.424%   -0.142%
  sample_entity_multi_property_indexed     100000                                            -1.322%   -1.906%   -0.803%
  large_dataset                            bench_query_population_property_entities          -3.877%   -5.095%   -2.738%
  large_dataset                            bench_query_population_multi_unindexed_entities   -1.456%   -2.258%   -0.635%
  sampling                                 sampling_multiple_l_reservoir_entities            -6.446%   -7.226%   -5.624%
  sampling                                 sampling_single_l_reservoir_entities              -3.197%   -3.653%   -2.708%
  sampling                                 sampling_single_known_length_entities             -1.262%   -1.846%   -0.632%
  sampling                                 sampling_multiple_known_length_entities           -2.151%   -2.605%   -1.718%
  sampling                                 sampling_single_unindexed_entities                -1.895%   -2.249%   -1.550%
  sample_entity_whole_population           10000                                             -1.242%   -2.009%   -0.557%

Unchanged:
  Group                                    Bench                                              Change  CI Lower  CI Upper
  ---------------------------------------  ------------------------------------------------  -------  --------  --------
  sample_entity_single_property_indexed    10000                                             -0.267%   -1.131%    0.584%
  large_dataset                            bench_filter_indexed_entity                        0.930%  -11.085%   14.694%
  large_dataset                            bench_filter_unindexed_entity                      1.463%   -3.150%    6.249%
  large_dataset                            bench_match_entity                                 0.144%   -1.151%    1.235%
  large_dataset                            bench_query_population_multi_indexed_entities      0.191%   -0.490%    0.849%
  large_dataset                            bench_query_population_derived_property_entities   0.011%   -0.529%    0.545%
  large_dataset                            bench_query_population_indexed_property_entities  -0.466%   -0.992%    0.018%
  sample_entity_whole_population           1000                                              -1.203%   -2.912%    0.150%
TXT

# Group 2: empty output to ensure concatenation has no blank line between groups
: >"$workdir/criterion-regressions-${group2}.txt"

out="$workdir/results.md"
node "$(pwd)/scripts/format_bench_pr_comment.mjs" \
  --out "$out" \
  --hyperfine-md /dev/null \
  --criterion-dir "$workdir" \
  --groups "${group1},${group2}"

expected="$workdir/expected.md"
cat >"$expected" <<'MD'
### Benchmark Results

#### Hyperfine

_Hyperfine output missing._

#### Criterion

##### Regressions

```
  Group                                    Bench                                              Change  CI Lower  CI Upper
  ---------------------------------------  ------------------------------------------------  -------  --------  --------
  sample_entity_single_property_indexed    100000                                             3.806%    3.117%    4.483%
  sample_entity_single_property_indexed    1000                                               2.592%    1.861%    3.328%
  sample_entity_single_property_unindexed  10000                                             29.296%   27.345%   31.246%
  sample_entity_single_property_unindexed  1000                                              24.533%   22.965%   25.916%
  sample_entity_multi_property_indexed     1000                                               0.774%    0.193%    1.318%
  sampling                                 sampling_multiple_unindexed_entities               2.509%    1.929%    3.084%
  sample_entity_whole_population           100000                                             2.338%    1.628%    3.068%
(no output)
```

##### Improvements

```
  Group                                    Bench                                              Change  CI Lower  CI Upper
  ---------------------------------------  ------------------------------------------------  -------  --------  --------
  sample_entity_single_property_unindexed  100000                                            -1.609%   -2.199%   -1.030%
  sample_entity_multi_property_indexed     10000                                             -0.778%   -1.424%   -0.142%
  sample_entity_multi_property_indexed     100000                                            -1.322%   -1.906%   -0.803%
  large_dataset                            bench_query_population_property_entities          -3.877%   -5.095%   -2.738%
  large_dataset                            bench_query_population_multi_unindexed_entities   -1.456%   -2.258%   -0.635%
  sampling                                 sampling_multiple_l_reservoir_entities            -6.446%   -7.226%   -5.624%
  sampling                                 sampling_single_l_reservoir_entities              -3.197%   -3.653%   -2.708%
  sampling                                 sampling_single_known_length_entities             -1.262%   -1.846%   -0.632%
  sampling                                 sampling_multiple_known_length_entities           -2.151%   -2.605%   -1.718%
  sampling                                 sampling_single_unindexed_entities                -1.895%   -2.249%   -1.550%
  sample_entity_whole_population           10000                                             -1.242%   -2.009%   -0.557%
(no output)
```

##### Unchanged

```
  Group                                    Bench                                              Change  CI Lower  CI Upper
  ---------------------------------------  ------------------------------------------------  -------  --------  --------
  sample_entity_single_property_indexed    10000                                             -0.267%   -1.131%    0.584%
  large_dataset                            bench_filter_indexed_entity                        0.930%  -11.085%   14.694%
  large_dataset                            bench_filter_unindexed_entity                      1.463%   -3.150%    6.249%
  large_dataset                            bench_match_entity                                 0.144%   -1.151%    1.235%
  large_dataset                            bench_query_population_multi_indexed_entities      0.191%   -0.490%    0.849%
  large_dataset                            bench_query_population_derived_property_entities   0.011%   -0.529%    0.545%
  large_dataset                            bench_query_population_indexed_property_entities  -0.466%   -0.992%    0.018%
  sample_entity_whole_population           1000                                              -1.203%   -2.912%    0.150%
(no output)
```
MD

# Normalize CRLF if any
if diff -u "$expected" "$out"; then
  echo "OK: format_bench_pr_comment.mjs integration test passed"
else
  echo "ERROR: merged output did not match expected" >&2
  exit 1
fi

# Security check: reject invalid group names (path traversal-like input).
if node "$(pwd)/scripts/format_bench_pr_comment.mjs" \
  --out "$workdir/invalid.md" \
  --hyperfine-md /dev/null \
  --criterion-dir "$workdir" \
  --groups "../etc/passwd" 2>"$workdir/invalid.err"; then
  echo "ERROR: expected invalid group name to fail" >&2
  exit 1
fi
if ! grep -q 'Invalid group name' "$workdir/invalid.err"; then
  echo "ERROR: expected invalid group error message" >&2
  exit 1
fi

# Security check: ensure fences expand to avoid code fence break-out.
fence_group="fencecheck"
cat >"$workdir/criterion-regressions-${fence_group}.txt" <<'TXT'
Unchanged:
payload line
```malicious-breakout
TXT
echo 'bench ``` table' >"$workdir/hyperfine-fence.md"

fence_out="$workdir/fence.md"
node "$(pwd)/scripts/format_bench_pr_comment.mjs" \
  --out "$fence_out" \
  --hyperfine-md "$workdir/hyperfine-fence.md" \
  --criterion-dir "$workdir" \
  --groups "$fence_group"

if ! grep -q '^````$' "$fence_out"; then
  echo "ERROR: expected expanded markdown fence (4 backticks)" >&2
  exit 1
fi
