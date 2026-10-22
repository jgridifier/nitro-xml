#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Multi-registry release driver. Default is DRY-RUN (no uploads).
#
# Single version source: repo-root VERSION (must match git tag v* when tagging).
#
# Env:
#   DRY_RUN=1          (default) — npm pack / cargo publish -n / maturin upload --dry-run
#   DRY_RUN=0          — real publish (requires secrets / OIDC); never set in PRs
#   SKIP_NPM=1 SKIP_CRATES=1 SKIP_PYPI=1 — skip a registry
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

DRY_RUN="${DRY_RUN:-1}"
VERSION="$(tr -d '[:space:]' < VERSION)"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.]+)?$ ]]; then
  echo "release_publish: invalid VERSION=$VERSION" >&2
  exit 1
fi

echo "release_publish: version=${VERSION} dry_run=${DRY_RUN}"

# --- sync version into package manifests (in-place for the job workspace) ---
sync_versions() {
  python3 - <<PY
from pathlib import Path
import re, json
ver = Path("VERSION").read_text().strip()
# npm
for pkg in [
    Path("packages/nitro-xml/package.json"),
    Path("packages/nitro-xml-react/package.json"),
]:
    if pkg.is_file():
        data = json.loads(pkg.read_text())
        data["version"] = ver
        pkg.write_text(json.dumps(data, indent=2) + "\n")
# cargo workspace
cargo = Path("rs/Cargo.toml")
text = cargo.read_text()
text2, n = re.subn(
    r'(?m)^version = "[^"]+"',
    f'version = "{ver}"',
    text,
    count=1,
)
if n != 1:
    raise SystemExit("release_publish: could not patch rs/Cargo.toml workspace version")
# also patch workspace.dependencies version keys
text2 = re.sub(
    r'(nitro-xml(?:-opc|-render|-xlsx|-docx|-pptx|-wasm)? = \{ version = ")[^"]+(", path)',
    rf'\g<1>{ver}\2',
    text2,
)
cargo.write_text(text2)
# pyproject if present
for py in Path(".").glob("**/pyproject.toml"):
    if "nitro-xml-py" in str(py) or py.parent.name in ("python", "nitro-xml-py"):
        t = py.read_text()
        t2, n = re.subn(r'(?m)^version = "[^"]+"', f'version = "{ver}"', t, count=1)
        if n:
            py.write_text(t2)
print(f"release_publish: synced manifests to {ver}")
PY
}

sync_versions

# Tag check when running from a v* ref
if [[ "${GITHUB_REF_TYPE:-}" == "tag" ]]; then
  tag="${GITHUB_REF_NAME:-}"
  if [[ "$tag" != "v${VERSION}" ]]; then
    echo "release_publish: tag $tag != v${VERSION} (VERSION file)" >&2
    exit 1
  fi
fi

# --- npm ---
if [[ "${SKIP_NPM:-0}" != "1" ]]; then
  echo "release_publish: npm"
  if [[ ! -f packages/nitro-xml/pkg/nitro_xml_wasm_bg.wasm ]]; then
    bash scripts/build-wasm.sh
  fi
  (
    cd packages/nitro-xml
    if [[ "$DRY_RUN" == "1" ]]; then
      npm pack --dry-run
    else
      npm publish --access public --provenance
    fi
  )
  if [[ -f packages/nitro-xml-react/package.json ]]; then
    (
      cd packages/nitro-xml-react
      npm install --ignore-scripts
      npm run build
      if [[ "$DRY_RUN" == "1" ]]; then
        npm pack --dry-run
      else
        npm publish --access public --provenance
      fi
    )
  fi
fi

# --- crates.io ---
if [[ "${SKIP_CRATES:-0}" != "1" ]]; then
  echo "release_publish: crates"
  if [[ -x scripts/ci/crates_package_dry_run.sh || -f scripts/ci/crates_package_dry_run.sh ]]; then
    if [[ "$DRY_RUN" == "1" ]]; then
      bash scripts/ci/crates_package_dry_run.sh
    else
      cd rs
      for crate in nitro-xml-opc nitro-xml-render nitro-xml-xlsx nitro-xml-docx nitro-xml-pptx nitro-xml nitro-xml-wasm; do
        cargo publish -p "$crate" --token "${CARGO_REGISTRY_TOKEN:?}"
      done
      cd "$ROOT"
    fi
  else
    echo "release_publish: WARN — crates_package_dry_run.sh missing (merge crates.io packaging PR first)" >&2
    if [[ "$DRY_RUN" != "1" ]]; then
      exit 1
    fi
  fi
fi

# --- PyPI ---
if [[ "${SKIP_PYPI:-0}" != "1" ]]; then
  echo "release_publish: pypi"
  if [[ -x scripts/ci/pypi_build_dry_run.sh ]]; then
    if [[ "$DRY_RUN" == "1" ]]; then
      bash scripts/ci/pypi_build_dry_run.sh
    else
      DRY_RUN=0 bash scripts/ci/pypi_build_dry_run.sh
    fi
  else
    echo "release_publish: WARN — scripts/ci/pypi_build_dry_run.sh missing (land PyPI PR first)" >&2
    if [[ "$DRY_RUN" != "1" ]]; then
      exit 1
    fi
  fi
fi

echo "release_publish: done (dry_run=${DRY_RUN})"
