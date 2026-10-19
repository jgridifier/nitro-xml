# Large-file time + memory baseline

Harness for the speed/memory workstream (large-xlsx first view first). Fixtures are
built on demand by `fixtures/gen/generate.py` — L/XL packages are **not** committed.

## Generate fixtures

```bash
python3 fixtures/gen/generate.py \
  --id gen-xlsx-cells-100k \
  --id gen-xlsx-cells-1m \
  --id gen-docx-pages-500 \
  --id gen-pptx-slides-200
```

## Native baseline (time + peak RSS)

```bash
cargo run --release --manifest-path perf/large/Cargo.toml -- \
  --xlsx-small fixtures/generated/xlsx/cells_100k.xlsx \
  --xlsx fixtures/generated/xlsx/cells_1m.xlsx \
  --docx fixtures/generated/docx/pages_500.docx \
  --pptx fixtures/generated/pptx/slides_200.pptx \
  --runs 7
```

Run one fixture per process when comparing RSS (VmHWM is process-lifetime).

## WASM open baseline

```bash
bash scripts/build-wasm.sh
node perf/large/wasm-baseline.mjs fixtures/generated/xlsx/cells_1m.xlsx 5
```
