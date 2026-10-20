<!-- SPDX-License-Identifier: MIT -->
# Lazy loader chunks

Modules here are **not** part of the first-load JS budget (`perf/budgets.json`
`loader_js_brotli_kb`). `scripts/ci/bundle_budget.sh` skips this directory the
same way it skips `loader/test/`.

| Module | Loaded when |
| --- | --- |
| `worker-host-tick.js` | First `WorkerDocument.tick()` (cached). Outline-aware take-frame loop; does not pull glyph tracing until needed. |
| `outline-worker.js` | Worker `take-frame` / `outline-results` when `handle.outlineCount > 0`. |
| `glyph-outline.js` | A paint frame queues `OutlineBatch` probes (WordArt envelope warp). Host dynamic-`import()`s this only if `outlineCount > 0`. |
| `journal.js` | Editable `WorkerDocument.open` (`readonly: false`) and package `createJournal` import — crash-recovery op journal (#62 / #30), not on the readonly viewer path. |
| `ribbon/*.js` | Demo host dynamic-`import()`s `mountOffice` (title bar, ribbon tabs, slide pane, status bar, drop zone) around an element; uses only `shellAction`. See `docs/ux-revision.md`. |
| `ribbon/commands.js`, `dirty.js`, `thumbs.js` | Same host import: per-format ribbon manifest (only engine-mapped command ids), unsaved-changes tracking for writable xlsx, lazy slide thumbnails for pptx. |
| `embedded/*.js` | Shell element has `mode="embedded"` (dynamic `import()` from `ShellViewerElement._em`). Single-row simplified-ribbon toolbar; see `docs/embedded-mode.md`. |
| `preload.js` | Host `preload()` — compile+cache wasm, prewarm worker, `initCore` before open (React embed). |

Keep this directory for on-demand host work that must not grow the initial loader.
