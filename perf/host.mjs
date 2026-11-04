// SPDX-License-Identifier: MIT
// Host fingerprint (#10): who measured. Stored as `host` in every result row and in the summary JSON, so a
// number can always be traced to a machine class. Raw hostnames are never stored, only a short hash.
import { createHash } from 'node:crypto';
import { arch, cpus, hostname, platform, release, totalmem } from 'node:os';
import { readFileSync } from 'node:fs';

const short = (s) => createHash('sha256').update(s).digest('hex').slice(0, 10);

function osName() {
  if (platform() === 'linux') {
    try {
      const m = /^PRETTY_NAME="?([^"\n]+)"?/m.exec(readFileSync('/etc/os-release', 'utf8'));
      if (m) return `${m[1]} (linux ${release()})`;
    } catch {}
  }
  return `${platform()} ${release()}`;
}

/** kind "ci" when GitHub Actions (or CI=true) is set, else "box". id is stable across runs on one machine. */
export function hostFingerprint(chromeVersion, env = process.env) {
  const ci = env.GITHUB_ACTIONS === 'true' || env.CI === 'true';
  const c = cpus();
  return {
    kind: ci ? 'ci' : 'box',
    id: ci ? `ci-${env.RUNNER_NAME || env.RUNNER_OS || 'runner'}-${env.RUNNER_ARCH || arch()}` : `box-${short(hostname())}`,
    os: osName(),
    arch: arch(),
    cpu_model: c[0]?.model?.trim() ?? 'unknown',
    cores: c.length,
    memory_gb: Math.round(totalmem() / 2 ** 30),
    chrome_version: chromeVersion,
    node: process.version,
    concurrent_runs: Number(env.PERF_CONCURRENT_RUNS || 1), // runs sharing this machine at once (baseline-sweep.mjs --workers)
  };
}
