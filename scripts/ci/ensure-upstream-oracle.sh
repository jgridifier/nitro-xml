#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Shared setup for TS-oracle gates that import pinned upstream source.
# bun install alone is not enough: editor modules import the `ooxml-core`
# package name, whose package.json exports point at dist/. On a clean runner
# dist/ is absent, so resolution fails with "Cannot find package 'ooxml-core'".
# Always install the workspace, then build ooxml-core when dist/pptx is missing.
set -euo pipefail

# Prefer explicit ROOT arg; when sourced, BASH_SOURCE locates this file (not $0).
_SCRIPT="${BASH_SOURCE[0]:-$0}"
ROOT="${1:-$(cd "$(dirname "$_SCRIPT")/../.." && pwd)}"
# Always pin to this checkout's upstream — do not inherit a stale UPSTREAM_DIR.
UPSTREAM_DIR="$ROOT/upstream"
PIN=e66aaec24adc0b68472eae1fc42aa4598e09c352

git -C "$ROOT" submodule update --init --depth 1 upstream
export UPSTREAM_DIR
if [ "$(git -C "$UPSTREAM_DIR" rev-parse HEAD)" != "$PIN" ]; then
  echo "ensure-upstream-oracle: upstream not at pin e66aaec (got $(git -C "$UPSTREAM_DIR" rev-parse HEAD))" >&2
  exit 1
fi

echo "ensure-upstream-oracle: bun install at upstream workspace root ($UPSTREAM_DIR)"
( cd "$UPSTREAM_DIR" && bun install )

if [ ! -d "$UPSTREAM_DIR/src/core/dist/pptx" ]; then
  echo "ensure-upstream-oracle: building ooxml-core (package exports → dist/)"
  ( cd "$UPSTREAM_DIR" && bun run --cwd src/core build )
fi

# Self-link so `import 'ooxml-core/…'` from source under src/core resolves.
mkdir -p "$UPSTREAM_DIR/src/core/node_modules"
if [ ! -e "$UPSTREAM_DIR/src/core/node_modules/ooxml-core" ]; then
  ln -sfn . "$UPSTREAM_DIR/src/core/node_modules/ooxml-core"
fi

# Verify the package name resolves before gates run.
if ! ( cd "$UPSTREAM_DIR/src/core" && bun -e 'await import("ooxml-core/pptx")' >/dev/null 2>&1 ); then
  echo "ensure-upstream-oracle: ooxml-core/pptx still unresolved after install+build" >&2
  exit 1
fi

export UPSTREAM_DIR
echo "ensure-upstream-oracle: ready (ooxml-core/pptx resolves)"
