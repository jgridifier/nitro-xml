<!-- SPDX-License-Identifier: MIT -->
# Scoreboard method (#39, #40)

The public scoreboard at <https://jgridifier.github.io/nitro-xml/scoreboard/> compares Nitro XML with the upstream ooxml TypeScript engine and with pinned third-party libraries. The harness lives in `perf/scoreboard/`. It is test and benchmark code only and is never shipped. Maintainers of any listed library are welcome to send corrections (version, configuration, adapter) by pull request against `perf/scoreboard/competitors.json` or the adapter.

## Races

| Race | Status |
|---|---|
| **Parse** | Live. Bytes in memory, then the entrant's parse call returns a model. |
| **Render to first paint** | Live for **xlsx** and **pptx** (#41): `perf/scoreboard/races/render/`. Times load+init+first paint; ranks only within PRODUCT_SPEC §5 pixel limits vs upstream TS. **docx** deferred (TODO(#23)). |
| **Open, edit, save** | Not run yet. Needs the editor (#71). |

## Fairness rules

- **Same machine and browser.** Every entrant runs on the same machine in the same headless Google Chrome build, through `playwright-core`. The page is cross-origin isolated (COOP/COEP), so `performance.now()` has full resolution. The machine, CPU, core count, RAM and Chrome version are recorded in every result file.
- **Pinned versions.** Each entrant uses an exact version (npm or crates.io version, or a git SHA), listed with its licence and upkeep notes in `competitors.json`. The upstream reference is pinned at `e66aaec`.
- **Fastest documented configuration.** Each entrant runs in its fastest documented configuration (`fastestConfig` in `competitors.json` gives the setting, why it was chosen and a doc link). Rust competitors are built for `wasm32-unknown-unknown` with release, `opt-level=3`, LTO, `codegen-units=1` and `wasm-opt -O3`, using the same toolchain as Nitro XML.
- **What is timed.** The timer covers only the parse call. The bytes are fetched first, and each run gets a fresh copy made outside the timed span. `gc()` runs before every run. Each (file, entrant) pair gets 2 warm-up runs and 7 timed runs in a fresh browser context and page. Entrants run in a seeded random order per file. Where a library has no parse-only API, its timing scope is stated on the page; PPTXjs, for example, includes HTML generation and DOM insertion.
- **Memory.** Memory figures are approximate: JS heap (`performance.memory`) before and after the last timed parse with the model retained, plus wasm linear memory. They are not peak measurements.
- **Who is not entered.** Entrants that do not build, cannot be pinned, or have no comparable boundary are shown as **not entered**, with the reason.

## Equivalence

A speed number is ranked against the reference only when the entrant's output equals upstream's for that file (`perf/scoreboard/lib/normalise.mjs`). Otherwise the file counts as "not comparable", and the first difference is stored in the result file.

| Format | What is compared | Normalisation |
|---|---|---|
| xlsx | Every non-empty cell as (sheet, A1 ref, type `n`/`s`/`b`/`e`, value) | Dates become 1900-system serials. Numbers are equal within 1e-9 relative. |
| docx | Body and table-cell paragraph texts, in order | Whitespace runs are collapsed, NFC. Empty paragraphs are dropped. |
| pptx | Per slide, paragraph texts in shape order | Same as docx. |

Ratios on the page are the geometric mean of (entrant median / upstream median) over the files where both ran and the entrant was comparable. Each ratio shows the file count it is based on.

## Corpus

The parse race uses the 90 real files in `fixtures/real/{xlsx,docx,pptx}/`, with provenance in their `SOURCES.md`. Generated size tiers are added when #4 lands.

## Reproduce

```bash
cd perf/scoreboard && npm ci
bash adapters/rust/build.sh      # calamine, umya-spreadsheet, docx-rs shims -> dist/rust
node build.mjs                   # bundles adapters, fetches pinned PPTXjs (sha256-checked)
node run.mjs                     # writes results/parse/<date>-<sha>.json and docs/scoreboard/data/parse-latest.json
```

`build.mjs` needs the upstream submodule built: `cd upstream && bun install && bun run build`. Options: `--runs`, `--warmup`, `--seed`, `--format`, `--entrants`, `--files`.

## Known caveats in the first published run (2026-10-11)

- **Loaded machine.** The run was on the shared 8-core development box while other jobs were compiling, so absolute times are noisy. Rankings were consistent file by file (see W/L counts). CI runs on a quiet runner will replace it.
- **LibreOffice WASM is not entered.** It can only be pulled from an unpinned `latest` URL, it is about 53 MB, and it has no parse-only boundary. It is planned for the first-paint race with a self-built, pinned build.
- **Evaluated but not entered.** `@silurus/ooxml`, shrimp-doc-viewer and pptx-preview are listed in `competitors.json` under `evaluated`, with the reasons. **BetterOffice** (`@betteroffice/docx@0.4.3`) is now entered in the docx parse race (`parseDocx`, opc+parse wasm only).
