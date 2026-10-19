#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Builds the scoreboard's Rust competitor shims for the browser:
#   cargo (wasm32-unknown-unknown, release: opt-level 3, LTO, codegen-units 1)
#   -> wasm-bindgen --target web (version pinned to the crate) -> wasm-opt -O3 (pinned binaryen).
# Writes perf/scoreboard/dist/rust/<shim>/ and dist/rust/status.json (one entry per shim, with the
# error when a shim does not build, so the race marks it "not entered" instead of failing).
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
out="$here/../../dist/rust"
tools="${NITRO_TOOLS_DIR:-$HOME/.nitro-tools}"
WASM_BINDGEN_VERSION=0.2.129
BINARYEN_VERSION=version_133
mkdir -p "$out" "$tools"

if ! command -v wasm-bindgen >/dev/null || [ "$(wasm-bindgen --version | awk '{print $2}')" != "$WASM_BINDGEN_VERSION" ]; then
  if [ ! -x "$tools/bin/wasm-bindgen" ] || [ "$("$tools/bin/wasm-bindgen" --version | awk '{print $2}')" != "$WASM_BINDGEN_VERSION" ]; then
    cargo install wasm-bindgen-cli --version "$WASM_BINDGEN_VERSION" --locked --root "$tools"
  fi
  export PATH="$tools/bin:$PATH"
fi
if ! command -v wasm-opt >/dev/null; then
  if [ ! -x "$tools/binaryen-$BINARYEN_VERSION/bin/wasm-opt" ]; then
    curl -fsSL "https://github.com/WebAssembly/binaryen/releases/download/$BINARYEN_VERSION/binaryen-$BINARYEN_VERSION-x86_64-linux.tar.gz" \
      | tar xz -C "$tools" || echo "wasm-opt unavailable; shims stay unoptimised by binaryen"
  fi
  export PATH="$tools/binaryen-$BINARYEN_VERSION/bin:$PATH"
fi

cd "$here"
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
status="{"
sep=""
for shim in calamine umya docx-rs nitro-xlsx nitro-docx nitro-pptx; do
  crate="scoreboard-$shim"
  lib="scoreboard_${shim//-/_}"
  log="$out/$shim.build.log"
  if cargo build --release --locked --target wasm32-unknown-unknown -p "$crate" >"$log" 2>&1 \
     && rm -rf "$out/$shim" \
     && wasm-bindgen --target web --no-typescript --out-dir "$out/$shim" --out-name shim \
          "target/wasm32-unknown-unknown/release/$lib.wasm" >>"$log" 2>&1; then
    opt="none"
    if command -v wasm-opt >/dev/null && wasm-opt -O3 --enable-bulk-memory --enable-nontrapping-float-to-int \
         --enable-sign-ext --enable-reference-types --enable-multivalue --enable-mutable-globals \
         "$out/$shim/shim_bg.wasm" -o "$out/$shim/shim_bg.opt.wasm" >>"$log" 2>&1; then
      mv "$out/$shim/shim_bg.opt.wasm" "$out/$shim/shim_bg.wasm"
      opt="$(wasm-opt --version)"
    fi
    size=$(stat -c %s "$out/$shim/shim_bg.wasm")
    status+="$sep\"$shim\":{\"built\":true,\"wasmBytes\":$size,\"wasmBindgen\":\"$WASM_BINDGEN_VERSION\",\"wasmOpt\":\"$opt\",\"rustc\":\"$(rustc --version)\"}"
    echo "built $shim ($size bytes, wasm-opt: $opt)"
  else
    err=$(grep -m1 -E '^error' "$log" | sed 's/["\\]/ /g' | cut -c1-300)
    status+="$sep\"$shim\":{\"built\":false,\"error\":\"${err:-see $shim.build.log}\"}"
    echo "FAILED $shim: $err"
  fi
  sep=","
done
echo "$status}" >"$out/status.json"
