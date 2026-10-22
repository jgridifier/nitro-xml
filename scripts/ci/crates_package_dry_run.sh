#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# crates.io packaging dry-run: `cargo package` (verify) then `cargo publish -n` per workspace crate in dependency order.
# Before the first crates.io release, internal path deps are not on crates.io; this script vendors each crate into
# `rs/target/publish-vendor/` as it is packaged so later crates can verify against earlier ones.
set -euo pipefail
cd "$(dirname "$0")/../.."
export PATH="$HOME/.cargo/bin:$PATH"
rustup toolchain install --no-self-update >/dev/null
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
cd rs

CRATES=(
  nitro-xml-opc
  nitro-xml-render
  nitro-xml-xlsx
  nitro-xml-docx
  nitro-xml-pptx
  nitro-xml
  nitro-xml-wasm
)

VENDOR_DIR="$PWD/target/publish-vendor"
CARGO_HOME_PUBLISH="$PWD/target/publish-cargo-home"
rm -rf "$VENDOR_DIR" "$CARGO_HOME_PUBLISH"
mkdir -p "$CARGO_HOME_PUBLISH"

echo "crates_package_dry_run: cargo vendor (external deps)"
cargo vendor "$VENDOR_DIR" >/dev/null 2>&1

cat > "$CARGO_HOME_PUBLISH/config.toml" <<EOF
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "$VENDOR_DIR"
EOF

vendor_internal_crate() {
  local name="$1"
  local version="$2"
  local crate="$VENDOR_DIR/$name"
  local tarball="$PWD/target/package/${name}-${version}.crate"
  rm -rf "$crate"
  mkdir -p "$crate"
  tar xzf "$tarball" -C "$crate" --strip-components=1
  python3 - "$crate" <<'PY'
import hashlib, json, sys
from pathlib import Path
root = Path(sys.argv[1])
files = {}
for p in sorted(root.rglob("*")):
    if p.is_file() and p.name != ".cargo-checksum.json":
        rel = p.relative_to(root).as_posix()
        files[rel] = hashlib.sha256(p.read_bytes()).hexdigest()
(root / ".cargo-checksum.json").write_text(json.dumps({"package": None, "files": files}))
PY
}

export CARGO_HOME="$CARGO_HOME_PUBLISH"
version="$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml","rb"))["workspace"]["package"]["version"])')"

for crate in "${CRATES[@]}"; do
  echo "crates_package_dry_run: cargo package -p ${crate}"
  cargo package -p "${crate}"
  echo "crates_package_dry_run: cargo publish -n -p ${crate} --registry crates-io"
  cargo publish -n -p "${crate}" --registry crates-io
  vendor_internal_crate "${crate}" "${version}"
done

echo "crates_package_dry_run: ok (${#CRATES[@]} crates)"
