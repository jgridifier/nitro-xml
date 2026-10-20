<!-- SPDX-License-Identifier: MIT -->
# React embedding latency

Baseline numbers from `perf/react-embed` (Vite + React 18, `mode="embedded"`).
Re-run with `cd perf/react-embed && npm install && npm run bench` after a
`bash scripts/build-wasm.sh`. No budget or CI gate yet — measurement harness only.

Machine: cloud agent VM, Chromium headless (Playwright), cold page per scenario.

## Startup (cold)

| Step | ms |
|---|---|
| WASM fetch | ~2 |
| `compileStreaming` | ~52 |
| `instantiateStreaming` | ~12 |
| Worker construct | ~0.1 |
| `initCore` (fetch+compile+instantiate) | ~80 |

Streaming compile and instantiate are used by the loader (`bootstrap.js`).
`mode="embedded"` also kicks `lazy/preload.js` (wasm compile + worker prewarm +
`initCore`) as soon as the attribute is set, so compile overlaps React mount.
Hosts can still call `preload()` earlier from route layout for maximum overlap.

## Mount → first paint

| Format | Tier | open ms | paint ms | mount→paint ms | bytes |
|---|---|---|---|---|---|
| xlsx | S | 225 | 35 | 260 | 8 012 |
| xlsx | M | 416 | 70 | 485 | 503 874 |
| docx | S | 119 | 20 | 139 | 2 945 |
| docx | M | 162 | 12 | 175 | 403 491 |
| pptx | S | 126 | 8 | 134 | 11 993 |
| pptx | M | 129 | 7 | 136 | 442 863 |

## Edit → repaint (p50 / p95)

| Interaction | p50 ms | p95 ms | n |
|---|---|---|---|
| xlsx cell (`setCellInput`) | 24.9 | 33.3 | 20 |
| docx type (`editorKey`) | 16.6 | 16.9 | 20 |
| pptx type (`editorKey`) | 16.6 | 16.9 | 20 |
| bold (`shellAction cmd home.bold`) | 25.3 | 56.4 | 10 |

## Remount / StrictMode

| Scenario | Result |
|---|---|
| Remount same xlsx S | first 205 ms → second 89 ms (Δ −116 ms; module already compiled) |
| React 18 StrictMode double-mount | `host_ready_count=2`, gap ~0.9 ms; mount→paint ~149 ms |

## Planned cut PRs (before → after tables land here)

1. Streaming compile path polish + Cache API module persistence
2. Host `prewarm()` / preload API (worker + wasm before file open)
3. Reuse one worker across mounts; coalesce edits per animation frame; clean unmount (no leaks)

Stay out of xlsx grid paint (separate track). No budget, threshold, or fixture changes in those PRs.
