#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Calc-graph differential (#72): recalc order, precedents, and values for
# formulas whose precedent cone has no function call and is not volatile.
# Invoked from scripts/ci/parity.sh. Agents never edit .github/workflows/.
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

echo "calc-parity: building xlsx_calc_dump"
( cd rs && cargo build --release -q -p nitro-xml-xlsx --example xlsx_calc_dump )

DUMP="$ROOT/rs/target/release/examples/xlsx_calc_dump"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

python3 - "$ROOT" "$WORK/files.txt" << 'PY'
import json, os, sys, zipfile
root, dest = sys.argv[1], sys.argv[2]
manifest = json.load(open(os.path.join(root, "fixtures/manifest.json"), encoding="utf-8"))
files = []
skipped = []
for entry in manifest.get("fixtures", []):
    if entry.get("format") != "xlsx":
        continue
    path = os.path.join(root, entry["path"])
    if not os.path.exists(path):
        skipped.append(entry["path"] + " (missing)")
        continue
    try:
        zipfile.ZipFile(path).close()
    except zipfile.BadZipFile:
        skipped.append(entry["path"] + " (not a zip)")
        continue
    files.append(path)
print(f"calc-parity: files={len(files)} skipped={len(skipped)}", file=sys.stderr)
for note in skipped:
    print(f"calc-parity: skip {note}", file=sys.stderr)
if not files:
    print("calc-parity: no xlsx fixtures", file=sys.stderr)
    sys.exit(1)
with open(dest, "w", encoding="utf-8") as fh:
    fh.write("\n".join(files))
    fh.write("\n")
PY

mapfile -t FILES < "$WORK/files.txt"

echo "calc-parity: rust dump (${#FILES[@]} files)"
"$DUMP" "${FILES[@]}" > "$WORK/rust.json"
echo "calc-parity: ts oracle"
bun "$ROOT/tools/model-compare/gates/ref/xlsx_calc_ref.ts" "${FILES[@]}" > "$WORK/ts.json"

