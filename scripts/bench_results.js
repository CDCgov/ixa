#!/usr/bin/env node
/*
Generates benchmark JSON artifacts from Hyperfine + Criterion outputs.

Usage (example):
  node scripts/bench_results.js \
    --repo owner/repo \
    --branch my-branch \
    --pr-number 123 \
    --base-ref main --base-sha abc \
    --head-ref feature --head-sha def \
    --hyperfine-json hyperfine.json \
    --criterion-log criterion-compare.txt \
    --history-in bench-history.json \
    --out-current bench-current.json \
    --history-out bench-history.json

All args are optional except input files; missing inputs produce empty result arrays.
*/

'use strict';

const fs = require('fs');
const path = require('path');

function die(message) {
  process.stderr.write(`${message}\n`);
  process.exit(2);
}

function getArgValue(argv, name) {
  const idx = argv.indexOf(name);
  if (idx === -1) return undefined;
  const value = argv[idx + 1];
  if (!value || value.startsWith('--')) die(`Missing value for ${name}`);
  return value;
}

function hasArg(argv, name) {
  return argv.includes(name);
}

function parseMaybeNumber(text) {
  if (text == null || text === '') return undefined;
  const n = Number(text);
  return Number.isFinite(n) ? n : undefined;
}

function parseDurationToSeconds(text) {
  // Examples: "12.3 ms ± 0.2 ms", "1.234 s", "456 µs"
  const m = String(text).trim().match(/([0-9]*\.?[0-9]+)\s*([a-zA-Zµμ]+)/);
  if (!m) return null;
  const value = Number(m[1]);
  const unitRaw = m[2];
  const unit = unitRaw.replace('μ', 'µ');
  const factor = {
    s: 1,
    sec: 1,
    ms: 1e-3,
    us: 1e-6,
    'µs': 1e-6,
    ns: 1e-9,
  }[unit] ?? null;
  if (!Number.isFinite(value) || factor == null) return null;
  return value * factor;
}

function readTextIfExists(filePath) {
  if (!filePath) return '';
  if (!fs.existsSync(filePath)) return '';
  return fs.readFileSync(filePath, 'utf8');
}

