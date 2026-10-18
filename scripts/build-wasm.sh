#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Builds packages/nitro-xml/pkg with wasm-pack (release: opt-level 3, LTO, codegen-units 1, wasm-opt -O3,
# SIMD on) and prints raw, gzip and brotli sizes of the .wasm and the JS glue (read by #7 and #46).
# Usage: bash scripts/build-wasm.sh [--dev] [--features <feat>] [--json <file>]
# Default (no --features) is the shipped package: bench exports and serde are off.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
# shellcheck source=ci/versions.env
. scripts/ci/versions.env
export PATH="$HOME/.cargo/bin:$PATH"
PROFILE=--release; JSON=""; FEATURES=""
while [ $# -gt 0 ]; do
  case $1 in
    --dev) PROFILE=--dev;;
    --features) FEATURES=$2; shift;;
    --json) JSON=$2; shift;;
  esac
  shift
done
WP="$NITRO_TOOLS_DIR/wasm-pack-$WASM_PACK_VERSION/wasm-pack"
if [ ! -x "$WP" ]; then
  mkdir -p "$(dirname "$WP")"
  curl -fsSL "https://github.com/wasm-bindgen/wasm-pack/releases/download/v$WASM_PACK_VERSION/wasm-pack-v$WASM_PACK_VERSION-x86_64-unknown-linux-musl.tar.gz" \
    | tar -xz -C "$(dirname "$WP")" --strip-components=1
fi
PKG="$ROOT/packages/nitro-xml"
EXTRA=()
if [ -n "$FEATURES" ]; then EXTRA=(-- --features "$FEATURES"); fi
"$WP" build "$ROOT/rs/crates/nitro-xml-wasm" $PROFILE --target web --no-pack --out-dir "$PKG/pkg" --out-name nitro_xml_wasm "${EXTRA[@]}"
rm -f "$PKG/pkg/.gitignore"
cp "$ROOT/LICENSE" "$ROOT/LICENSE-THIRD-PARTY" "$ROOT/NOTICE" "$PKG/"
# Strip JSDoc/block comments and leading indentation from generated glue so they
# do not inflate the loader+glue brotli budget (#132 method; headroom for
# WordArt outline exports and docx/editor). Indentation is insignificant in JS.
node - "$PKG/pkg/nitro_xml_wasm.js" <<'STRIP'
const fs = require("fs");
const path = process.argv[2];
let s = fs.readFileSync(path, "utf8");
s = s.replace(/\/\*\*?[\s\S]*?\*\//g, "");
s = s.replace(/^[ \t]*\/\/.*$/gm, "");
s = s.replace(/^[ \t]+/gm, "");
s = s.replace(/\n{2,}/g, "\n");
fs.writeFileSync(path, s);
STRIP
node - "$PKG/pkg" "$JSON" <<'JS'
const fs = require('fs'), zlib = require('zlib'), path = require('path');
const [dir, json] = process.argv.slice(2);
const rows = ['nitro_xml_wasm_bg.wasm', 'nitro_xml_wasm.js'].map((f) => {
  const b = fs.readFileSync(path.join(dir, f));
  return { file: f, raw: b.length, gzip: zlib.gzipSync(b, { level: 9 }).length,
    brotli: zlib.brotliCompressSync(b, { params: { [zlib.constants.BROTLI_PARAM_QUALITY]: 11 } }).length };
});
console.log('file                      raw      gzip    brotli');
for (const r of rows) console.log(r.file.padEnd(24), String(r.raw).padStart(8), String(r.gzip).padStart(8), String(r.brotli).padStart(8));
if (json) fs.writeFileSync(json, JSON.stringify(rows, null, 2) + '\n');
JS
