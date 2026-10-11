#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Rust checks for the rs/ workspace (job: rust): cargo fmt --check, clippy -D warnings, cargo test --workspace.
# The toolchain (version, rustfmt, clippy, wasm32 target) is pinned by rust-toolchain.toml.
set -euo pipefail
cd "$(dirname "$0")/../.."
export PATH="$HOME/.cargo/bin:$PATH"
# Install the pinned toolchain explicitly; rustup's implicit install from rust-toolchain.toml is deprecated.
rustup toolchain install --no-self-update >/dev/null
cd rs
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
