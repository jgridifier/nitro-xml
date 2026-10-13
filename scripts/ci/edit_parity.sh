#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Edit round-trip gate (b): edits/*.json on the TS reference — apply+save
# deterministic, part-level expected diffs recorded (job: edit-parity). Issue #50.
#
# Definition of done for #50: this script no longer prints NOT IMPLEMENTED.
# Ask Jared (or the Chief of Staff's gh login) to add the `edit-parity` job to
# the branch-protection required checks. Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

# shellcheck source=versions.env
. scripts/ci/versions.env
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"
export NITRO_TOOLS_DIR="${NITRO_TOOLS_DIR:-$HOME/.nitro-tools}"

if ! command -v bun >/dev/null 2>&1; then
  curl -fsSL https://bun.sh/install | bash
  export PATH="$HOME/.bun/bin:$PATH"
fi

# Validators (#48): soft-skip inside the harness when tools are absent.
bash scripts/ci/setup-validators.sh || true

# Pinned upstream + ooxml-core dist (required for `ooxml-core/pptx` package imports).
# shellcheck source=ensure-upstream-oracle.sh
. scripts/ci/ensure-upstream-oracle.sh "$ROOT"

echo "edit-parity: installing harness deps"
( cd parity/roundtrip && bun install )
( cd parity/edit && bun install )

echo "edit-parity: schema / corpus unit tests"
( cd parity/edit && bun test )

echo "edit-parity: formula-shift negative control (#76)"
( cd rs && cargo test -p nitro-xml-xlsx --lib catches_unshifted_formula_after_row_insert -- --nocapture )

echo "edit-parity: docx writer example"
( cd rs && cargo build --release -p nitro-xml-docx --example docx_save )

echo "edit-parity: xlsx writer example"
( cd rs && cargo build --release -p nitro-xml-xlsx --example xlsx_save )

echo "edit-parity: pptx writer example"
( cd rs && cargo build --release -p nitro-xml-pptx --example pptx_save )

echo "edit-parity: TS-vs-TS apply+save over edits/*.json, plus Rust docx, xlsx, and pptx compare"
set +e
( cd parity/edit && bun ./run.mjs --ci --rust-docx --rust-xlsx --rust-pptx --negative-control )
rc=$?
set -e

if [ -n "${GITHUB_STEP_SUMMARY:-}" ] && [ -f parity/edit/out/gate-summary.txt ]; then
  {
    echo "## Edit-parity gate (#50)"
    echo '```'
    cat parity/edit/out/gate-summary.txt
    echo '```'
  } >> "$GITHUB_STEP_SUMMARY"
fi

if [ "$rc" -ne 0 ]; then
  echo "edit-parity: FAIL" >&2
  exit 1
fi

echo "edit-parity: post-edit layout IR (incremental vs TS) (#81)"
( cd rs && cargo build --release -q -p nitro-xml-docx --example docx_edit_layout_dump )
set +e
( cd parity/edit && bun ./docx_layout_after_edit.mjs --ci )
layout_rc=$?
set -e
if [ -n "${GITHUB_STEP_SUMMARY:-}" ] && [ -f parity/edit/out/layout-after-edit-summary.txt ]; then
  {
    echo "## Post-edit layout IR (#81)"
    echo '```'
    cat parity/edit/out/layout-after-edit-summary.txt
    echo '```'
  } >> "$GITHUB_STEP_SUMMARY"
fi
if [ "$layout_rc" -ne 0 ]; then
  echo "edit-parity: post-edit layout FAIL" >&2
  exit 1
fi

echo "edit-parity: PASS"
echo "Please add the \`edit-parity\` job to branch-protection required checks (Jared / Chief of Staff)."
exit 0
