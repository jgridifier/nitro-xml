#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# wasm32 checks (job: wasm): release build of the wasm crate (SIMD and the §3 feature set come from
# .cargo/config.toml), then wasm-bindgen-test for every workspace crate in headless Chrome.
# Tools are installed into $NITRO_TOOLS_DIR by this script, never by workflow YAML.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD
# shellcheck source=versions.env
. scripts/ci/versions.env
export PATH="$HOME/.cargo/bin:$PATH"
rustup toolchain install --no-self-update >/dev/null

# wasm-bindgen-test-runner must match the wasm-bindgen version in rs/Cargo.lock exactly.
WB_VERSION=$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/version = |"/,""); print; exit}' rs/Cargo.lock)
WB_DIR="$NITRO_TOOLS_DIR/wasm-bindgen-$WB_VERSION"
if [ ! -x "$WB_DIR/wasm-bindgen-test-runner" ]; then
  mkdir -p "$WB_DIR"
  url="https://github.com/wasm-bindgen/wasm-bindgen/releases/download/$WB_VERSION/wasm-bindgen-$WB_VERSION-x86_64-unknown-linux-musl.tar.gz"
  curl -fsSL "$url" | tar -xz -C "$WB_DIR" --strip-components=1 \
    || { echo "prebuilt wasm-bindgen $WB_VERSION unavailable; building it"; cargo install --locked --root "$WB_DIR.cargo" wasm-bindgen-cli --version "$WB_VERSION"; cp "$WB_DIR.cargo/bin/"* "$WB_DIR/"; }
fi
export PATH="$WB_DIR:$PATH"

# Headless Chrome via chromedriver: GitHub's ubuntu-latest image provides it ($CHROMEWEBDRIVER);
# elsewhere, fetch the chromedriver matching the installed Chrome.
if [ -z "${CHROMEDRIVER:-}" ]; then
  if [ -n "${CHROMEWEBDRIVER:-}" ] && [ -x "$CHROMEWEBDRIVER/chromedriver" ]; then
    CHROMEDRIVER="$CHROMEWEBDRIVER/chromedriver"
  elif command -v chromedriver >/dev/null; then
    CHROMEDRIVER=$(command -v chromedriver)
  else
    CV=$(google-chrome --version | grep -oE '[0-9]+(\.[0-9]+){3}')
    CD_DIR="$NITRO_TOOLS_DIR/chromedriver-$CV"
    if [ ! -x "$CD_DIR/chromedriver" ]; then
      mkdir -p "$CD_DIR"; tmp=$(mktemp -d)
      curl -fsSL -o "$tmp/cd.zip" "https://storage.googleapis.com/chrome-for-testing-public/$CV/linux64/chromedriver-linux64.zip"
      (cd "$tmp" && python3 -m zipfile -e cd.zip . ) && mv "$tmp"/chromedriver-linux64/chromedriver "$CD_DIR/" && chmod +x "$CD_DIR/chromedriver"; rm -rf "$tmp"
    fi
    CHROMEDRIVER="$CD_DIR/chromedriver"
  fi
fi
export CHROMEDRIVER
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner
export WASM_BINDGEN_USE_BROWSER=1   # run every wasm test in the browser (headless Chrome via $CHROMEDRIVER)

cd "$ROOT/rs"
cargo build --release --locked -p nitro-xml-wasm --target wasm32-unknown-unknown
ls -l target/wasm32-unknown-unknown/release/*.wasm
cargo test --workspace --locked --target wasm32-unknown-unknown --exclude nitro-xml-py --exclude nitro-xml-py
