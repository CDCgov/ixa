#!/usr/bin/env bash
set -euo pipefail

# Local test harness for scripts/bench_results.js
# Usage:
#   bash scripts/test_bench_results.sh

repo="acme/example"

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT

cat >"$workdir/hyperfine.json" <<'JSON'
{
  "results": [
    {
      "command": "echo hello",
      "mean": 0.012,
      "min": 0.011,
      "max": 0.013,
      "stddev": 0.001,
      "times": [0.011, 0.012, 0.013]
    }
  ]
}
JSON

cat >"$workdir/criterion-compare.txt" <<'TXT'
Benchmarking sample_people/sampling_multiple_l_reservoir
Benchmarking sample_people/another_bench
  time:   [10.771 ms 10.811 ms 10.866 ms]
TXT

cat >"$workdir/bench-history.json" <<'JSON'
{
  "schema": 1,
  "updated_at": "2020-01-01T00:00:00.000Z",
  "runs": []
}
JSON

history1="$workdir/bench-history.1.json"
history2="$workdir/bench-history.2.json"
history3="$workdir/bench-history.3.json"

node "$(pwd)/scripts/bench_results.js" \
  --repo "$repo" \
  --branch "feature/test" \
  --pr-number 42 \
  --base-ref main --base-sha 1111111 \
  --head-ref feature/test --head-sha 2222222 \
  --run-at "2026-02-09T00:00:00.000Z" \
  --hyperfine-json "$workdir/hyperfine.json" \
  --criterion-log "$workdir/criterion-compare.txt" \
  --history-in "$workdir/bench-history.json" \
  --out-current "$workdir/bench-current.json" \
  --history-out "$history1"

jq -e '.branch=="feature/test" and (.hyperfine.results|length)==1 and (.criterion.results|length)>=1' "$workdir/bench-current.json" >/dev/null
jq -e '(.runs|length)==1 and (.runs[0].pr_number==42) and (.runs[0].head.sha=="2222222")' "$history1" >/dev/null

# Add another PR; should append (history grows).
node "$(pwd)/scripts/bench_results.js" \
  --repo "$repo" \
  --branch "feature/other" \
  --pr-number 43 \
  --base-ref main --base-sha 1111111 \
  --head-ref feature/other --head-sha 3333333 \
  --run-at "2026-02-09T01:00:00.000Z" \
  --hyperfine-json "$workdir/hyperfine.json" \
  --criterion-log "$workdir/criterion-compare.txt" \
  --history-in "$history1" \
  --out-current "$workdir/bench-current.json" \
  --history-out "$history2"

jq -e '(.runs|length)==2 and ([.runs[].pr_number]|sort)==[42,43]' "$history2" >/dev/null

# Re-run PR 42; should update existing entry (history length unchanged).
node "$(pwd)/scripts/bench_results.js" \
  --repo "$repo" \
  --branch "feature/test-rerun" \
  --pr-number 42 \
  --base-ref main --base-sha 1111111 \
  --head-ref feature/test-rerun --head-sha 4444444 \
  --run-at "2026-02-09T02:00:00.000Z" \
  --hyperfine-json "$workdir/hyperfine.json" \
  --criterion-log "$workdir/criterion-compare.txt" \
  --history-in "$history2" \
  --out-current "$workdir/bench-current.json" \
  --history-out "$history3"

jq -e '(.runs|length)==2 and ([.runs[].pr_number]|sort)==[42,43]' "$history3" >/dev/null
jq -e '([.runs[] | select(.pr_number==42)] | length)==1' "$history3" >/dev/null
jq -e '(.runs[] | select(.pr_number==42) | .head.sha)=="4444444" and (.runs[] | select(.pr_number==42) | .run_at)=="2026-02-09T02:00:00.000Z" and (.runs[] | select(.pr_number==42) | .branch)=="feature/test-rerun"' "$history3" >/dev/null
jq -e '(.runs[] | select(.pr_number==43) | .head.sha)=="3333333"' "$history3" >/dev/null

# Validate criterion time triple parsed.
low_ms=$(jq -r '.criterion.results[] | select(.name=="sample_people/another_bench") | .time_text[0]' "$workdir/bench-current.json")
if [[ "$low_ms" != "10.771 ms" ]]; then
  echo "Expected criterion low time 10.771 ms, got: $low_ms" >&2
  exit 1
fi

echo "OK: bench_results.js test passed"
