#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Formula-function differential (#73 Math & Trig / Statistical, #74 Logical /
# Text / Information / Date & Time, including the 1904 date system).
# Sweep every catalog function in those families, then in-scope xlsx corpus
# formulas, against upstream recalculateAll. A built-in negative control must fail.
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

echo "fns-parity: building xlsx_fns_dump"
( cd rs && cargo build --release -q -p nitro-xml-xlsx --example xlsx_fns_dump )

DUMP="$ROOT/rs/target/release/examples/xlsx_fns_dump"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

echo "fns-parity: sweep cases"
bun "$ROOT/tools/model-compare/gates/ref/xlsx_fns_cases.ts" > "$WORK/sweep.json"

python3 - "$ROOT" "$WORK/corpus_raw.json" << 'PY'
import json, os, sys, zipfile
import xml.etree.ElementTree as ET
root, dest = sys.argv[1], sys.argv[2]
manifest = json.load(open(os.path.join(root, "fixtures/manifest.json"), encoding="utf-8"))

def local(tag):
    return tag.rsplit("}", 1)[-1]

def text_of(node):
    parts = []
    if node.text:
        parts.append(node.text)
    for child in node:
        parts.append(text_of(child))
        if child.tail:
            parts.append(child.tail)
    return "".join(parts)

def col_row(ref):
    col = 0
    i = 0
    while i < len(ref) and ref[i].isalpha():
        col = col * 26 + (ord(ref[i].upper()) - 64)
        i += 1
    if i == 0 or i == len(ref):
        return None
    row = int(ref[i:])
    if row <= 0 or col <= 0:
        return None
    return col - 1, row - 1

def shared_strings(zf):
    name = "xl/sharedStrings.xml"
    if name not in zf.namelist():
        return []
    root_el = ET.fromstring(zf.read(name))
    out = []
    for si in root_el:
        if local(si.tag) != "si":
            continue
        out.append(text_of(si))
    return out

def sheet_targets(zf):
    wb = ET.fromstring(zf.read("xl/workbook.xml"))
    rels = ET.fromstring(zf.read("xl/_rels/workbook.xml.rels"))
    targets = {}
    for rel in rels:
        if local(rel.tag) != "Relationship":
            continue
        rid = rel.attrib.get("Id")
        target = rel.attrib.get("Target", "")
        if target.startswith("/"):
            target = target.lstrip("/")
        elif not target.startswith("xl/"):
            target = "xl/" + target
        targets[rid] = target
    sheets = []
    for node in wb.iter():
        if local(node.tag) != "sheet":
            continue
        name = node.attrib.get("name")
        rid = node.attrib.get("{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id")
        if not name or not rid or rid not in targets:
            continue
        path = targets[rid]
        if "worksheets/" not in path:
            continue
        sheets.append((name, path))
    return sheets

def cell_value(cell, strings):
    kind = cell.attrib.get("t", "")
    if kind == "inlineStr":
        for child in cell:
            if local(child.tag) == "is":
                return text_of(child)
        return ""
    value = None
    for child in cell:
        if local(child.tag) == "v":
            value = child.text or ""
            break
    if kind == "s":
        try:
            return strings[int(value or 0)]
        except (ValueError, IndexError):
            return ""
    if kind == "b":
        return value == "1"
    if kind == "e":
        return {"error": value or "#VALUE!"}
    if kind == "str":
        return value or ""
    if value is None or value == "":
        return None
    try:
        return float(value)
    except ValueError:
        return value

cases = []
files = 0
skipped = []
for entry in manifest.get("fixtures", []):
    if entry.get("format") != "xlsx":
        continue
    path = os.path.join(root, entry["path"])
    if not os.path.exists(path):
        skipped.append(entry["path"] + " (missing)")
        continue
    try:
        zf = zipfile.ZipFile(path)
    except zipfile.BadZipFile:
        skipped.append(entry["path"] + " (not a zip)")
        continue
    files += 1
    try:
        strings = shared_strings(zf)
        sheets = sheet_targets(zf)
    except (KeyError, ET.ParseError):
        continue
    names = [name for name, _path in sheets]
    stored = []
    formulas = []
    for name, path_in in sheets:
        if path_in not in zf.namelist():
            continue
        try:
            sheet = ET.fromstring(zf.read(path_in))
        except ET.ParseError:
            continue
        for cell in sheet.iter():
            if local(cell.tag) != "c":
                continue
            ref = cell.attrib.get("r")
            if not ref:
                continue
            ref = ref.replace("$", "")
            parsed = col_row(ref)
            if not parsed:
                continue
            col, row = parsed
            formula = None
            formula_kind = None
            for child in cell:
                if local(child.tag) == "f":
                    formula_kind = child.attrib.get("t")
                    formula = text_of(child).strip()
                    break
            value = cell_value(cell, strings)
            key = f"{name}!{ref}"
            stored.append((key, value))
            if formula and formula_kind not in ("array", "dataTable") and len(formula) <= 1500:
                formulas.append((formula, name, row, col))
    if len(stored) > 2500 or len(formulas) > 400:
        continue
    cells = {key: value for key, value in stored}
    for formula, sheet, row, col in formulas:
        cases.append({
            "formula": formula,
            "cells": cells,
            "sheet": sheet,
            "row": row,
            "col": col,
            "sheets": names or ["Sheet1"],
            "source": entry["path"],
        })
