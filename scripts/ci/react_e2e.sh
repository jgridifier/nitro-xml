#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# React embed save gate (E1/#30): Playwright e2e for examples/vite-react
# (edit-save.spec.js, all 3 formats), then validates saved bytes through the
# Open XML SDK validator and LibreOffice headless.
#
# Called from roundtrip.sh so the roundtrip job covers it.
# Ask Jared (or the Chief of Staff's gh login) to keep roundtrip in the
# branch-protection required checks — no new job entry needed.
# Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

# shellcheck source=versions.env
. scripts/ci/versions.env
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"
export NITRO_TOOLS_DIR="${NITRO_TOOLS_DIR:-$HOME/.nitro-tools}"

# Hard-fail if validators are missing; this gate is not optional in CI.
export NITRO_REQUIRE_VALIDATORS=1
if [ ! -f "${NITRO_TOOLS_DIR}/.validators-stamp" ]; then
  echo "react-e2e: validators not installed — run setup-validators.sh first" >&2
  exit 1
fi
TOOLS_STAMP="$(cat "${NITRO_TOOLS_DIR}/.validators-stamp")"
EXPECTED_STAMP="${DOTNET_SDK_VERSION}|${OPENXML_SDK_VERSION}|${LIBREOFFICE_APT_PACKAGE}"
if [ "$TOOLS_STAMP" != "$EXPECTED_STAMP" ]; then
  echo "react-e2e: validator stamp mismatch (got '$TOOLS_STAMP'; want '$EXPECTED_STAMP')" >&2
  echo "react-e2e: re-run setup-validators.sh" >&2
  exit 1
fi

if ! command -v node >/dev/null 2>&1; then
  echo "react-e2e: node not found" >&2
  exit 1
fi

# --- Build WASM (required for the vite-react dev server). ---
echo "react-e2e: building WASM"
bash scripts/build-wasm.sh

# --- Install @nitro-xml/react dist. ---
echo "react-e2e: building @nitro-xml/react"
( cd packages/nitro-xml-react && npm install --no-fund --no-audit && npm run build )

# --- Prepare fixtures for the vite dev server. ---
EXAMPLE="$ROOT/examples/vite-react"
mkdir -p "$EXAMPLE/public/fixtures"
cp fixtures/generated/xlsx/cells_1k.xlsx  "$EXAMPLE/public/fixtures/"
cp fixtures/generated/docx/pages_10.docx  "$EXAMPLE/public/fixtures/"
cp fixtures/generated/pptx/slides_10.pptx "$EXAMPLE/public/fixtures/"

# --- Install example deps + Playwright chromium. ---
echo "react-e2e: installing example deps"
( cd "$EXAMPLE" && npm install --no-fund --no-audit )
echo "react-e2e: installing Playwright chromium"
( cd "$EXAMPLE" && npm run test:e2e:install )

# --- Run Playwright (write saved bytes to SAVE_OUT). ---
SAVE_OUT="$ROOT/out/react-e2e-saved"
mkdir -p "$SAVE_OUT"
export NITRO_REACT_E2E_OUT="$SAVE_OUT"

echo "react-e2e: running Playwright e2e"
set +e
( cd "$EXAMPLE" && npm run test:e2e -- --reporter=line )
pw_rc=$?
set -e

if [ "$pw_rc" -ne 0 ]; then
  echo "react-e2e: Playwright FAILED (exit $pw_rc)" >&2
fi

# --- Validate saved files with Open XML SDK and LibreOffice. ---
overall_rc=0

validate_file() {
  local output="$1" input="$2" ext="$3" label="$4"
  # PRODUCT_SPEC §5(a): no new validator errors, plus equal reopened unit counts.
  # Reuse the corpus gate's comparisons; do not hide pre-existing input errors.
  if ! node --input-type=module - "$input" "$output" "$ext" "$label" <<'JS'
import { readFileSync } from 'node:fs';
import { validateOpenXml, errorSubset, libreOfficeReopen } from './parity/roundtrip/validate.mjs';
const [input, output, ext, label] = process.argv.slice(2);
const inputBytes = readFileSync(input);
const outputBytes = readFileSync(output);
const before = validateOpenXml(input);
const after = validateOpenXml(output);
for (const result of [before, after]) {
  if (result.skipped || result.errors?.some(e =>
    ['validator-fail', 'validator-non-json', 'package-open'].includes(e.id))) {
    throw new Error(`react-e2e: validator unavailable or package unreadable: ${JSON.stringify(result)}`);
  }
}
const validation = errorSubset(before.errors, after.errors);
console.log(`react-e2e: ${label}: input errors=${before.errors.length}, output errors=${after.errors.length}, new errors=${validation.extras.length}`);
if (!validation.ok) throw new Error(`New Open XML errors: ${JSON.stringify(validation.extras)}`);
const reopened = libreOfficeReopen(inputBytes, outputBytes, ext);
console.log(`react-e2e: LibreOffice ${label}: ${JSON.stringify(reopened)}`);
if (reopened.skipped || !reopened.ok) throw new Error('LibreOffice reopen/count comparison failed');
JS
  then
    overall_rc=1
  fi
}

validate_file "$SAVE_OUT/saved_edit.xlsx" "$EXAMPLE/public/fixtures/cells_1k.xlsx" xlsx "edit-save xlsx"
validate_file "$SAVE_OUT/saved_edit.docx" "$EXAMPLE/public/fixtures/pages_10.docx" docx "edit-save docx"
validate_file "$SAVE_OUT/saved_edit.pptx" "$EXAMPLE/public/fixtures/slides_10.pptx" pptx "edit-save pptx"
validate_file "$SAVE_OUT/saved_noop.xlsx" "$EXAMPLE/public/fixtures/cells_1k.xlsx" xlsx "noop-save xlsx"

# --- Report to GitHub step summary. ---
{
  echo "## React e2e save gate (E1/#30)"
  echo "- Playwright: $([ "$pw_rc" -eq 0 ] && echo 'PASS' || echo "FAIL (exit $pw_rc)")"
  echo "- Validator: $([ "$overall_rc" -eq 0 ] && echo 'PASS' || echo 'FAIL (see above)')"
  echo "- Saved artefacts: \`$SAVE_OUT\`"
} | tee /tmp/react-e2e-summary.txt

if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  cat /tmp/react-e2e-summary.txt >> "$GITHUB_STEP_SUMMARY"
fi

if [ "$pw_rc" -ne 0 ] || [ "$overall_rc" -ne 0 ]; then
  echo "react-e2e: FAIL (playwright=$pw_rc validator=$overall_rc)" >&2
  exit 1
fi
echo "react-e2e: PASS"
exit 0
