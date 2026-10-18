#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Bundle and startup gate: per-format brotli KB ceilings (ratchet down only), JS loader size budget,
# startup/time-to-interactive gate (job: bundle-budget).
#
# #37 implements the loader JS budget check below.
# Per-format wasm payload + TTI remain TODO(#46).
#
# Definition of done for #46: replace the TODO(#46) note with the real payload/startup checks
# and ask Jared (or the Chief of Staff's gh login) to keep `bundle-budget` in required checks.
# Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

BUDGETS="$ROOT/perf/budgets.json"
LOADER_DIR="$ROOT/packages/nitro-xml/loader"
GLUE="$ROOT/packages/nitro-xml/pkg/nitro_xml_wasm.js"

if [ ! -f "$BUDGETS" ]; then
  echo "bundle-budget: missing $BUDGETS" >&2
  exit 1
fi

# Loader unit tests (protocol / allow-list); run before size so failures are obvious.
node --test "$LOADER_DIR"/test/*.mjs

if [ ! -f "$GLUE" ]; then
  echo "bundle-budget: building wasm package (glue missing)"
  bash "$ROOT/scripts/build-wasm.sh" >/dev/null
fi

# Measure hand-written loader + generated glue (concatenated, brotli q=11).
read -r MEASURED_BYTES CEILING_KB <<EOF
$(node - "$LOADER_DIR" "$GLUE" "$BUDGETS" <<'JS'
const fs = require('fs');
const zlib = require('zlib');
const path = require('path');
const [loaderDir, gluePath, budgetsPath] = process.argv.slice(2);
const budgets = JSON.parse(fs.readFileSync(budgetsPath, 'utf8'));
const ceilingKb = budgets.loader_js_brotli_kb;
if (typeof ceilingKb !== 'number' || !(ceilingKb > 0)) {
  console.error('bundle-budget: budgets.json loader_js_brotli_kb must be a positive number');
  process.exit(1);
}
const files = [];
function walk(d) {
  for (const ent of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, ent.name);
    if (ent.isDirectory()) {
      // test/: unit tests. lazy/: dynamic-import chunks (WordArt glyph-outline)
      // loaded only when a frame queues OutlineBatch — not on the first-load path.
      if (ent.name === 'test' || ent.name === 'lazy') continue;
      walk(p);
    } else if (ent.name.endsWith('.js')) files.push(p);
  }
}
walk(loaderDir);
files.sort();
if (!files.length) {
  console.error('bundle-budget: no loader JS under', loaderDir);
  process.exit(1);
}
let buf = Buffer.alloc(0);
for (const f of files) buf = Buffer.concat([buf, fs.readFileSync(f), Buffer.from('\n')]);
buf = Buffer.concat([buf, fs.readFileSync(gluePath)]);
const brotli = zlib.brotliCompressSync(buf, {
  params: { [zlib.constants.BROTLI_PARAM_QUALITY]: 11 },
});
process.stdout.write(`${brotli.length} ${ceilingKb}\n`);
process.stderr.write(
  `bundle-budget: loader JS (hand-written + glue) brotli=${brotli.length} B` +
    ` (${(brotli.length / 1024).toFixed(2)} KB), ceiling=${ceilingKb} KB` +
    ` (${files.length} loader files + glue)\n`,
);
JS
)
EOF

CEILING_BYTES=$(node -e "process.stdout.write(String(Math.floor(Number(process.argv[1])*1024)))" "$CEILING_KB")
SUMMARY="loader JS brotli: ${MEASURED_BYTES} B / ceiling ${CEILING_BYTES} B (${CEILING_KB} KB)"
echo "bundle-budget: $SUMMARY"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  echo "### bundle-budget (loader #37)" >> "$GITHUB_STEP_SUMMARY"
  echo "$SUMMARY" >> "$GITHUB_STEP_SUMMARY"
fi

if [ "$MEASURED_BYTES" -gt "$CEILING_BYTES" ]; then
  echo "bundle-budget: FAIL loader JS exceeds loader_js_brotli_kb in perf/budgets.json (ratchet-down only)" >&2
  exit 1
fi

echo "bundle-budget: loader OK"
msg="TODO(#46): per-format initial-payload ceilings and time-to-interactive not yet gated"
echo "$msg"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then echo "$msg" >> "$GITHUB_STEP_SUMMARY"; fi