function readJsonIfExists(filePath) {
  if (!filePath) return null;
  if (!fs.existsSync(filePath)) return null;
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function parseCriterionCompareLog(text) {
  // Extracts benchmark name and the reported time confidence interval triple.
  // Example:
  //   Benchmarking sample_people/sampling_multiple_l_reservoir
  //   ...
  //   time:   [10.771 ms 10.811 ms 10.866 ms]
  const lines = String(text || '').split(/\r?\n/);
  const resultsByName = new Map();
  let currentName = null;

  for (const line of lines) {
    const benchMatch = line.match(/^Benchmarking\s+(.+?)\s*$/);
    if (benchMatch) {
      currentName = benchMatch[1].trim();
      continue;
    }

    const timeMatch = line.match(
      /\btime:\s*\[\s*([0-9]*\.?[0-9]+\s*[a-zA-Zµμ]+)\s+([0-9]*\.?[0-9]+\s*[a-zA-Zµμ]+)\s+([0-9]*\.?[0-9]+\s*[a-zA-Zµμ]+)\s*\]/,
    );
    if (!timeMatch || !currentName) continue;

    const t1 = timeMatch[1];
    const t2 = timeMatch[2];
    const t3 = timeMatch[3];
    const s1 = parseDurationToSeconds(t1);
    const s2 = parseDurationToSeconds(t2);
    const s3 = parseDurationToSeconds(t3);
    if (s1 == null || s2 == null || s3 == null) continue;

    resultsByName.set(currentName, {
      name: currentName,
      time_text: [t1, t2, t3],
      time_sec: [s1, s2, s3],
    });
  }

  return Array.from(resultsByName.values());
}

function parseHyperfineJson(hyperfineJson) {
  if (!hyperfineJson || !Array.isArray(hyperfineJson.results)) return [];

  return hyperfineJson.results.map((r) => {
    const times = Array.isArray(r.times) ? r.times.filter((n) => Number.isFinite(n)) : [];
    return {
      name: r.command ?? r.parameter ?? 'unknown',
      times_sec: times,
      mean_sec: Number.isFinite(r.mean) ? r.mean : undefined,
      min_sec: Number.isFinite(r.min) ? r.min : undefined,
      max_sec: Number.isFinite(r.max) ? r.max : undefined,
      stddev_sec: Number.isFinite(r.stddev) ? r.stddev : undefined,
    };
  });
}

function normalizeRunPrNumber(run) {
  if (!run || typeof run !== 'object') return undefined;
  const raw = run.pr_number;
  if (raw == null) return undefined;
  const n = Number(raw);
  return Number.isFinite(n) ? n : undefined;
}

function main() {
  const argv = process.argv.slice(2);

  if (hasArg(argv, '--help') || hasArg(argv, '-h')) {
    process.stdout.write(fs.readFileSync(__filename, 'utf8').split('\n').slice(0, 40).join('\n') + '\n');
    return;
  }

  const repo = getArgValue(argv, '--repo') ?? process.env.GITHUB_REPOSITORY;
  const branch = getArgValue(argv, '--branch') ?? process.env.RUN_BRANCH ?? process.env.GITHUB_REF_NAME;
  const prNumber = parseMaybeNumber(getArgValue(argv, '--pr-number') ?? process.env.PR_NUMBER);

  const baseRef = getArgValue(argv, '--base-ref') ?? process.env.BASE_REF;
  const baseSha = getArgValue(argv, '--base-sha') ?? process.env.BASE_SHA;

  const headRef = getArgValue(argv, '--head-ref') ?? process.env.HEAD_REF;
  const headSha = getArgValue(argv, '--head-sha') ?? process.env.HEAD_SHA ?? process.env.GITHUB_SHA;

  const runAt = getArgValue(argv, '--run-at') ?? new Date().toISOString();

  const hyperfineJsonPath = getArgValue(argv, '--hyperfine-json') ?? 'hyperfine.json';
  const criterionLogPath = getArgValue(argv, '--criterion-log') ?? 'criterion-compare.txt';

  const outCurrent = getArgValue(argv, '--out-current') ?? 'bench-current.json';
  const historyIn = getArgValue(argv, '--history-in');
  const historyOut = getArgValue(argv, '--history-out') ?? 'bench-history.json';

  const hyperfineJson = readJsonIfExists(hyperfineJsonPath);
  const hyperfineTimings = parseHyperfineJson(hyperfineJson);

  const criterionCompareLog = readTextIfExists(criterionLogPath);
  const criterionTimings = parseCriterionCompareLog(criterionCompareLog);

  const payload = {
    schema: 1,
    generated_at: runAt,
    repository: repo,
    pr_number: prNumber,
    branch,
    base: {
      ref: baseRef,
      sha: baseSha,
    },
    head: {
      ref: headRef,
      sha: headSha,
      url: repo && headSha ? `https://github.com/${repo}/commit/${headSha}` : undefined,
    },
    hyperfine: {
      results: hyperfineTimings,
    },
    criterion: {
      results: criterionTimings,
    },
  };

  fs.writeFileSync(outCurrent, JSON.stringify(payload, null, 2));

  // History handling.
  const historyPath = path.resolve(historyOut);
  const history = historyIn
    ? (readJsonIfExists(historyIn) ?? { schema: 1, runs: [] })
    : (readJsonIfExists(historyPath) ?? { schema: 1, runs: [] });

  if (!Array.isArray(history.runs)) history.runs = [];

  history.schema = 1;
  history.updated_at = runAt;
  const newRun = {
    run_at: runAt,
    branch,
    pr_number: prNumber,
    base: payload.base,
    head: payload.head,
    hyperfine: payload.hyperfine.results,
    criterion: payload.criterion.results,
  };

  // For PRs, keep a single entry per PR number (reruns update in-place rather than append).
  if (Number.isFinite(prNumber)) {
    history.runs = history.runs.filter((r) => normalizeRunPrNumber(r) !== prNumber);
  }

  history.runs.push(newRun);

  fs.writeFileSync(historyPath, JSON.stringify(history, null, 2));
}

main();
