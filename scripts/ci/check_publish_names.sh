#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Read-only availability check for recommended publish names.
# Does not register or publish anything. Exit 0 if all recommended names are free
# (or --report-only); exit 1 if a recommended name is taken.
set -euo pipefail

UA="${PUBLISH_NAMES_UA:-nitro-xml-name-check/0.1 (+https://github.com/jgridifier/nitro-xml; read-only)}"
REPORT_ONLY=0
STRICT=1
for arg in "$@"; do
  case "$arg" in
    --report-only) REPORT_ONLY=1; STRICT=0 ;;
    --strict) STRICT=1 ;;
    -h|--help)
      echo "Usage: $0 [--report-only] [--strict]"
      echo "  Checks npm / crates.io / PyPI for recommended Nitro XML publish names."
      echo "  Never registers or publishes."
      exit 0
      ;;
  esac
done

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
taken=0
free=0

http_get() {
  local url="$1" out="$2"
  curl -sL -A "$UA" -H 'Accept: application/json' -o "$out" -w "%{http_code}" "$url"
}

check_npm() {
  local name="$1"
  local enc
  enc="$(python3 -c "import urllib.parse,sys; print(urllib.parse.quote(sys.argv[1], safe=''))" "$name")"
  local code
  code="$(http_get "https://registry.npmjs.org/${enc}" "$tmp/npm.json")"
  if [[ "$code" == "404" ]]; then
    echo "npm     FREE  $name"
    free=$((free + 1))
  elif [[ "$code" == "200" ]]; then
    echo "npm     TAKEN $name"
    taken=$((taken + 1))
  else
    echo "npm     ERROR $name HTTP $code" >&2
    taken=$((taken + 1))
  fi
}

check_crates() {
  local name="$1"
  local code
  code="$(http_get "https://crates.io/api/v1/crates/${name}" "$tmp/crates.json")"
  if [[ "$code" == "404" ]]; then
    echo "crates  FREE  $name"
    free=$((free + 1))
  elif [[ "$code" == "200" ]]; then
    echo "crates  TAKEN $name"
    taken=$((taken + 1))
  else
    echo "crates  ERROR $name HTTP $code" >&2
    taken=$((taken + 1))
  fi
}

check_pypi() {
  local name="$1"
  local code
  code="$(http_get "https://pypi.org/pypi/${name}/json" "$tmp/pypi.json")"
  if [[ "$code" == "404" ]]; then
    echo "pypi    FREE  $name"
    free=$((free + 1))
  elif [[ "$code" == "200" ]]; then
    echo "pypi    TAKEN $name"
    taken=$((taken + 1))
  else
    echo "pypi    ERROR $name HTTP $code" >&2
    taken=$((taken + 1))
  fi
}

echo "Recommended publish names (read-only; see docs/PUBLISHING_NAMES.md)"
echo "---"
check_npm "nitro-xml"
check_npm "@nitro-xml/react"
check_crates "nitro-xml"
check_crates "nitro-xml-opc"
check_crates "nitro-xml-render"
check_crates "nitro-xml-xlsx"
check_crates "nitro-xml-docx"
check_crates "nitro-xml-pptx"
check_crates "nitro-xml-wasm"
check_pypi "nitro-xml"

# npm org / scope (informational)
org_code="$(http_get "https://registry.npmjs.org/-/org/nitro-xml/user" "$tmp/org.json")"
if [[ "$org_code" == "404" ]]; then
  echo "npm-org MISSING nitro-xml (create before publishing @nitro-xml/*)"
else
  echo "npm-org HTTP $org_code nitro-xml"
fi

echo "---"
echo "free=$free taken_or_error=$taken"
if [[ "$STRICT" -eq 1 && "$taken" -gt 0 ]]; then
  echo "check_publish_names: one or more recommended names are taken or errored" >&2
  exit 1
fi
exit 0
