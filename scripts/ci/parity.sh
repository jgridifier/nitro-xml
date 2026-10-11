#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Viewer / foundation parity: TS oracle vs Rust (job: parity).
# Runs fixtures/verify --skip-on-demand, then bun parity/run --ci (xml + opc +
# xlsx worksheet (#16) + view-IR (#18) + docx styles/body (#20/#21) + pptx
# presentation model (#24) today; later LayoutResult flips ready when dumps
# land). Visual pixelmatch remains #45.
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
( cd rs && cargo build --release -q -p nitro-xml-opc --example xml_dump --example opc_dump )
( cd rs && cargo build --release -q -p nitro-xml-xlsx --example xlsx_dump --example xlsx_view_ir_dump )
( cd rs && cargo build --release -q -p nitro-xml-docx --example docx_styles_dump )
( cd rs && cargo build --release -q -p nitro-xml-pptx --example pptx_dump )
python3 tools/model-compare/gates/make_opc_fixtures.py >/dev/null

echo "parity: running harness (ready comparisons)"
set +e
bun parity/run --ci
parity_rc=$?
( cd parity && bun test )
test_rc=$?
set -e

if [ "$parity_rc" -ne 0 ] || [ "$test_rc" -ne 0 ]; then
  echo "parity: FAIL (harness=$parity_rc tests=$test_rc)" >&2
  exit 1
fi

msg="parity: PASS (xml + opc + xlsx worksheet + view-IR + docx styles/body + pptx presentation ready)"
echo "$msg"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  echo "$msg" >> "$GITHUB_STEP_SUMMARY"
  echo "" >> "$GITHUB_STEP_SUMMARY"
  echo "Please add the \`parity\` job to branch-protection required checks (Jared / Chief of Staff)." >> "$GITHUB_STEP_SUMMARY"
fi
