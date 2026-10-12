#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Undo/redo parity gate (c): model snapshots after every do/undo/redo step
# against the TS oracle over edits/*.json (job: undo-parity). Issue #51.
#
# Definition of done for #51: this script no longer prints NOT IMPLEMENTED.
# Ask Jared (or the Chief of Staff's gh login) to add the `undo-parity` job to
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

# Depends on #50 edits corpus + edit apply helper.
if [ ! -d edits/xlsx ] || [ ! -f parity/edit/apply.mjs ]; then
  echo "undo-parity: edits/*.json / parity/edit missing (needs #50)" >&2
  exit 1
fi

# Pinned upstream + ooxml-core dist (required for `ooxml-core/pptx` package imports).
# shellcheck source=ensure-upstream-oracle.sh
. scripts/ci/ensure-upstream-oracle.sh "$ROOT"

echo "undo-parity: installing harness deps"
( cd parity/roundtrip && bun install )
( cd parity/edit && bun install )
( cd parity/undo && bun install )

echo "undo-parity: snapshot unit tests"
( cd parity/undo && bun test )

echo "undo-parity: do/undo-all/redo-all over edits/*.json (TS oracle)"
set +e
( cd parity/undo && bun ./run.mjs --ci )
rc=$?
set -e

echo "undo-parity: Rust xlsx checkpoints vs live TS dump (#64)"
set +e
( cd rs && cargo test -p nitro-xml-xlsx catches_xlsx_undo_parity_checkpoints_match_ts -- --nocapture )
rust_xlsx_rc=$?
set -e

echo "undo-parity: Rust docx text/format checkpoints vs live TS dump (#78)"
set +e
( cd rs && cargo test -p nitro-xml-docx catches_docx_undo_parity_checkpoints_match_ts -- --nocapture )
rust_docx_rc=$?
set -e

echo "undo-parity: Rust pptx checkpoints vs live TS dump (#84)"
set +e
( cd rs && cargo test -p nitro-xml-pptx catches_pptx_undo_parity_checkpoints_match_ts -- --nocapture )
rust_pptx_rc=$?
set -e

if [ -n "${GITHUB_STEP_SUMMARY:-}" ] && [ -f parity/undo/out/gate-summary.txt ]; then
  {
    echo "## Undo-parity gate (#51 / #64 / #78 / #84)"
    echo '```'
    cat parity/undo/out/gate-summary.txt
    echo "Rust xlsx vs TS: exit $rust_xlsx_rc (22/22 expected when green)"
    echo "Rust docx vs TS: exit $rust_docx_rc (22/22 text+format+table scripts)"
    echo "Rust pptx vs TS: exit $rust_pptx_rc (#84)"
    echo '```'
  } >> "$GITHUB_STEP_SUMMARY"
fi

if [ "$rc" -ne 0 ] || [ "$rust_xlsx_rc" -ne 0 ] || [ "$rust_docx_rc" -ne 0 ] || [ "$rust_pptx_rc" -ne 0 ]; then
  echo "undo-parity: FAIL (ts=$rc rust_xlsx=$rust_xlsx_rc rust_docx=$rust_docx_rc rust_pptx=$rust_pptx_rc)" >&2
  exit 1
fi
echo "undo-parity: PASS"
echo "Please add the \`undo-parity\` job to branch-protection required checks (Jared / Chief of Staff)."
exit 0
