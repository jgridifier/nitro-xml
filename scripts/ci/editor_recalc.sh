#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Editor recalculation and formula-coverage gate (#77).
# Every edits/xlsx/*.json formula cell matches the upstream edit session after
# each do/undo/redo. A negative control (recalc off) must disagree.
# The only approved catalog gap is the 12 Database functions.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

# shellcheck source=versions.env
. scripts/ci/versions.env
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"

if ! command -v bun >/dev/null 2>&1; then
  curl -fsSL https://bun.sh/install | bash
  export PATH="$HOME/.bun/bin:$PATH"
fi

# shellcheck source=ensure-upstream-oracle.sh
. scripts/ci/ensure-upstream-oracle.sh "$ROOT"

echo "editor-recalc: building xlsx_editor_recalc"
( cd rs && cargo build --release -q -p nitro-xml-xlsx --example xlsx_editor_recalc )
BIN="$ROOT/rs/target/release/examples/xlsx_editor_recalc"

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

echo "editor-recalc: catalog vs Rust registry"
"$BIN" --names > "$WORK/rust-names.tsv"
bun -e '
const { FUNCTION_CATALOG } = await import(process.env.UPSTREAM_DIR + "/src/core/xlsx/formula/functions/registry.ts");
for (const info of FUNCTION_CATALOG) console.log(info.name + "\t" + info.category);
' > "$WORK/ts-names.tsv"

python3 - "$WORK/ts-names.tsv" "$WORK/rust-names.tsv" "$ROOT/docs/formula-coverage.md" << 'PY'
import sys
ts_path, rust_path, doc_path = sys.argv[1:]
def load(path):
    rows = {}
    for line in open(path, encoding="utf-8"):
        line = line.strip()
        if not line:
            continue
        name, cat = line.split("\t", 1)
        rows[name] = cat
    return rows
ts, rust = load(ts_path), load(rust_path)
missing = sorted(set(ts) - set(rust))
extra = sorted(set(rust) - set(ts))
approved = [
    "DAVERAGE", "DCOUNT", "DCOUNTA", "DGET", "DMAX", "DMIN",
    "DPRODUCT", "DSTDEV", "DSTDEVP", "DSUM", "DVAR", "DVARP",
]
if extra:
    print("editor-recalc: Rust registry has names absent from FUNCTION_CATALOG:", extra)
    sys.exit(1)
if missing != approved:
    print("editor-recalc: catalog gap is not the 12 Database functions")
    print(" missing:", missing)
    sys.exit(1)
doc = open(doc_path, encoding="utf-8").read()
for name in approved:
    if name not in doc:
        print(f"editor-recalc: docs/formula-coverage.md omits {name}")
        sys.exit(1)
if "469" not in doc or "481" not in doc:
    print("editor-recalc: coverage doc is missing the 481/469 counts")
    sys.exit(1)
print(f"editor-recalc: catalog {len(ts)} rust {len(rust)} gap {len(missing)} (Database only)")
PY

MANIFEST="$ROOT/fixtures/manifest.json"
python3 - "$MANIFEST" "$ROOT" << 'PY' > "$WORK/fixtures.tsv"
import json, sys
root = sys.argv[2]
manifest = json.load(open(sys.argv[1], encoding="utf-8"))
for row in manifest["fixtures"]:
    print(f"{row['id']}\t{root}/{row['path']}")
PY

