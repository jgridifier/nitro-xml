# Fixture generators

Deterministic generators for the generated fixtures in `fixtures/generated/` (fixed seed 4, fixed zip
timestamps and member order; two runs are byte-identical). Build with `python3 fixtures/gen/generate.py
--all` (or `--id <recipe>`); `fixtures/verify` checks every sha256, regenerating on-demand files.

## Tiers

S < 100 KB, M < 1 MB, L < 10 MB, XL >= 10 MB (compressed package size).

- **S / M** (`tiers.py`): committed; a few use an unreferenced `pad.bin` as a size placeholder.
- **L / XL** (`workloads.py`, on demand, gitignored): size comes from real content that the parser and
  renderer must process. There is **no `pad.bin`** in any L/XL package (#117).

| format | L / XL content |
| --- | --- |
| xlsx | 1-5 sheets of 20-column rows (shared-string, integer and decimal cells), a shared-string table of 20k-250k entries; sheet 1 keeps the `dimension`, merge and colour-scale rules |
| docx | 1,200-7,000 page-break-separated sections of styled paragraphs (with bold runs), a 5x4 table every 4th page, an inline PNG every 10th page cycling a pool of distinct noise PNGs (each rel-referenced) |
| pptx | 1,000-6,000 slides, each with a title, six text paragraphs and a picture relationship into a pool of distinct noise PNGs |

Text is drawn from a 4,096-word pseudo vocabulary; numbers and picture bytes come from
`random.Random(f"4:<recipe-id>:<part>")`. Exact counts are in each recipe's `content` dict and the
manifest `features` (`cells-N`, `sheets-N`, `pages-N`, `slides-N`, `images-N`).

File names keep their legacy tier labels (`cells_1m`, `pages_500`, `slides_200`, `xl_*`) because perf,
parity and `.gitignore` reference them; the names do not state the content counts.

Gates: the Stage 1 WASM-beats-TS gate, the scoreboard and #8 may arm against L/XL now that this has
landed; they must not use the older pad-based L/XL packages. To add an intentionally orphaned part, use a
separate fixture with `features: ["stress:orphan-part"]`.