print(f"fns-parity: corpus files={files} raw={len(cases)} skipped={len(skipped)}", file=sys.stderr)
for note in skipped:
    print(f"fns-parity: skip {note}", file=sys.stderr)
with open(dest, "w", encoding="utf-8") as fh:
    json.dump(cases, fh, ensure_ascii=False)
    fh.write("\n")
PY

echo "fns-parity: filter in-scope corpus formulas"
bun "$ROOT/tools/model-compare/gates/ref/xlsx_fns_cases.ts" --filter "$WORK/corpus_raw.json" > "$WORK/corpus.json"

bun - "$WORK/sweep.json" "$WORK/corpus.json" "$WORK/cases.json" << 'JS'
const [sweepPath, corpusPath, dest] = process.argv.slice(2);
const sweep = JSON.parse(await Bun.file(sweepPath).text());
const corpus = JSON.parse(await Bun.file(corpusPath).text());
await Bun.write(dest, JSON.stringify([...sweep, ...corpus]));
console.error(`fns-parity: cases sweep=${sweep.length} corpus=${corpus.length}`);
JS

echo "fns-parity: rust dump"
"$DUMP" "$WORK/cases.json" > "$WORK/rust.json"
echo "fns-parity: ts oracle"
bun "$ROOT/tools/model-compare/gates/ref/xlsx_fns_ref.ts" "$WORK/cases.json" > "$WORK/ts.json"

compare() {
  local negative="${1:-}"
  bun - "$WORK/rust.json" "$WORK/ts.json" "$WORK/cases.json" "$negative" << 'JS'
const [rustPath, tsPath, casesPath, negative] = process.argv.slice(2);
const rust = JSON.parse(await Bun.file(rustPath).text());
const ts = JSON.parse(await Bun.file(tsPath).text());
const cases = JSON.parse(await Bun.file(casesPath).text());
if (!Array.isArray(rust) || !Array.isArray(ts)) {
  console.error("fns-parity: dump is not an array");
  process.exit(1);
}
if (negative === "1") {
  if (rust.length === 0) rust.push({ formula: "", rows: [["~corrupt"]] });
  else rust[0] = { ...rust[0], rows: [["~corrupt"]] };
}
if (rust.length !== ts.length) {
  console.error(`fns-parity: length rust=${rust.length} ts=${ts.length}`);
  process.exit(1);
}
function close(a, b) {
  if (typeof a === "number" && typeof b === "number") {
    if (Object.is(a, b)) return "exact";
    if (Number.isFinite(a) && Number.isFinite(b) && Number(a.toExponential(14)) === Number(b.toExponential(14))) {
      return "rounded";
    }
    return null;
  }
  if (Array.isArray(a) || Array.isArray(b)) {
    if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) return null;
    let kind = "exact";
    for (let i = 0; i < a.length; i++) {
      const got = close(a[i], b[i]);
      if (!got) return null;
      if (got === "rounded") kind = "rounded";
    }
    return kind;
  }
  if (a && b && typeof a === "object" && typeof b === "object") {
    const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
    let kind = "exact";
    for (const key of keys) {
      const got = close(a[key], b[key]);
      if (!got) return null;
      if (got === "rounded") kind = "rounded";
    }
    return kind;
  }
  return Object.is(a, b) ? "exact" : null;
}
let bad = 0;
let exact = 0;
let rounded = 0;
for (let i = 0; i < rust.length; i++) {
  const kind = close(rust[i], ts[i]);
  if (kind === "exact") exact++;
  else if (kind === "rounded") rounded++;
  else {
    bad++;
    if (bad <= 12 && negative !== "1") {
      const formula = cases[i]?.formula ?? rust[i]?.formula ?? "";
      console.error(`fns-parity: mismatch #${i} ${JSON.stringify(String(formula).slice(0, 180))}`);
      console.error(`  rust ${JSON.stringify(rust[i]).slice(0, 500)}`);
      console.error(`  ts   ${JSON.stringify(ts[i]).slice(0, 500)}`);
    }
  }
}
if (negative === "1") {
  if (bad === 0) {
    console.error("fns-parity: negative-control did not detect corruption");
    process.exit(0);
  }
  console.error(`fns-parity: negative-control detected ${bad} mismatch(es)`);
  process.exit(1);
}
if (bad !== 0) {
  console.error(`fns-parity: FAIL ${bad} mismatch(es) exact=${exact} rounded=${rounded}`);
  process.exit(1);
}
const sweep = cases.filter((item) => item.kind !== "corpus").length;
const corpus = cases.length - sweep;
console.log(`fns-parity: PASS cases=${rust.length} sweep=${sweep} corpus=${corpus} exact=${exact} rounded=${rounded}`);
JS
}

echo "fns-parity: compare"
compare 0

echo "fns-parity: negative control"
set +e
compare 1
nc=$?
set -e
if [ "$nc" -eq 0 ]; then
  echo "fns-parity: negative-control unexpectedly passed" >&2
  exit 1
fi
echo "fns-parity: negative-control FAIL as expected"
exit 0
