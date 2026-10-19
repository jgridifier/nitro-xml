#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Assembles the Pages site into _site/:
#   /           plan (docs/)
#   /demo/      live WASM demos (normal + embedded × xlsx/docx/pptx)
#   /perf/      scoreboard slot (filled by the dashboard issue / #9)
#
# Called by .github/workflows/pages.yml. Agents must not edit that workflow —
# ask Jared to set Pages source to "GitHub Actions" and repo variable
# PAGES_DEPLOY=actions so actions/deploy-pages runs.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

rm -rf _site
mkdir -p _site
cp -R docs/. _site/

# --- Live demos (not counted toward the loader brotli budget) ---
bash "$ROOT/scripts/build-wasm.sh"
mkdir -p _site/demo
# Demo sources (HTML/JS/CSS/samples). Do not vendor nitro-xml from a local tree.
cp -R "$ROOT/demo/." "_site/demo/"
rm -rf "_site/demo/nitro-xml"
mkdir -p _site/demo/nitro-xml/loader _site/demo/nitro-xml/pkg
# Vendor loader + pkg with paths matching packages/nitro-xml (bootstrap uses ../pkg).
cp -R "$ROOT/packages/nitro-xml/loader/." "_site/demo/nitro-xml/loader/"
rm -rf "_site/demo/nitro-xml/loader/test"
cp -R "$ROOT/packages/nitro-xml/pkg/." "_site/demo/nitro-xml/pkg/"

# Build stamp shown in the demo footer.
printf '{"sha":"%s","date":"%s"}\n' "$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo dev)" "$(date -u +%F)" > _site/demo/build.json

# Sanity: sample fixtures + wasm present
test -f _site/demo/samples/sample.xlsx
test -f _site/demo/samples/sample.docx
test -f _site/demo/samples/sample.pptx
test -f _site/demo/nitro-xml/pkg/nitro_xml_wasm_bg.wasm
test -f _site/demo/nitro-xml/loader/index.js

msg="pages: plan + /demo/ (wasm $(du -h _site/demo/nitro-xml/pkg/nitro_xml_wasm_bg.wasm | awk '{print $1}')) assembled; scoreboard TODO(#9)"
echo "$msg"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  echo "$msg" >> "$GITHUB_STEP_SUMMARY"
fi
exit 0
