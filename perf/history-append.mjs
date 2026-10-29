#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * Append run results to perf/history.jsonl (#8). Only main-branch CI runs (and seeding) call this; PR runs never do.
 *
 *   node perf/history-append.mjs --from results.jsonl [--commit sha] [--branch main] [--runner ci|local]
 *                                [--host-id name] [--ci-job id] [--history perf/history.jsonl] [--reseed]
 *   Lines of type "override" (written by gate.mjs) are appended verbatim.
 *
 * Reseed (one line): node perf/history-append.mjs --reseed --from perf/baselines/ts-<sha>.jsonl
 *   drops every previously seeded entry, then seeds from that file. The commit comes from the file name.
 */
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { cpus, hostname, platform, release } from 'node:os';
import { execSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { basename, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { toHistoryEntries, readJsonl } from './aggregate.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '..');

/** Best-effort host fingerprint; chrome_version is filled per entry from the run rows. */
export function hostFingerprint({ kind, id } = {}) {
  const ci = !!process.env.GITHUB_ACTIONS;
  return {
    kind: kind ?? (ci ? 'ci' : 'box'),
    id: id ?? (ci ? process.env.RUNNER_NAME ?? 'github-hosted' : hostname()),
    os: `${platform()} ${release()}`,
    cpu: cpus()[0]?.model ?? 'unknown',
    cores: cpus().length,
  };
}

function fixtureShas() {
  const out = {};
  const fx = JSON.parse(readFileSync(join(here, 'fixtures.json'), 'utf8')).fixtures;
  for (const f of fx) {
    const p = join(repo, f.path);
    if (existsSync(p)) out[f.id] = createHash('sha256').update(readFileSync(p)).digest('hex');
  }
  return out;
}

const gitDate = (sha) => {
  try { return execSync(`git -C ${repo} show -s --format=%cI ${sha}`, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim(); } catch { return new Date().toISOString(); }
};

export function parseArgs(argv) {
  const o = {};
  for (let i = 0; i < argv.length; i++) {
    const m = /^--([^=]+)(?:=(.*))?$/.exec(argv[i]);
    if (!m) throw new Error(`unexpected argument: ${argv[i]}`);
    o[m[1]] = m[2] ?? (argv[i + 1] && !argv[i + 1].startsWith('--') ? argv[++i] : true);
  }
  return o;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const a = parseArgs(process.argv.slice(2));
  if (!a.from) throw new Error('usage: node perf/history-append.mjs --from results.jsonl [--reseed] (see header)');
  const histFile = a.history ?? join(here, 'history.jsonl');
  let hist = existsSync(histFile) ? readFileSync(histFile, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)) : [];
  const seeding = !!a.reseed || /^ts-[0-9a-f]+\.jsonl$/.test(basename(a.from));
  if (a.reseed) hist = hist.filter((e) => !e.seeded);
  const lines = readJsonl(a.from);
  const overrides = lines.filter((l) => l.type === 'override');
  const rows = lines.filter((l) => !l.type);
  const commit = a.commit ?? (seeding ? /^ts-([0-9a-f]+)\.jsonl$/.exec(basename(a.from))?.[1] : null) ?? execSync('git rev-parse HEAD', { encoding: 'utf8' }).trim();
  const entries = toHistoryEntries(rows, {
    commit,
    date: gitDate(commit),
    branch: a.branch ?? (seeding ? 'main' : execSync('git rev-parse --abbrev-ref HEAD', { encoding: 'utf8' }).trim()),
    runner: a.runner ?? (seeding ? 'local' : process.env.GITHUB_ACTIONS ? 'ci' : 'local'),
    host: hostFingerprint({ id: a['host-id'] }),
    ci_job: a['ci-job'] ?? process.env.GITHUB_RUN_ID,
    fixture_sha256: fixtureShas(),
    seeded: seeding,
  });
  const seen = new Set(hist.map((e) => `${e.commit}|${e.engine}|${e.fixture_id}|${e.mode}|${e.cpu_throttle}|${e.host?.id}`));
  let added = 0;
  for (const e of entries) {
    if (seen.has(`${e.commit}|${e.engine}|${e.fixture_id}|${e.mode}|${e.cpu_throttle}|${e.host?.id}`)) continue;
    hist.push(e);
    added++;
  }
  hist.push(...overrides);
  writeFileSync(histFile, hist.map((e) => JSON.stringify(e)).join('\n') + '\n');
  console.log(`history-append: +${added} entries, +${overrides.length} override record(s) -> ${histFile}`);
}
