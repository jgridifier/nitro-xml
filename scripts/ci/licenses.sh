#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Licence check (job: licenses):
#  1. cargo-deny (deny.toml): dependency licences on the allow-list, bans, sources, advisories.
#  2. Workspace crates: `MIT AND Apache-2.0` if the crate holds any Apache-2.0 (ported) file, else `MIT`.
#  3. Every source file carries an SPDX header; every Apache-2.0 file has a PROVENANCE.md row.
#  4. npm: packages/*/package.json is MIT and production dependencies are on the same allow-list.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD
# shellcheck source=versions.env
. scripts/ci/versions.env
export PATH="$HOME/.cargo/bin:$PATH"
fail=0; err(){ echo "licenses: $*" >&2; fail=1; }

# 1. cargo-deny, prebuilt and pinned.
DENY="$NITRO_TOOLS_DIR/cargo-deny-$CARGO_DENY_VERSION/cargo-deny"
if [ ! -x "$DENY" ]; then
  mkdir -p "$(dirname "$DENY")"
  curl -fsSL "https://github.com/EmbarkStudios/cargo-deny/releases/download/$CARGO_DENY_VERSION/cargo-deny-$CARGO_DENY_VERSION-x86_64-unknown-linux-musl.tar.gz" \
    | tar -xz -C "$(dirname "$DENY")" --strip-components=1
fi
"$DENY" --manifest-path rs/Cargo.toml --config "$ROOT/deny.toml" check licenses bans sources advisories || err "cargo-deny failed"

# 2 and 3. Workspace crate licence fields, SPDX headers and PROVENANCE rows.
python3 - <<'PY' || fail=1
import pathlib, re, sys, tomllib
root = pathlib.Path('.')
bad = []
ws = tomllib.loads((root / 'rs/Cargo.toml').read_text())
ws_lic = ws.get('workspace', {}).get('package', {}).get('license')
prov = (root / 'PROVENANCE.md').read_text()
spdx = re.compile(r'SPDX-License-Identifier:\s*([^\s*>]+(?:\s+(?:AND|OR)\s+[^\s*>]+)*)')
SRC = ('.rs', '.ts', '.mjs', '.js', '.sh', '.py')
def header(p):
    m = spdx.search('\n'.join(p.read_text(errors='replace').splitlines()[:5]))
    return m.group(1) if m else None
for toml in sorted(root.glob('rs/crates/*/Cargo.toml')):
    crate = toml.parent; pkg = tomllib.loads(toml.read_text())['package']
    lic = pkg.get('license'); lic = ws_lic if isinstance(lic, dict) and lic.get('workspace') else lic
    files = [p for p in crate.rglob('*') if p.is_file() and p.suffix in SRC and 'target' not in p.parts]
    ported = []
    for p in files:
        h = header(p)
        if h is None: bad.append(f'{p}: missing SPDX-License-Identifier header')
        elif 'Apache-2.0' in h:
            ported.append(p)
            if str(p) not in prov: bad.append(f'{p}: Apache-2.0 (ported) file has no PROVENANCE.md row')
    want = 'MIT AND Apache-2.0' if ported else 'MIT'
    if lic != want: bad.append(f'{crate.name}: license = "{lic}", expected "{want}"' + (f' (holds ported files, e.g. {ported[0]})' if ported else ''))
for p in sorted(root.glob('packages/*/package.json')):
    import json
    if json.loads(p.read_text()).get('license') != 'MIT': bad.append(f'{p}: "license" must be "MIT"')
for b in bad: print('licenses:', b, file=sys.stderr)
sys.exit(1 if bad else 0)
PY

# 4. npm production dependencies of packages/* (none until #2 adds the package skeleton).
ALLOW='MIT;Apache-2.0;BSD-2-Clause;BSD-3-Clause;ISC;Zlib;Unicode-3.0;Unicode-DFS-2016'
for pkg in packages/*/; do
  [ -f "$pkg/package.json" ] || continue
  ( cd "$pkg" && npm ci --omit=dev --ignore-scripts --no-audit --no-fund >/dev/null \
    && npx --yes license-checker-rseidelsohn@4 --production --excludePrivatePackages --onlyAllow "$ALLOW" --summary ) \
    || err "npm production dependency licences in $pkg"
done

[ $fail -eq 0 ] && echo "licenses: ok" || exit 1
