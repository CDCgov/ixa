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
  "runs": [
    {
      "run_at": "2020-01-01T00:00:00.000Z",
      "branch": "main",
      "hyperfine": [],
      "criterion": []
    }
  ]
}
JSON

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
  --history-out "$workdir/bench-history.out.json"

jq -e '.branch=="feature/test" and (.hyperfine.results|length)==1 and (.criterion.results|length)>=1' "$workdir/bench-current.json" >/dev/null
jq -e '(.runs|length)==2 and (.runs[-1].branch=="feature/test")' "$workdir/bench-history.out.json" >/dev/null

# Validate criterion time triple parsed.
low_ms=$(jq -r '.criterion.results[] | select(.name=="sample_people/another_bench") | .time_text[0]' "$workdir/bench-current.json")
if [[ "$low_ms" != "10.771 ms" ]]; then
  echo "Expected criterion low time 10.771 ms, got: $low_ms" >&2
  exit 1
fi

echo "OK: bench_results.js test passed"
