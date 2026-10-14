#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Bidirectional text gate (#44): Arabic/Hebrew fixtures pass the visual
# threshold, and logical-order copy matches the TS oracle (job: bidi).
# Editing-surface checks (#57) will extend this script later.
#
# Definition of done for #44: this script no longer prints NOT IMPLEMENTED.
# Ask Jared (or the Chief of Staff's gh login) to add the `bidi` job to the
# branch-protection required checks. Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

# shellcheck source=versions.env
. scripts/ci/versions.env
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"

# Cap rustc parallelism on hosted runners: full-workspace release + wasm-pack
# LTO has OOMed / disconnected the bidi job (runner lost communication ~46m)
# while the suite itself stays green locally in a few minutes.
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"

# timeout(1) when available — apt under `playwright install --with-deps` can
# hang indefinitely on a contended dpkg lock, which never reaches the `||`
# browser-only fallback.
run_with_timeout() {
  local secs=$1
  shift
  if command -v timeout >/dev/null 2>&1; then
    timeout --signal=TERM --kill-after=30s "$secs" "$@"
  else
    "$@"
  fi
}

if ! command -v bun >/dev/null 2>&1; then
  curl -fsSL https://bun.sh/install | bash
  export PATH="$HOME/.bun/bin:$PATH"
fi

rustup toolchain install --no-self-update >/dev/null

# Pinned upstream submodule (TS oracle for copy + view-IR).
git submodule update --init --depth 1 upstream
export UPSTREAM_DIR="$ROOT/upstream"
if [ "$(git -C "$UPSTREAM_DIR" rev-parse HEAD)" != e66aaec24adc0b68472eae1fc42aa4598e09c352 ]; then
  echo "bidi: upstream not at pin e66aaec" >&2
  exit 1
fi
if [ ! -d "$UPSTREAM_DIR/node_modules" ]; then
  ( cd "$UPSTREAM_DIR" && bun install )
fi

echo "bidi: ensuring MIT Arabic/Hebrew fixtures"
python3 fixtures/bidi/generate.py

echo "bidi: building wasm (shell copy) + view-IR dump"
bash scripts/build-wasm.sh
( cd rs && cargo build --release -q -p nitro-xml-xlsx --example xlsx_view_ir_dump )
( cd rs && cargo build --release -q -p nitro-xml-pptx --example pptx_dump )

echo "bidi: installing visual host deps (Playwright from #45)"
( cd parity/visual && bun install )
# Prefer browser-only install first (fast, no apt). System deps only if needed.
if ! ( cd parity/visual && run_with_timeout 300 bunx playwright install chromium ); then
  echo "bidi: chromium install failed; retrying with --with-deps (10m cap)" >&2
  ( cd parity/visual && run_with_timeout 600 bunx playwright install --with-deps chromium ) \
    || ( cd parity/visual && run_with_timeout 300 bunx playwright install chromium )
fi

echo "bidi: running logical-order copy + visual checks"
set +e
run_with_timeout 1200 bash -c 'cd tests/bidi && bun ./run.mjs'
rc=$?
set -e
if [ "$rc" -eq 124 ]; then
  echo "bidi: FAIL (viewer checks timed out after 20m)" >&2
  exit 1
fi

if [ -n "${GITHUB_STEP_SUMMARY:-}" ] && [ -f tests/bidi/out/gate-summary.txt ]; then
  {
    echo "## Bidi gate (#44)"
    echo '```'
    cat tests/bidi/out/gate-summary.txt
    echo '```'
  } >> "$GITHUB_STEP_SUMMARY"
fi

if [ "$rc" -ne 0 ]; then
  echo "bidi: FAIL" >&2
  exit 1
fi

echo "bidi: running editing-surface checks (#57)"
set +e
run_with_timeout 600 bash -c 'cd tests/bidi/edit && bun ./run.mjs'
edit_rc=$?
set -e
if [ "$edit_rc" -eq 124 ]; then
  echo "bidi: edit FAIL (timed out after 10m)" >&2
  exit 1
fi

if [ -n "${GITHUB_STEP_SUMMARY:-}" ] && [ -f tests/bidi/edit/out/gate-summary.txt ]; then
  {
    echo "## Bidi edit gate (#57)"
    echo '```'
    cat tests/bidi/edit/out/gate-summary.txt
    echo '```'
  } >> "$GITHUB_STEP_SUMMARY"
fi

if [ "$edit_rc" -ne 0 ]; then
  echo "bidi: edit FAIL" >&2
  exit 1
fi
echo "bidi: PASS"
exit 0
