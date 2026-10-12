#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Viewer / foundation parity: TS oracle vs Rust (job: parity).
# Runs fixtures/verify --skip-on-demand, then bun parity/run --ci (xml + opc +
# xlsx worksheet (#16) + view-IR (#18) + engine API (#19) + shell (#29) +
# docx styles/body (#20/#21) + layout + page-count (#22) + pptx presentation model (#24) +
# chart model/S1 SVG (#28)), then visual pixelmatch (#45).
#
# Definition of done for #5: this script no longer prints NOT IMPLEMENTED.
# Ask Jared (or the Chief of Staff's gh login) to add the `parity` job to the
# branch-protection required checks. Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

# shellcheck source=versions.env
. scripts/ci/versions.env
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"

if ! command -v bun >/dev/null 2>&1; then
  curl -fsSL https://bun.sh/install | bash
  export PATH="$HOME/.bun/bin:$PATH"
fi

rustup toolchain install --no-self-update >/dev/null

# Pinned upstream submodule (TS oracle).
git submodule update --init --depth 1 upstream
export UPSTREAM_DIR="$ROOT/upstream"
if [ "$(git -C "$UPSTREAM_DIR" rev-parse HEAD)" != e66aaec24adc0b68472eae1fc42aa4598e09c352 ]; then
  echo "parity: upstream not at pin e66aaec" >&2
  exit 1
fi
if [ ! -d "$UPSTREAM_DIR/node_modules" ]; then
  ( cd "$UPSTREAM_DIR" && bun install )
fi

echo "parity: verifying fixture corpus (skip on-demand L/XL)"
python3 fixtures/verify --skip-on-demand

echo "parity: building Rust dump examples"
( cd rs && cargo build --release -q -p nitro-xml-opc --example xml_dump --example opc_dump --example chart_dump )
( cd rs && cargo build --release -q -p nitro-xml-render --example chart_render_svg )
( cd rs && cargo build --release -q -p nitro-xml-xlsx --example xlsx_dump --example xlsx_view_ir_dump --example xlsx_engine_dump --example xlsx_shell_dump )
( cd rs && cargo build --release -q -p nitro-xml-docx --example docx_styles_dump --example docx_layout_dump --example docx_pagecount_dump )

echo "parity: chromium for docx page-count (#22)"
( cd parity/visual && bun install )
( cd parity/visual && bunx playwright install --with-deps chromium ) \
  || ( cd parity/visual && bunx playwright install chromium )
( cd rs && cargo build --release -q -p nitro-xml-pptx --example pptx_dump )
python3 tools/model-compare/gates/make_opc_fixtures.py >/dev/null

echo "parity: running harness (ready comparisons)"
set +e
bun parity/run --ci
parity_rc=$?
# Root-level parity unit tests only (roundtrip/ has its own package; see roundtrip.sh).
( cd parity && bun test diff.test.ts normalize.test.ts )
test_rc=$?
set -e

if [ "$parity_rc" -ne 0 ] || [ "$test_rc" -ne 0 ]; then
  echo "parity: FAIL (harness=$parity_rc tests=$test_rc)" >&2
  exit 1
fi

echo "parity: visual pixelmatch (#45)"
# Always materialise artifact paths for the workflow upload-artifact step (#148),
# even if the visual runner crashes before writing its own summary/diffs.
mkdir -p parity/visual/out/diffs
printf '%s\n' 'visual: not run yet' > parity/visual/out/summary.txt

# Wasm package for <nitro-xlsx> visual cases (loader hosts the built pkg/).
bash scripts/build-wasm.sh
( cd parity/visual && bun install )
# System deps when the runner allows; fall back to browser-only install.
( cd parity/visual && bunx playwright install --with-deps chromium ) \
  || ( cd parity/visual && bunx playwright install chromium )
set +e
( cd parity/visual && bun test && bun ./run.mjs )
visual_rc=$?
set -e

# Guarantee summary.txt exists for artifact upload after any failure mode.
if [ ! -f parity/visual/out/summary.txt ] || ! grep -q '^visual:' parity/visual/out/summary.txt 2>/dev/null; then
  printf '%s\n' "visual: FAIL (runner exited ${visual_rc}; no summary written)" > parity/visual/out/summary.txt
fi

if [ "$visual_rc" -ne 0 ]; then
  echo "parity: FAIL (visual=$visual_rc)" >&2
  if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
    {
      echo "## Visual gate (#45) FAILED"
      echo '```'
      cat parity/visual/out/summary.txt
      echo '```'
      echo "Diff PNGs under \`parity/visual/out/diffs/\` (CI artifact \`visual-parity-diffs\`)."
    } >> "$GITHUB_STEP_SUMMARY"
  fi
  exit 1
fi

echo "parity: formula parser differential (#71)"
bash scripts/ci/formula_parity.sh

msg="parity: PASS (xml + opc + xlsx worksheet/view-IR/engine/shell + formula parser (#71) + docx styles/body/layout/page-count + pptx + chart model/S1 SVG + visual DPR1/2/print)"
echo "$msg"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  echo "$msg" >> "$GITHUB_STEP_SUMMARY"
  echo "" >> "$GITHUB_STEP_SUMMARY"
  echo "## Visual gate (#45)" >> "$GITHUB_STEP_SUMMARY"
  echo '```' >> "$GITHUB_STEP_SUMMARY"
  cat parity/visual/out/summary.txt >> "$GITHUB_STEP_SUMMARY"
  echo '```' >> "$GITHUB_STEP_SUMMARY"
fi
