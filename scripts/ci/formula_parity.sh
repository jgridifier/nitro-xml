#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Formula parser differential (#71): every stored formula in the xlsx corpus
# against upstream parseFormula (AST + refs + token join). Invoked from
# scripts/ci/parity.sh. Agents never edit .github/workflows/.
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

echo "formula-parity: building xlsx_formula_dump"
( cd rs && cargo build --release -q -p nitro-xml-xlsx --example xlsx_formula_dump )

DUMP="$ROOT/rs/target/release/examples/xlsx_formula_dump"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

python3 - "$ROOT" "$WORK/formulas.json" << 'PY'
import html, json, os, re, sys, zipfile
root, dest = sys.argv[1], sys.argv[2]
manifest = json.load(open(os.path.join(root, "fixtures/manifest.json"), encoding="utf-8"))
open_re = re.compile(rb"<((?:[\w.]+:)?)(f|formula|definedName)\b([^>]*)>", re.I)
formulas = []
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
    for name in zf.namelist():
        if not name.lower().endswith(".xml"):
            continue
        data = zf.read(name)
        if b"<f" not in data and b"definedName" not in data and b"formula" not in data.lower():
            continue
        for mo in open_re.finditer(data):
            attrs = mo.group(3)
            if attrs.rstrip().endswith(b"/"):
                continue
            prefix, tag = mo.group(1), mo.group(2)
            close = re.compile(rb"</" + re.escape(prefix) + re.escape(tag) + rb">", re.I)
            cm = close.search(data, mo.end())
            if not cm:
                continue
            raw = data[mo.end():cm.start()]
            try:
                text = raw.decode("utf-8")
            except UnicodeDecodeError:
                text = raw.decode("utf-8", "replace")
            text = html.unescape(text)
            if text.strip():
                formulas.append(text)
print(f"formula-parity: files={files} formulas={len(formulas)} skipped={len(skipped)}", file=sys.stderr)
for note in skipped:
    print(f"formula-parity: skip {note}", file=sys.stderr)
with open(dest, "w", encoding="utf-8") as fh:
    json.dump(formulas, fh, ensure_ascii=False)
    fh.write("\n")
if files == 0 or not formulas:
    print("formula-parity: no formulas extracted", file=sys.stderr)
    sys.exit(1)
PY

echo "formula-parity: rust dump"
"$DUMP" "$WORK/formulas.json" > "$WORK/rust.json"
echo "formula-parity: ts oracle"
bun "$ROOT/tools/model-compare/gates/ref/xlsx_formula_ref.ts" "$WORK/formulas.json" > "$WORK/ts.json"

compare() {
  local negative="${1:-}"
  bun - "$WORK/rust.json" "$WORK/ts.json" "$negative" << 'JS'
const [rustPath, tsPath, negative] = process.argv.slice(2);
const rust = JSON.parse(await Bun.file(rustPath).text());
const ts = JSON.parse(await Bun.file(tsPath).text());
if (!Array.isArray(rust) || !Array.isArray(ts)) {
  console.error("formula-parity: dump is not an array");
  process.exit(1);
}
if (negative === "1") {
  if (rust.length === 0) rust.push({ corrupted: true });
  else rust[0] = { ...rust[0], join: String(rust[0].join ?? "") + "~" };
}
if (rust.length !== ts.length) {
  console.error(`formula-parity: length rust=${rust.length} ts=${ts.length}`);
  process.exit(negative === "1" ? 1 : 1);
}
let bad = 0;
for (let i = 0; i < rust.length; i++) {
  const a = JSON.stringify(rust[i]);
  const b = JSON.stringify(ts[i]);
  if (a !== b) {
    bad++;
    if (bad <= 8 && negative !== "1") {
      const formula = ts[i]?.formula ?? rust[i]?.formula ?? "";
      console.error(`formula-parity: mismatch #${i} ${JSON.stringify(String(formula).slice(0, 180))}`);
      console.error(`  rust ${a.slice(0, 400)}`);
      console.error(`  ts   ${b.slice(0, 400)}`);
    }
  }
}
if (negative === "1") {
  if (bad === 0) {
    console.error("formula-parity: negative-control did not detect corruption");
    process.exit(0);
  }
  console.error(`formula-parity: negative-control detected ${bad} mismatch(es)`);
  process.exit(1);
}
if (bad !== 0) {
  console.error(`formula-parity: FAIL ${bad} mismatch(es)`);
  process.exit(1);
}
console.log(`formula-parity: PASS formulas=${rust.length}`);
JS
}

echo "formula-parity: compare"
compare 0

echo "formula-parity: negative control"
set +e
compare 1
nc=$?
set -e
if [ "$nc" -eq 0 ]; then
  echo "formula-parity: negative-control unexpectedly passed" >&2
  exit 1
fi
echo "formula-parity: negative-control FAIL as expected"
exit 0
