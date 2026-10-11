#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Ship check (job: ship-check): no shipped artefact may reference the upstream TS engine (ooxml-core,
# ooxml-ui, upstream/), ProseMirror or Lit. Scans every packages/* package after building it:
# package.json dependencies, the `npm pack --dry-run` file list and the wasm import section.
set -euo pipefail
cd "$(dirname "$0")/../.."
# Self-test first. Catches: a checker that silently passes everything (bad fixture must fail, by every route).
if python3 scripts/ci/ship_check.py scripts/ci/ship_check_fixtures/bad-package >/dev/null 2>&1; then
  echo "ship-check: self-test FAILED: the deliberately bad fixture package passed" >&2; exit 1
fi
n=$(python3 scripts/ci/ship_check.py scripts/ci/ship_check_fixtures/bad-package 2>&1 >/dev/null | grep -c 'references\|package.json' || true)
[ "$n" -ge 5 ] || { echo "ship-check: self-test FAILED: expected >=5 findings in the bad fixture, got $n" >&2; exit 1; }
[ -f packages/nitro-xml/pkg/nitro_xml_wasm_bg.wasm ] || bash scripts/build-wasm.sh >/dev/null
python3 scripts/ci/ship_check.py packages/*/
