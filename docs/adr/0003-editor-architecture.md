<!-- SPDX-License-Identifier: MIT -->
# ADR 0003: Editor architecture — commands, transactions, history, writer, journal (#47)

**Status:** Proposed (E0 lock for E1 issues #63–#69).  
**Date:** 2026-10-11  
**Depends on:** PRODUCT_SPEC.md §5 (a)–(e) and §6; boundary (`docs/boundary.md`); renderer ADR (`docs/adr/0002-renderer-port.md`); upstream pin ChristopherVR/ooxml @ `e66aaec`.  
**Issue path note:** #47 listed `docs/adr/0004-editor.md`; this file is `0003-editor-architecture.md` (next free ADR number after 0002).

## Context

M2 ships the editor entirely in Rust/WASM. The upstream TS editor is the **test-only oracle** for model results (edit scripts #50, undo parity #51). No ProseMirror, Lit, or TS UI ships. This ADR maps how upstream actually edits today, then locks the Rust module layout E1 issues claim against.

Upstream is **three parallel stacks**, not one shared engine:

| Format | Unit of undo | Incremental signal | Save |
|---|---|---|---|
| **xlsx** | `HistoryStep` of scoped before/after snapshots (`edit/history.ts`) | `WorkbookChange` → grid invalidate + one rAF paint; calc via `touchedCells` | Regenerate modeled parts; `PackageWriter.carry` copies the rest |
| **docx** | ProseMirror history (`closeHistory` isolates explicit commands) | PM transaction → view update | Patch ZIP from remembered `PackageContext`; identical model → **original bytes** |
| **pptx** | `EditorHistory` of **pre-mutation** slide-array snapshots | Per-slide `isDirty` / fingerprint | Skip untouched slides; re-serialize dirty ones |

There is **no op journal** in upstream core (pptx IndexedDB autosave is full-deck snapshots). Nitro invents the journal in the loader (#62).

## Decision summary

1. **One shared edit core** in `nitro-xml-opc` (transactions, change-sets, history), with format crates owning commands and writers.
2. **Command registry** matches upstream’s xlsx `Command` / `CommandRegistry` shape and is shared by ribbon, shortcuts, and the headless API (#53).
3. **Transaction** = one atomic `run` / `batch` that records scopes into a history step and emits a **change-set** for incremental layout/paint.
4. **Undo/redo** reproduces the TS oracle per format (#51 / #64); E1 locks xlsx semantics first.
5. **Writer** rewrites only dirty / modeled parts; untouched parts (and unknown markup) pass through **byte for byte**, aiming to beat `parity/roundtrip/ts-losses.json` (#65).
6. **Op journal** lives in the loader (#62), not in WASM linear memory.
7. **Edits cross the JS/WASM boundary** like paint: ≤1 event batch in and ≤1 command buffer out per frame (`docs/boundary.md`).

## Upstream → Nitro map (xlsx is the E1 template)

### Commands (shared with chrome #53)

Upstream (`src/core/xlsx/ui/commands.ts`):

```ts
Command { id, label, icon?, shortcut?, editing?, enabled?, checked?, value?, run(ctx, arg?) }
CommandRegistry { register, registerAll, get, list, run, isEnabled }
```

Ribbon / keyboard call `registry.run(id)`; `run` emits `ribbon-action`, awaits `command.run`, then `requestRender()`.

**Nitro:** same id scheme (`home.bold`, …) and registry API in Rust. Chrome (#53) and headless `Handle` share one registry instance per document.

| Concern | Upstream | Nitro module |
|---|---|---|
| Registry + types | `xlsx/ui/commands.ts` | `nitro-xml-wasm/src/chrome/commands.rs` (#53) |
| Ribbon / shortcuts | `ui/src/xlsx/ribbon/`, `keyboard.ts` | `nitro-xml-wasm/src/chrome/{ribbon,shortcuts}.rs` (#53) |
| Format command bodies | `xlsx/ui/commands/*.ts` → `EditSession` | `nitro-xml-xlsx/src/edit/{cells,structure,…}.rs` (#66–#67); later docx/pptx |

**Deliberate differ:** registry and chrome are Rust/WASM, not Lit. Command **ids and enabled/checked semantics** stay oracle-aligned so edit scripts and ribbons stay comparable.

### Transactions and change-sets

Upstream has **no type named `Transaction` / `ChangeSet`** on xlsx. The real contract is:

- `EditContext.run(label, kind, scopes, fn)` / `EditSession.batch(label, fn)` — one undoable step; nested batches flatten (outer label wins).
- `EditScope` — what to snapshot (`cells` / `parts` / `sheet` / `refs` / `shift` / `meta` / `workbook`).
- `WorkbookChange { kind, label, sheet?, ranges?, structural, … }` — notification after commit (`kind` includes `batch` / `undo` / `redo`).

**Nitro names (acceptance language of #63):**

| Term | Meaning |
|---|---|
| **Transaction** | Open step: capture scopes → mutate → capture after → push history **or** rollback on error |
| **Change-set** | Stable summary after commit: dirty ranges / parts / structural flag / calc hint — feeds paint and docx incremental relayout (#81) |

| Concern | Upstream | Nitro module |
|---|---|---|
| Session + run/batch | `xlsx/edit/{session,context,types}.ts` | `nitro-xml-opc/src/edit/{mod,transaction,change_set}.rs` (#63) |
| WASM entry | Lit `EditorCore` | `nitro-xml-wasm/src/edit.rs` (#63) |
| xlsx cell / structure ops | `xlsx/edit/*.ts` | `nitro-xml-xlsx/src/edit/{cells,structure}.rs` (#66, #67) |
| Docx incremental layout consumer | (PM view) | `nitro-xml-docx/src/layout/incremental.rs` (#81) — consumes change-sets, must match full relayout |

**Deliberate differ:** one shared transaction/change-set core for all formats. Docx/pptx adaptors translate their model mutations into the same step + change-set shape so #50/#51 harnesses stay uniform. We do **not** ship ProseMirror; rich text uses the E-text surface.

### Undo / redo (#51, #64)

xlsx rules that #64 must match (from `history.ts` / `session.ts`):

1. One outer `run`/`batch` → one `HistoryStep`; empty entries → nothing pushed.
2. `push` appends to `done`, drops past `historyLimit` (default **100**), **clears redo**.
3. Undo/redo restore scoped snapshots **in place** (sheet object identity preserved).
4. **Typing coalescing is UI-side only** (`EditBridge` buffers; commit = one `setCellInput` step). Consecutive commits do **not** merge in `History`.
5. **Selection is not stored** on xlsx steps — undo leaves the UI selection alone.
6. Failed `run`/`batch` rolls back and records nothing.
7. Save / dirty-clear does **not** clear history.

docx: explicit commands call `dispatchIsolatedCommand` → `closeHistory` so they stay separate from typing. pptx: pre-mutation full snapshots, `maxDepth` 100 (class) / 120 (Vue UI).

| Concern | Upstream | Nitro module |
|---|---|---|
| History stack | `xlsx/edit/history.ts` | `nitro-xml-opc/src/edit/history.rs` (#64) |
| Undo-parity gate | (oracle) | `scripts/ci/undo_parity.sh` (#51) |

**Deliberate differ:** one history implementation with format-specific snapshot codecs. E1 gates against **xlsx** TS behavior; docx/pptx parity arms when those command sets land. Selection-restore policy for rich text is an E-text decision — xlsx stays “no restore” for oracle match.

### Lossless writer (#65, #68)

Upstream xlsx: regenerate workbook/sheets/styles/strings/theme/…; `carry` copies still-referenced source parts; signatures not carried; `calcChain` dropped from regenerated rels. Docx: no-op model → original bytes. Pptx: skip clean slides.

**Nitro strategy (stricter pass-through):**

1. Keep the source package index on the handle (as TS `Workbook.source`).
2. Track **dirty parts** from change-sets; rewrite only those (plus mandatory dependents: e.g. shared strings / styles when cells change).
3. Untouched parts, `extLst`, `mc:AlternateContent`, unknown parts, and **VBA** (`vbaProject.bin` + signature + data + rels/content-types) pass through **byte for byte**.
4. No-op save must pass gate (a) and be **never worse** than `parity/roundtrip/ts-losses.json`; ratchet the inventory down when we beat TS.

| Concern | Upstream | Nitro module |
|---|---|---|
| Shared OPC write / carry | `xlsx/write/package-writer.ts` | `nitro-xml-opc/src/write/` (#65) |
| xlsx workbook writer | `xlsx/write/save.ts` + parts | `nitro-xml-xlsx/src/write/` (#68) |

**Deliberate differ:** prefer carry-over-regenerate whenever the model did not touch a part (TS often regenerates theme/docProps and loses identity). That is how we beat the inventory without changing edit semantics.

### Op journal / crash recovery (#62)

**Upstream:** none in the edit engine.  
**Nitro (PRODUCT_SPEC §5(e)):**

- Loader keeps `originalBytes` + append-only **op journal** (stable command ids + args, same shape as `edits/*.json` / headless API).
- On WASM crash during edit: fresh worker → reload originals → replay journal; if replay fails, offer last good in-memory save blob.
- Journal is **loader JS** (`packages/nitro-xml/loader/journal.ts`), counted in the loader budget — not a second TS engine.

| Concern | Module |
|---|---|
| Journal + replay orchestration | `packages/nitro-xml/loader/journal.ts` (#62) |
| CI crash injection | `scripts/ci/crash_recovery.sh` (edit arm, #62) |

### JS/WASM boundary for edits

Same rules as paint (`docs/boundary.md`, ADR 0002):

- Loader coalesces input → **≤1** `pushEventBatch` per frame.
- Rust commits transactions in the worker; encodes chrome + document paint into **one** command buffer; loader `applyCommandBuffer` once on main.
- No per-keystroke or per-cell WASM→JS calls. This is what keeps the loader under budget and satisfies the R0 port rule for editor chrome (HoP: chrome shares the buffer).

## Module and file map (claim table for #63–#69)

Paths are Nitro unless marked upstream.

| Issue | Owns | Responsibility |
|---|---|---|
| **#53** | `rs/crates/nitro-xml-wasm/src/chrome/{commands,ribbon,shortcuts}.rs` | Registry shape + ribbon/shortcut wiring; readonly hides chrome; lazy editor module |
| **#63** | `rs/crates/nitro-xml-opc/src/edit/` · `nitro-xml-wasm/src/edit.rs` | Transaction, change-set, document handle apply/rollback; wasm entry |
| **#64** | `rs/crates/nitro-xml-opc/src/edit/history.rs` | Undo/redo matching TS (grouping, coalescing boundary, depth, selection policy) |
| **#65** | `rs/crates/nitro-xml-opc/src/write/` | OPC package writer: dirty rewrite + byte carry; VBA byte-identity; beats `ts-losses.json` |
| **#66** | `rs/crates/nitro-xml-xlsx/src/edit/cells.rs` | Cell value / number format / style; shared strings / style dedupe |
| **#67** | `rs/crates/nitro-xml-xlsx/src/edit/structure.rs` | Row/col size/hide; sheet add/rename/delete/reorder + ref updates |
| **#68** | `rs/crates/nitro-xml-xlsx/src/write/` | xlsx save: dirty parts, `fullCalcOnLoad`, stale dependents |
| **#69** | `rs/crates/nitro-xml-xlsx/src/editor/` · `<nitro-xlsx>` edit mode | Cell editor, formula bar, ribbon wiring, stale marking; arm xlsx edit perf |
| **#62** | `packages/nitro-xml/loader/journal.ts` | Op journal + replay (gate e) |
| **#51** | `scripts/ci/undo_parity.sh` (+ harness) | Gate (c) oracle |
| **#81** | `rs/crates/nitro-xml-docx/src/layout/incremental.rs` | Change-set → incremental re-pagination (E3; must ≡ full relayout) |

Shared crates (already decided elsewhere):

- `nitro-xml-render` — command buffer for document **and** chrome paint (ADR 0002).
- `nitro-xml-wasm` — handle, event batches, edit dispatch, chrome.
- Format crates — model + format commands + format writers + painters.

## Deliberate differences (checklist)

| Differ from upstream | Reason |
|---|---|
| Single shared edit core vs three TS stacks | One harness (#50/#51), one journal schema, less E2–E4 drift |
| No ProseMirror / Lit / Vue | Design lock: only loader JS ships |
| Named change-sets feeding layout | Docx #81 and xlsx stale/paint need a stable dirty summary; TS implied this via `WorkbookChange` + grid signatures |
| Stricter byte carry on save | Gate (a): never worse than TS; beat `ts-losses.json` |
| Op journal in loader | Gate (e); WASM crash must not lose the journal |
| ≤1 frame batched handoff for edit→paint | Loader budget + R0 rule; TS paints in-process DOM |

## Out of scope / follow-ups

- E-text surface (caret, IME, bidi, clipboard, spellcheck) — #55–#61; single-line before E1, rich text before E3.
- Docx/pptx command bodies and writers — E2–E4; they plug into this core.
- Collab / Yjs — upstream has paths; Nitro does not claim them in E0–E1.
- Implementation code — this ADR is docs only.

## Consequences

- #53 and #63 can proceed from this map without inventing parallel registries or transaction types.
- #64 treats xlsx `History` semantics as the E1 oracle; format deviations need an explicit note in that PR.
- #65/#68 treat `ts-losses.json` as a ceiling, not a target to match.
- #62’s journal records **command ids + args** at the registry boundary so replay does not depend on DOM.
