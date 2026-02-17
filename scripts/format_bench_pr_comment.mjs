#!/usr/bin/env node
/*
Builds a single PR comment markdown from benchmark artifacts.

It consolidates Criterion output into 3 sections (Regressions/Improvements/Unchanged)
that each include all groups.

Inputs:
- hyperfine markdown file (optional)
- criterion regressions output files from ixa-bench's `check_criterion_regressions`

Usage:
  node scripts/format_bench_pr_comment.mjs \
    --out results.md \
    --hyperfine-md hyperfine.md \
    --criterion-dir artifacts/raw \
    --groups indexing,large_dataset,sampling,counts,algorithms,sample_entity_scaling,examples
*/

import fs from 'node:fs';
import path from 'node:path';
import { parseArgs } from 'node:util';
import { pathToFileURL } from 'node:url';

const SECTION_TITLES = ['Regressions', 'Improvements', 'Unchanged'];

function readTextIfExists(filePath) {
  if (!filePath) return '';
  if (!fs.existsSync(filePath)) return '';
  return fs.readFileSync(filePath, 'utf8');
}

function isSafeGroupName(group) {
  return /^[A-Za-z0-9_-]+$/.test(group);
}

function safeJoinWithin(rootDir, fileName) {
  const root = path.resolve(rootDir);
  const full = path.resolve(root, fileName);
  const rel = path.relative(root, full);
  if (rel.startsWith('..') || path.isAbsolute(rel)) {
    throw new Error(`Resolved path escapes criterion dir: ${fileName}`);
  }
  return full;
}

function maxBacktickRun(text) {
  const matches = String(text || '').match(/`+/g);
  if (!matches) return 0;
  return matches.reduce((max, s) => Math.max(max, s.length), 0);
}

function fencedBlock(text) {
  const body = String(text || '').trimEnd();
  const fence = '`'.repeat(Math.max(3, maxBacktickRun(body) + 1));
  return [fence, body || '(none)', fence].join('\n');
}

function extractNamedSection(text, title) {
  const lines = String(text || '').split(/\r?\n/);
  const headerRe = new RegExp(`^${title}:`);
  const anyHeaderRe = /^(Regressions|Improvements|Unchanged):/;

  let start = -1;
  for (let i = 0; i < lines.length; i += 1) {
    if (headerRe.test(lines[i])) {
      start = i;
      break;
    }
  }
  if (start === -1) return null;

  const out = [];
  for (let i = start; i < lines.length; i += 1) {
    if (i !== start && anyHeaderRe.test(lines[i])) break;
    out.push(lines[i]);
  }
  // Trim trailing blank lines
  while (out.length > 0 && out[out.length - 1].trim() === '') out.pop();
  return out.join('\n');
}

function extractNamedSectionBody(text, title) {
  const section = extractNamedSection(text, title);
  if (section == null) return null;
  const lines = section.split(/\r?\n/);
  // Drop the leading "<Title>:" line if present so we can concatenate groups.
  if (lines.length > 0 && lines[0].trim() === `${title}:`) {
    lines.shift();
  }
  // Trim leading/trailing blank lines after dropping the header.
  while (lines.length > 0 && lines[0].trim() === '') lines.shift();
  while (lines.length > 0 && lines[lines.length - 1].trim() === '') lines.pop();
  return lines.join('\n');
}

function buildMarkdown({ hyperfineMd, criterionDir, groups }) {
  const bySection = {
    Regressions: [],
    Improvements: [],
    Unchanged: [],
  };

  for (const group of groups) {
    if (!isSafeGroupName(group)) {
      throw new Error(
        `Invalid group name "${group}". Allowed pattern: [A-Za-z0-9_-]+`,
      );
    }
    const p = safeJoinWithin(criterionDir, `criterion-regressions-${group}.txt`);
    const raw = readTextIfExists(p).trimEnd();

    const extracted = Object.fromEntries(
      SECTION_TITLES.map((t) => [t, extractNamedSectionBody(raw, t)]),
    );

    const hasStructured = SECTION_TITLES.some((t) => extracted[t] != null);

    for (const t of SECTION_TITLES) {
      let body = extracted[t];
      if (body == null) {
        if (!raw) body = '(no output)';
        else if (!hasStructured) body = t === 'Unchanged' ? raw : '(no structured output)';
        else body = '(none)';
      }

      bySection[t].push({ group, body });
    }
  }

  const lines = [];
  lines.push('### Benchmark Results', '');

  lines.push('#### Hyperfine', '');
  if (String(hyperfineMd || '').trim()) {
    lines.push(fencedBlock(String(hyperfineMd)), '');
  } else {
    lines.push('_Hyperfine output missing._', '');
  }

  lines.push('#### Criterion', '');

  for (const title of SECTION_TITLES) {
    lines.push(`##### ${title}`, '');
    const content = bySection[title]
      .map((entry) => String(entry.body || '').trimEnd() || '(none)')
      .join('\n');
    lines.push(fencedBlock(content));
    lines.push('');
  }

  while (lines.length > 0 && lines[lines.length - 1] === '') lines.pop();
  return lines.join('\n') + '\n';
}

function build() {
  const { values } = parseArgs({
    options: {
      out: { type: 'string' },
      'hyperfine-md': { type: 'string', default: 'hyperfine.md' },
      'criterion-dir': { type: 'string', default: 'artifacts/raw' },
      groups: { type: 'string' },
    },
  });

  const outPath = values.out || 'results.md';
  const hyperfineMdPath = values['hyperfine-md'] || 'hyperfine.md';
  const criterionDir = values['criterion-dir'] || 'artifacts/raw';

  const groups = String(values.groups || '')
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);

  if (groups.length === 0) {
    throw new Error('Missing --groups (comma-separated)');
  }

  const hyperfineMd = readTextIfExists(hyperfineMdPath);

  const md = buildMarkdown({ hyperfineMd, criterionDir, groups });
  fs.writeFileSync(outPath, md, 'utf8');
}

function isMainModule() {
  const argvPath = process.argv[1];
  if (!argvPath) return false;
  return import.meta.url === pathToFileURL(argvPath).href;
}

if (isMainModule()) {
  try {
    build();
  } catch (e) {
    const msg = e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e);
    process.stderr.write(`format_bench_pr_comment.mjs: ${msg}\n`);
    process.exitCode = 1;
  }
}