compare() {
  local negative="${1:-}"
  bun - "$WORK/rust.json" "$WORK/ts.json" "$negative" << 'JS'
const [rustPath, tsPath, negative] = process.argv.slice(2);
const rust = JSON.parse(await Bun.file(rustPath).text());
const ts = JSON.parse(await Bun.file(tsPath).text());
const rustResults = rust.results ?? [];
const tsResults = ts.results ?? [];
if (negative === "1") {
  const cell = rustResults.find((item) => item.cells && item.cells.length)?.cells?.[0];
  if (cell) cell.rank = (cell.rank ?? 0) + 1;
  else rustResults.push({ file: "negative-control", cells: [{ rank: 1 }] });
}
if (rustResults.length !== tsResults.length) {
  console.error(`calc-parity: files rust=${rustResults.length} ts=${tsResults.length}`);
  process.exit(1);
}
let formulas = 0;
let values = 0;
let bad = 0;
let skipped = 0;
const fields = ["sheet","row","col","formula","rank","cyclic","circular","volatile","unsupported","keepCached","hasCall","deps","dynamicDeps"];
for (let f = 0; f < tsResults.length; f++) {
  const a = rustResults[f];
  const b = tsResults[f];
  if (a.error && b.error) {
    skipped++;
    continue;
  }
  if (a.error || b.error || !a.cells || !b.cells) {
    bad++;
    if (bad <= 8 && negative !== "1") {
      console.error(`calc-parity: load mismatch ${b.file ?? a.file}`);
      console.error(`  rust ${a.error ?? "cells"}`);
      console.error(`  ts   ${b.error ?? "cells"}`);
    }
    continue;
  }
  if (a.cells.length !== b.cells.length) {
    bad++;
    if (bad <= 8 && negative !== "1") {
      console.error(`calc-parity: count ${b.file} rust=${a.cells.length} ts=${b.cells.length}`);
    }
    continue;
  }
  const impure = impurity(b.cells);
  for (let i = 0; i < b.cells.length; i++) {
    formulas++;
    const left = a.cells[i];
    const right = b.cells[i];
    for (const field of fields) {
      if (stable(left[field]) !== stable(right[field])) {
        bad++;
        if (bad <= 8 && negative !== "1") report(b.file, i, field, left, right);
      }
    }
    const compareValue = right.circular || !impure[i];
    if (compareValue) {
      values++;
      if (stable(left.value) !== stable(right.value)) {
        bad++;
        if (bad <= 8 && negative !== "1") report(b.file, i, "value", left, right);
      }
    }
  }
}
if (negative === "1") {
  if (bad === 0) {
    console.error("calc-parity: negative-control did not detect corruption");
    process.exit(0);
  }
  console.error(`calc-parity: negative-control detected ${bad} mismatch(es)`);
  process.exit(1);
}
if (bad !== 0) {
  console.error(`calc-parity: FAIL ${bad} mismatch(es)`);
  process.exit(1);
}
console.log(`calc-parity: PASS files=${tsResults.length - skipped} formulas=${formulas} values=${values} skipped=${skipped}`);

function report(file, index, field, left, right) {
  const formula = right.formula ?? left.formula ?? "";
  console.error(`calc-parity: mismatch ${file} #${index} ${field} ${JSON.stringify(String(formula).slice(0, 160))}`);
  console.error(`  rust ${stable(left[field]).slice(0, 400)}`);
  console.error(`  ts   ${stable(right[field]).slice(0, 400)}`);
}

function stable(value) {
  return JSON.stringify(sortKeys(value));
}

function sortKeys(value) {
  if (Array.isArray(value)) return value.map(sortKeys);
  if (value && typeof value === "object") {
    const out = {};
    for (const key of Object.keys(value).sort()) out[key] = sortKeys(value[key]);
    return out;
  }
  return value;
}

function impurity(cells) {
  const impure = cells.map((cell) => !!(cell.hasCall || cell.volatile));
  const bySheet = new Map();
  cells.forEach((cell, index) => {
    let sheet = bySheet.get(cell.sheet);
    if (!sheet) bySheet.set(cell.sheet, (sheet = new Map()));
    let list = sheet.get(cell.col);
    if (!list) sheet.set(cell.col, (list = []));
    list.push(index);
  });
  const formulasIn = (area, visit) => {
    const cols = bySheet.get(area.sheet);
    if (!cols) return;
    const { start, end } = area.range;
    const scan = (list) => {
      if (!list) return;
      for (const index of list) {
        const cell = cells[index];
        if (cell.row >= start.row && cell.row <= end.row) visit(index);
      }
    };
    if (end.col - start.col + 1 <= cols.size) {
      for (let col = start.col; col <= end.col; col++) scan(cols.get(col));
    } else {
      for (const [col, list] of cols) {
        if (col >= start.col && col <= end.col) scan(list);
      }
    }
  };
  let guard = 0;
  let changed = true;
  while (changed && guard++ < cells.length + 1) {
    changed = false;
    for (let i = 0; i < cells.length; i++) {
      if (impure[i]) continue;
      let hit = false;
      const look = (area) => {
        if (hit || !area) return;
        formulasIn(area, (index) => {
          if (impure[index]) hit = true;
        });
      };
      for (const dep of cells[i].deps ?? []) look(dep);
      for (const dep of cells[i].dynamicDeps ?? []) look(dep);
      if (hit) {
        impure[i] = true;
        changed = true;
      }
    }
  }
  return impure;
}
JS
}

echo "calc-parity: compare"
compare 0

echo "calc-parity: negative control"
set +e
compare 1
nc=$?
set -e
if [ "$nc" -eq 0 ]; then
  echo "calc-parity: negative-control unexpectedly passed" >&2
  exit 1
fi
echo "calc-parity: negative-control FAIL as expected"
exit 0
