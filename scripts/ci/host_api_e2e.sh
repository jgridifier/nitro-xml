#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Host-API browser e2e (job: host-api-e2e): build wasm if needed, assemble _site
# the way the host-api Playwright scripts expect, install Playwright Chromium,
# and run every scripts/e2e/host-api/*.py. Any failing check, or zero scripts,
# fails the job. Browser checks must fail, never warn-and-pass.
#
# Optional: HOST_API_PYTHON (venv interpreter that already has Playwright),
# HOST_API_CHROME (chromium executable; scripts fall back to Playwright's
# bundled browser when the box-local path is absent, so ubuntu-latest works).
#
# Ask Jared (or the Chief of Staff's gh login) to add the `host-api-e2e` job
# to ci.yml after this script lands. Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

# shellcheck source=versions.env
. scripts/ci/versions.env
export PATH="$HOME/.cargo/bin:$PATH"

PY="${HOST_API_PYTHON:-python3}"
if ! command -v "$PY" >/dev/null 2>&1 && [[ "$PY" != /* ]]; then
  PY=$(command -v python3)
fi
if ! command -v "$PY" >/dev/null 2>&1 && [[ ! -x "$PY" ]]; then
  echo "host-api-e2e: python not found (set HOST_API_PYTHON or install python3)" >&2
  exit 1
fi

OUT="$ROOT/scripts/e2e/host-api/out"
mkdir -p "$OUT"

need_wasm=0
if [[ ! -f "$ROOT/packages/nitro-xml/pkg/nitro_xml_wasm_bg.wasm" ]]; then
  need_wasm=1
fi

if [[ "$need_wasm" -eq 1 ]]; then
  if ! command -v rustup >/dev/null 2>&1; then
    echo "host-api-e2e: rustup missing (needed for wasm build)" >&2
    exit 1
  fi
  rustup toolchain install --no-self-update >/dev/null
  echo "host-api-e2e: building wasm"
  bash scripts/build-wasm.sh
else
  echo "host-api-e2e: reusing existing packages/nitro-xml/pkg"
fi

echo "host-api-e2e: assembling _site"
rm -rf "$ROOT/_site"
mkdir -p "$ROOT/_site/demo/nitro-xml/loader" "$ROOT/_site/demo/nitro-xml/pkg"
cp -R "$ROOT/demo/." "$ROOT/_site/demo/"
cp -R "$ROOT/packages/nitro-xml/loader/." "$ROOT/_site/demo/nitro-xml/loader/"
rm -rf "$ROOT/_site/demo/nitro-xml/loader/test"
cp -R "$ROOT/packages/nitro-xml/pkg/." "$ROOT/_site/demo/nitro-xml/pkg/"

# Reuse an existing venv when HOST_API_PYTHON is set; otherwise install into
# the current interpreter (CI: setup-python 3.12 on ubuntu-latest).
if ! "$PY" -c 'import playwright' >/dev/null 2>&1; then
  echo "host-api-e2e: installing Playwright package"
  "$PY" -m pip install --disable-pip-version-check playwright
fi
echo "host-api-e2e: installing Playwright Chromium"
"$PY" -m playwright install --with-deps chromium \
  || "$PY" -m playwright install chromium

# Prefer an explicit chrome, else a box-local headless shell if present, else
# Playwright's installed chromium (CI / ubuntu-latest).
if [[ -z "${HOST_API_CHROME:-}" ]]; then
  box_chrome="/home/box/.cache/ms-playwright/chromium_headless_shell-1254/chrome-headless-shell-linux64/chrome-headless-shell"
  if [[ -x "$box_chrome" ]]; then
    export HOST_API_CHROME="$box_chrome"
  else
    HOST_API_CHROME=$("$PY" - <<'PY'
from playwright.sync_api import sync_playwright
with sync_playwright() as p:
    print(p.chromium.executable_path)
PY
    ) || true
    if [[ -n "${HOST_API_CHROME:-}" && -x "${HOST_API_CHROME}" ]]; then
      export HOST_API_CHROME
    else
      unset HOST_API_CHROME || true
    fi
  fi
fi

shopt -s nullglob
scripts=("$ROOT"/scripts/e2e/host-api/*.py)
if [[ ${#scripts[@]} -eq 0 ]]; then
  echo "host-api-e2e: no scripts found under scripts/e2e/host-api/*.py" >&2
  if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    echo "## Host-API e2e" >> "$GITHUB_STEP_SUMMARY"
    echo "FAIL: no scripts found under \`scripts/e2e/host-api/*.py\`." >> "$GITHUB_STEP_SUMMARY"
  fi
  exit 1
fi

# Default ports already differ (selection_dirty=7892, fonts_focus=7893;
# others may bind an ephemeral port). Pass unique ports when a script accepts
# a positional port so sequential runs cannot collide with a leftover server.
failed=()
passed=()
i=0
for script in "${scripts[@]}"; do
  name=$(basename "$script" .py)
  log="$OUT/${name}.log"
  port=$((18000 + i))
  i=$((i + 1))
  echo "host-api-e2e: running ${name} (port ${port})"
  set +e
  "$PY" "$script" "$port" >"$log" 2>&1
  rc=$?
  set -e
  if [[ "$rc" -eq 2 ]]; then
    # Script did not accept a port argument; retry with no extras.
    echo "host-api-e2e: ${name} rejected the port argument; retrying without it"
    set +e
    "$PY" "$script" >"$log" 2>&1
    rc=$?
    set -e
  fi
  if [[ "$rc" -ne 0 ]]; then
    echo "host-api-e2e: FAIL ${name} (exit ${rc})" >&2
    tail -n 80 "$log" >&2 || true
    failed+=("${name} (${rc})")
  else
    echo "host-api-e2e: PASS ${name}"
    passed+=("${name}")
  fi
done

summary_file="$OUT/summary.txt"
{
  echo "host-api-e2e: ${#passed[@]} passed, ${#failed[@]} failed, ${#scripts[@]} total"
  if [[ ${#passed[@]} -gt 0 ]]; then
    echo "PASS: ${passed[*]}"
  fi
  if [[ ${#failed[@]} -gt 0 ]]; then
    echo "FAIL: ${failed[*]}"
  fi
} | tee "$summary_file"

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  {
    echo "## Host-API e2e"
    echo
    echo '```'
    cat "$summary_file"
    echo '```'
    for script in "${scripts[@]}"; do
      name=$(basename "$script" .py)
      log="$OUT/${name}.log"
      echo
      echo "### ${name}"
      echo '```'
      if [[ -f "$log" ]]; then
        cat "$log"
      else
        echo "(no log)"
      fi
      echo '```'
    done
  } >> "$GITHUB_STEP_SUMMARY"
fi

if [[ ${#failed[@]} -ne 0 ]]; then
  echo "host-api-e2e: FAIL" >&2
  exit 1
fi
echo "host-api-e2e: PASS"
exit 0
