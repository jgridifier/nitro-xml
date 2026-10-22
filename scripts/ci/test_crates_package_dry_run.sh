#!/usr/bin/env bash
# Catches: workspace path deps missing version keys or publishable crates inheriting `publish = false` from the workspace.
# SPDX-License-Identifier: MIT
set -euo pipefail
cd "$(dirname "$0")/../.."
export PATH="$HOME/.cargo/bin:$PATH"

python3 - <<'PY'
import pathlib
import re
import sys
import tomllib

root = pathlib.Path("rs")
ws = tomllib.loads((root / "Cargo.toml").read_text())
ws_pkg = ws.get("workspace", {}).get("package", {})
ws_ver = ws_pkg.get("version")
if ws_ver != "0.1.0":
    sys.exit(f"workspace.package.version must be 0.1.0 for first publish, got {ws_ver!r}")

ws_deps = ws.get("workspace", {}).get("dependencies", {})
internal = [
    "nitro-xml-opc",
    "nitro-xml-render",
    "nitro-xml-xlsx",
    "nitro-xml-docx",
    "nitro-xml-pptx",
    "nitro-xml",
    "nitro-xml-wasm",
]
for name in internal:
    spec = ws_deps.get(name)
    if not isinstance(spec, dict):
        sys.exit(f"workspace.dependencies.{name} missing")
    if "path" not in spec:
        sys.exit(f"workspace.dependencies.{name} must keep path")
    if spec.get("version") != ws_ver:
        sys.exit(
            f"workspace.dependencies.{name} version {spec.get('version')!r} != workspace {ws_ver!r}"
        )

publishable = set(internal)
for toml in sorted((root / "crates").glob("*/Cargo.toml")):
    pkg = tomllib.loads(toml.read_text())["package"]
    name = pkg["name"]
    if name not in publishable:
        continue
    pub = pkg.get("publish")
    if pub is not True:
        sys.exit(f"{name}: publish must be true explicitly, got {pub!r}")
    readme = pkg.get("readme")
    if not readme:
        sys.exit(f"{name}: missing readme")
    readme_path = toml.parent / readme
    if not readme_path.is_file():
        sys.exit(f"{name}: readme {readme} missing at {readme_path}")

fuzz = tomllib.loads((root / "fuzz/Cargo.toml").read_text())["package"]
if fuzz.get("publish") is not False:
    sys.exit("fuzz package must stay publish = false")

print("test_crates_package_dry_run: Cargo.toml invariants ok")
PY

if [[ "${NITRO_CRATES_PACKAGE_SKIP_DRY_RUN:-}" == "1" ]]; then
  echo "test_crates_package_dry_run: skipping full dry-run (NITRO_CRATES_PACKAGE_SKIP_DRY_RUN=1)"
  exit 0
fi

bash scripts/ci/crates_package_dry_run.sh