scripts=0
steps=0
formula_cells=0
shopt -s nullglob
for script in "$ROOT"/edits/xlsx/*.json; do
  id=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$script")
  fixture_id=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["fixture"])' "$script")
  fixture=$(awk -F '\t' -v id="$fixture_id" '$1==id {print $2; exit}' "$WORK/fixtures.tsv")
  if [ -z "$fixture" ] || [ ! -f "$fixture" ]; then
    echo "editor-recalc: missing fixture $fixture_id for $id" >&2
    exit 1
  fi
  bun "$ROOT/tools/model-compare/gates/ref/xlsx_editor_recalc.mjs" "$fixture" "$script" "$WORK/ts.json"
  "$BIN" "$fixture" "$script" "$WORK/rust.json"
  python3 - "$id" "$WORK/ts.json" "$WORK/rust.json" << 'PY'
import json, sys
script_id, ts_path, rust_path = sys.argv[1:]
ts = json.load(open(ts_path, encoding="utf-8"))
rust = json.load(open(rust_path, encoding="utf-8"))
tc, rc = ts["checkpoints"], rust["checkpoints"]
if len(tc) != len(rc):
    print(f"editor-recalc: {script_id} checkpoint count ts={len(tc)} rust={len(rc)}")
    sys.exit(1)
formulas = 0
for i, (a, b) in enumerate(zip(tc, rc)):
    if a.get("phase") != b.get("phase"):
        print(f"editor-recalc: {script_id} checkpoint {i} phase {a.get('phase')} vs {b.get('phase')}")
        sys.exit(1)
    if a.get("formulas") != b.get("formulas"):
        print(f"editor-recalc: {script_id} checkpoint {i} ({a.get('phase')}) formula mismatch")
        print("ts  ", json.dumps(a.get("formulas"), ensure_ascii=False)[:2000])
        print("rust", json.dumps(b.get("formulas"), ensure_ascii=False)[:2000])
        sys.exit(1)
    formulas += len(a.get("formulas") or [])
PY
  read -r n_steps n_forms < <(python3 -c 'import json,sys
ts=json.load(open(sys.argv[1]))
forms=sum(len(c.get("formulas") or []) for c in ts["checkpoints"])
print(len(ts["checkpoints"]), forms)' "$WORK/ts.json")
  scripts=$((scripts + 1))
  steps=$((steps + n_steps))
  formula_cells=$((formula_cells + n_forms))
done

if [ "$scripts" -lt 1 ]; then
  echo "editor-recalc: no edits/xlsx scripts" >&2
  exit 1
fi

echo "editor-recalc: negative control (recalc off must disagree on xlsx-set-cell-formula)"
neg_script="$ROOT/edits/xlsx/xlsx-set-cell-formula.json"
neg_fixture_id=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["fixture"])' "$neg_script")
neg_fixture=$(awk -F '\t' -v id="$neg_fixture_id" '$1==id {print $2; exit}' "$WORK/fixtures.tsv")
bun "$ROOT/tools/model-compare/gates/ref/xlsx_editor_recalc.mjs" "$neg_fixture" "$neg_script" "$WORK/ts-neg.json"
"$BIN" --no-recalc "$neg_fixture" "$neg_script" "$WORK/rust-neg.json"
python3 - "$WORK/ts-neg.json" "$WORK/rust-neg.json" << 'PY'
import json, sys
ts = json.load(open(sys.argv[1], encoding="utf-8"))
rust = json.load(open(sys.argv[2], encoding="utf-8"))
if ts == rust:
    print("editor-recalc: negative control matched TS with recalc disabled")
    sys.exit(1)
# The typed formula must stay null on the Rust side and be calculated on TS.
def after_dos(doc):
    dos = [c for c in doc["checkpoints"] if c["phase"] == "do"]
    return dos[-1]["formulas"] if dos else []
ts_f, rust_f = after_dos(ts), after_dos(rust)
if not ts_f or ts_f[0].get("v") in (None,):
    print("editor-recalc: negative control TS side did not calculate", ts_f)
    sys.exit(1)
if not rust_f or rust_f[0].get("v") is not None:
    print("editor-recalc: negative control Rust side was not stale", rust_f)
    sys.exit(1)
print("editor-recalc: negative control FAIL as expected")
PY

echo "editor-recalc: PASS scripts=$scripts steps=$steps formula_cells=$formula_cells ported=469 gap=12"
