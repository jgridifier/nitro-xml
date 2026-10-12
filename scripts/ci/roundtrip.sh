#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Lossless save gate (a): open→save canonical package compare, validator error
# subset, LibreOffice reopen (job: roundtrip). Issues #49 + #48.
#
# Definition of done: this script no longer prints NOT IMPLEMENTED.
# Ask Jared (or the Chief of Staff's gh login) to keep `roundtrip` in the
# branch-protection required checks. Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

# shellcheck source=versions.env
. scripts/ci/versions.env
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"
export NITRO_TOOLS_DIR="${NITRO_TOOLS_DIR:-$HOME/.nitro-tools}"
export NITRO_REQUIRE_VALIDATORS=1

if ! command -v bun >/dev/null 2>&1; then
  curl -fsSL https://bun.sh/install | bash
  export PATH="$HOME/.bun/bin:$PATH"
fi

echo "roundtrip: installing validators (#48)"
bash scripts/ci/setup-validators.sh
export PATH="${NITRO_TOOLS_DIR}/dotnet:$PATH"
export DOTNET_ROOT="${NITRO_TOOLS_DIR}/dotnet"

# Pinned upstream submodule (TS oracle).
git submodule update --init --depth 1 upstream
export UPSTREAM_DIR="$ROOT/upstream"
if [ "$(git -C "$UPSTREAM_DIR" rev-parse HEAD)" != e66aaec24adc0b68472eae1fc42aa4598e09c352 ]; then
  echo "roundtrip: upstream not at pin e66aaec" >&2
  exit 1
fi
if [ ! -d "$UPSTREAM_DIR/src/core/node_modules" ] && [ ! -d "$UPSTREAM_DIR/node_modules" ]; then
  ( cd "$UPSTREAM_DIR" && bun install )
fi

echo "roundtrip: installing harness deps"
( cd parity/roundtrip && bun install )

echo "roundtrip: building Rust opc_noop_save + opc_touch_save (#65)"
( cd rs && cargo build --release -q -p nitro-xml-opc --example opc_noop_save --example opc_touch_save )

echo "roundtrip: C14N / rels / zip-cd unit tests"
( cd parity/roundtrip && bun test )

echo "roundtrip: TS-vs-TS + Rust identity-shortcut + Open XML SDK + LibreOffice (#49 / #65 / #48)"
set +e
( cd parity/roundtrip && bun ./run.mjs --ci --engine both )
rc_identity=$?
set -e

echo "roundtrip: Rust touch-rebuild proof + negative control (#65)"
set +e
( cd parity/roundtrip && bun ./run.mjs --ci --mode touch --negative-control )
rc_touch=$?
set -e

{
  echo "## Round-trip gate (#49 / #65 / #48)"
  if [ -f parity/roundtrip/out/gate-summary.txt ]; then
    echo '### Identity (shortcut documented)'
    echo '```'
    cat parity/roundtrip/out/gate-summary.txt
    echo '```'
  fi
  if [ -f parity/roundtrip/out/touch-gate-summary.txt ]; then
    echo '### Touch-rebuild (real PackageWriter path)'
    echo '```'
    cat parity/roundtrip/out/touch-gate-summary.txt
    echo '```'
  fi
} > /tmp/roundtrip-gate-combined.txt

if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  cat /tmp/roundtrip-gate-combined.txt >> "$GITHUB_STEP_SUMMARY"
fi
cat /tmp/roundtrip-gate-combined.txt

if [ "$rc_identity" -ne 0 ] || [ "$rc_touch" -ne 0 ]; then
  echo "roundtrip: FAIL (identity=$rc_identity touch=$rc_touch)" >&2
  exit 1
fi
echo "roundtrip: PASS"
exit 0
