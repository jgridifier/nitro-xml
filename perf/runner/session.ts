// SPDX-License-Identifier: MIT
/**
 * Long-session correctness runner (#90). Test/bench only — never shipped.
 *
 * Each format runs exactly OPS (1000) attempted editor operations (edits + undo + redo),
 * then save, reopen sanity checks, and undo-parity snapshot verification on the
 * in-memory history stack (TS default limit 100).
 *
 * TODO(#90): memory growth gate vs TS editor (perf paused)
 * TODO(#90): session timing / keystroke latency (perf paused)
 */

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const OPS = 1000;

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, "../..");
const nativeDir = join(here, "native");
const manifestPath = join(nativeDir, "Cargo.toml");

function cargoBin(): string {
  return process.env.CARGO ?? "cargo";
}

function runHarness(): { ok: boolean; stdout: string; stderr: string; code: number } {
  if (!existsSync(manifestPath)) {
    throw new Error(`missing native harness: ${manifestPath}`);
  }
  const child = spawnSync(cargoBin(), ["run", "--quiet", "--release", "--manifest-path", manifestPath], {
    cwd: repoRoot,
    encoding: "utf8",
    env: process.env,
  });
  return {
    ok: child.status === 0,
    stdout: child.stdout ?? "",
    stderr: child.stderr ?? "",
    code: child.status ?? 1,
  };
}

export function main(): number {
  console.error(`long-session: building native harness (OPS=${OPS})…`);
  const result = runHarness();
  if (result.stdout.trim()) {
    process.stdout.write(result.stdout);
    if (!result.stdout.endsWith("\n")) {
      process.stdout.write("\n");
    }
  }
  if (result.stderr.trim()) {
    process.stderr.write(result.stderr);
    if (!result.stderr.endsWith("\n")) {
      process.stderr.write("\n");
    }
  }
  return result.ok ? 0 : result.code;
}

if (import.meta.main) {
  process.exit(main());
}
