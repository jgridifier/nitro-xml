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

VALIDATE_OUT="${NITRO_TOOLS_DIR}/ooxml-validate"
export DOTNET_ROOT="${NITRO_TOOLS_DIR}/dotnet"
export PATH="${DOTNET_ROOT}:$PATH"

if [ -x "$VALIDATE_OUT/ooxml-validate" ]; then
  VALIDATE_CMD=("$VALIDATE_OUT/ooxml-validate")
else
  VALIDATE_CMD=(dotnet "$VALIDATE_OUT/ooxml-validate.dll")
fi

SOFFICE=$(command -v soffice || command -v libreoffice || true)
if [ -z "$SOFFICE" ]; then
  echo "react-e2e: soffice/libreoffice not on PATH after setup-validators.sh" >&2
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
  local path="$1"
  local ext="$2"
  local label="$3"

  if [ ! -f "$path" ]; then
    echo "react-e2e: FAIL $label output not found at $path" >&2
    overall_rc=1
    return
  fi
  echo "react-e2e: ooxml-validate $label"
  set +e
  json=$("${VALIDATE_CMD[@]}" "$path" 2>/dev/null)
  v_rc=$?
  set -e
  if [ "$v_rc" -ne 0 ] && [ -z "$json" ]; then
    echo "react-e2e: FAIL ooxml-validate exited $v_rc for $label" >&2
    overall_rc=1
    return
  fi
  err_count=$(python3 -c 'import json,sys; a=json.loads(sys.argv[1]); print(len(a))' "$json" 2>/dev/null || echo "-1")
  if [ "$err_count" = "-1" ]; then
    echo "react-e2e: FAIL ooxml-validate returned non-JSON for $label" >&2
    overall_rc=1
  elif [ "$err_count" -gt 0 ]; then
    echo "react-e2e: FAIL ooxml-validate found $err_count error(s) in $label: $json" >&2
    overall_rc=1
  else
    echo "react-e2e: ooxml-validate OK (0 errors) for $label"
  fi

  echo "react-e2e: LibreOffice $label"
  tmp_dir=$(mktemp -d)
  trap 'rm -rf "$tmp_dir"' EXIT
  set +e
  "$SOFFICE" --headless --norestore --convert-to pdf --outdir "$tmp_dir" "$path" >/dev/null 2>&1
  lo_rc=$?
  set -e
  pdf_count=$(find "$tmp_dir" -name "*.pdf" | wc -l)
  rm -rf "$tmp_dir"
  trap - EXIT
  if [ "$lo_rc" -ne 0 ] || [ "$pdf_count" -eq 0 ]; then
    echo "react-e2e: FAIL LibreOffice conversion failed (exit=$lo_rc pdf=$pdf_count) for $label" >&2
    overall_rc=1
  else
    echo "react-e2e: LibreOffice OK (pdf produced) for $label"
  fi
}

# xlsx converts to pdf via LibreOffice as well.
validate_file "$SAVE_OUT/saved_edit.xlsx"  "xlsx" "edit-save xlsx"
validate_file "$SAVE_OUT/saved_edit.docx"  "docx" "edit-save docx"
validate_file "$SAVE_OUT/saved_edit.pptx"  "pptx" "edit-save pptx"
validate_file "$SAVE_OUT/saved_noop.xlsx"  "xlsx" "noop-save xlsx"

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
