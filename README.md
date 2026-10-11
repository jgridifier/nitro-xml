# Nitro XML

**A Rust → WebAssembly engine that views and edits Word (.docx), Excel (.xlsx) and PowerPoint (.pptx) files entirely in the browser.** Bytes never leave the page. Nitro XML is a port of [ChristopherVR/ooxml](https://github.com/ChristopherVR/ooxml) (Apache-2.0), and its goal is to be the fastest, most efficient, highest-quality OOXML renderer and editor available, beating the original TypeScript engine on every format, at every file size.

**Status: planning, Stage 0 not started.** No engine code exists yet.

- 📄 **Plan:** https://jgridifier.github.io/nitro-xml/ ([docs/PLAN.md](docs/PLAN.md))
- 📐 **Product spec and gates:** [docs/PRODUCT_SPEC.md](docs/PRODUCT_SPEC.md) (source of every parity, perf, bundle, accessibility and editor gate)
- 🗂️ **Board:** https://github.com/users/jgridifier/projects/2 (view **Ready now** for claimable work)

## What it is

- **Full Rust/WASM.** Package (zip), XML streaming, OPC, the document models, layout, rendering, the editing surface (text input, selection, caret, ribbons), commands, undo/redo and the lossless writer are all Rust compiled to WebAssembly.
- **Nothing in TypeScript ships.** The only JavaScript is a tiny loader (WASM bootstrap and browser event forwarding) with its own size budget and gate.
- **The upstream TS engine stays in the test and bench harness only**, pinned at commit `e66aaec`, as the correctness reference (parity oracle) and the speed baseline.
- One npm package (`nitro-xml`) and Rust crates `nitro-xml-opc`, `nitro-xml-xlsx`, `nitro-xml-docx`, `nitro-xml-pptx`, `nitro-xml-wasm` and the umbrella `nitro-xml`.

## Goals

1. **Fidelity:** output matches the TS reference (view-IR parity and pixel diffs), and saves round-trip losslessly.
2. **Speed and memory:** WASM beats TS on every format, every size tier (S, M, L, XL) and at 1x and 4x CPU: first paint (including WASM download and init), cold parse, scroll and peak memory. Pass means a lower median **and** a 95% CI of the WASM/TS ratio entirely below 1.0; a tie fails. Typing, undo and save must beat TS too.
3. **Efficient by design:** a single streaming pass from zip to model with no full DOM; lazy per-sheet, per-page and per-slide parsing; work in a worker; bulk Rust-to-JS transfer; SIMD on for hot loops; streaming compile, cached module and a pre-warmed worker; a small core module with each format loaded lazily; per-format bundle ceilings that only go down; a startup-time gate.
4. **Accessible and international:** real selectable text, screen-reader and keyboard access, IME/OS text input, bidirectional text and spellcheck.
5. **Honest public scoreboard:** every commit publishes WASM vs TS, plus races against the best existing alternatives on the same corpus. Losses are published as prominently as wins.

## Non-goals

- Running macros (never). VBA and other unknown parts are preserved byte for byte.
- Visio, Teams and MCP from upstream.
- A TypeScript runtime fallback. If WASM cannot run (old browser, CSP blocks wasm, init failure), the component shows an "unsupported browser" message.
- Framework bindings beyond the web component and the headless API (for now).
- Real-time collaboration and PDF/image export (open questions, default out).

## Milestones

- **Milestone 1, viewer:** Stage 0 (scaffolding), Stage 1 (perf harness, TS baseline, scoreboard), Stage 2 (shared OPC, streaming zip and XML, boundary), **R0 (renderer design with Jared:** a DOM-vs-canvas spike on the Excel grid, text shaping and fonts, the worker split and the loader size budget; no renderer issue is claimable before R0 closes), then the Excel (3), Word (4) and PowerPoint (5) viewers, the Rust viewer shell (6) and hardening (7, the M1 gate).
- **Milestone 2, editor (later phase):** E0 (editor harnesses, TS editor baseline, Rust editor chrome), **E-text** (the Rust text-editing surface: single-line before E1, rich text before E3; IME, bidi, spellcheck, accessibility and crash recovery each with their own gate), E1 (xlsx edits and lossless save), E2 (Rust formula engine), E3 (docx), E4 (pptx), E5 (write-path hardening). Editor issues are labelled `blocked-m2` until the M1 gate passes and Jared confirms the budget.

## Decisions (defaults, open to change)

Jared has not answered every open question in the plan, so these are the plan's recommendations plus his later calls. Comment on the board or an issue to change any of them.

| # | Decision | Current call |
|---|---|---|
| D1 | Repo shape | New repo, not a fork. Upstream pinned at `e66aaec24adc0b68472eae1fc42aa4598e09c352` as the parity oracle and algorithm source |
| D2 | Licence | MIT; files ported or derived from ooxml stay Apache-2.0 (see below) |
| D3 | TS engine | Test and bench harness only (reference and baseline). No runtime TS, no fallback |
| D4 | Scope | Full editor suite, viewer first (Jared) |
| D5 | Format order | xlsx, then docx, then pptx |
| D6 | Perf browser | Measured in Chromium; WebKit and Firefox get functional smoke tests |
| D7 | Perf CI cadence | A small perf corpus on every PR; the full corpus nightly and on demand. Every commit posts its numbers |
| D8 | Legacy and encrypted files | `.doc/.xls/.ppt`, password-protected files and CSV show an "unsupported format" message in v1; a Rust track after E5 (open question) |
| D9 | Renderer approach | DOM vs canvas is **Jared's design decision** ([#33](https://github.com/jgridifier/nitro-xml/issues/33), R0, informed by spikes #34 to #36); every renderer must give real selectable text, screen-reader and keyboard access, IME/OS text input, bidi and crisp high-DPI output |
| D10 | Viewer edits | [#29](https://github.com/jgridifier/nitro-xml/issues/29) is a placeholder awaiting Jared's list |
| D11 | Crate layout | `nitro-xml-opc` holds the shared zip, XML, OPC and DrawingML modules; format crates on top; `nitro-xml-wasm` is the only wasm-bindgen crate |
| D12 | Threads | None by default: single-threaded wasm plus one worker per document, no COOP/COEP needed by embedders |

## How to contribute

Work is split into agent-sized issues so Cursor, Claude Code, Codex and OpenCode agents (and human Developers) can run in parallel. The contract is **[AGENTS.md](AGENTS.md)**:

1. Pick an unassigned `status:ready` issue whose dependencies are closed (board view **Ready now**).
2. Comment `claiming`, then assign yourself or add `agent:<name>`.
3. One branch per issue, `issue-<N>-<slug>`; a small PR with `Closes #N`.
4. Run the tests and the perf gate locally (`scripts/ci/*.sh`).
5. Don't touch files outside the issue's scope, and never edit `.github/workflows/`.
6. Post a handoff comment when you finish: what changed, what's left, gotchas.

See also [CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

Nitro XML is MIT-licensed. Files ported or derived from ooxml remain under Apache-2.0 (see [NOTICE](NOTICE), [LICENSE-THIRD-PARTY](LICENSE-THIRD-PARTY) and [PROVENANCE.md](PROVENANCE.md)).
