# Long-session runner (#90)

Correctness-only harness: **1000** attempted edit operations per format (`xlsx`, `docx`, `pptx`), then save, reopen, and undo-parity checks. Memory and timing gates are intentionally omitted (`TODO(#90)` in `session.ts`).

## Run

From the repo root:

```bash
bash scripts/ci/long_session.sh
```

Or directly:

```bash
cd perf/runner && bun session.ts
```

Native harness only:

```bash
cargo run --release --manifest-path perf/runner/native/Cargo.toml
```

Fixtures:

- `fixtures/generated/xlsx/cells_1k.xlsx`
- `fixtures/generated/docx/painter_line_numbers.docx`
- `fixtures/generated/pptx/slides_10.pptx`
