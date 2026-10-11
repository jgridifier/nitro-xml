<!-- SPDX-License-Identifier: MIT -->
# painter-calls

This tool counts the DOM and canvas-2D calls upstream's TypeScript painters make per animation frame, broken down by the rows of [docs/painters.md](../../docs/painters.md) (#34). It is a measurement tool for the R0 port record. It is not shipped, is not part of any CI job, and has no npm dependencies of its own: Vite and Playwright are loaded from the upstream checkout you point it at.

## Run

You need a scratch copy of upstream at the pinned commit `e66aaec` with `bun install` done. Never use the `upstream/` submodule; the tool reads the copy you give it but does not modify it. You also need system Chrome.

```bash
cp -a /workspace/ooxml-src /workspace/ooxml-measure      # once; never measure in the original
node tools/painter-calls/measure.mjs --upstream /workspace/ooxml-measure --out tools/painter-calls/results.json
```

Options:
- Pass format names (`xlsx docx pptx`) to run only those formats.
- `--chrome PATH` selects a browser binary (default `/usr/bin/google-chrome`).

The script prints one line per scenario and writes `results.json`.

## What it does

1. **Serve upstream unbundled.** It serves `harness/` with Vite, resolving `ooxml-core/*` and `ooxml-ui/*` to upstream's TypeScript sources (the same mapping as upstream's `OOXML_CORE_SRC` aliases). Because modules are not bundled, every stack frame names an upstream file.
2. **Mount the painters.** `harness/main.ts` mounts:
   - `<xlsx-editor>` and `<docx-editor>` from `src/ui/src/{xlsx,docx}`;
   - for pptx, `loadPresentation` and `renderSlideStage` from `src/ui/src/pptx/dom`, called the way upstream's vanilla viewer calls them (it rebuilds the whole stage on every slide change).
3. **Instrument the DOM.** `instrument.js` is injected before any page script runs. It wraps:
   - node creation: `createElement`, `createElementNS`, `createTextNode`, `importNode`, `cloneNode`, `DOMParser.parseFromString`;
   - tree changes: `appendChild`, `insertBefore`, `removeChild`, `replaceChild`, `append`, `prepend`, `replaceChildren`, `remove`, `before`, `after`, `replaceWith`, `insertAdjacent*`;
   - attributes: `setAttribute`, `setAttributeNS`, `removeAttribute`, `toggleAttribute`, and the `className` and `hidden` setters;
   - `classList` changes: `add`, `remove`, `toggle`, `replace`;
   - text and markup: the `textContent`, `innerHTML` and `innerText` setters;
   - inline style: `setProperty`, `removeProperty`, the `cssText` setter and every CSS property setter on the style object, so `el.style.left = ...` and `Object.assign(el.style, ...)` are counted;
   - canvas 2D: every `CanvasRenderingContext2D` and `OffscreenCanvasRenderingContext2D` method and state setter.
4. **Attribute each call.** Each call is charged to a row by walking its stack from the innermost frame outward. The first upstream file listed in `rows.json` wins. Unlisted files (node_modules, layout engines) are skipped and the walk continues. The generic helpers listed as `DEFER` in `measure.mjs` (element factories and text measurers) are charged to their caller instead.
   - Calls whose stack reaches a file classified as excluded (chrome, editor or shell) are reported under that exclusion.
   - Calls whose stack has upstream frames but no classified file are reported as `other`, and the innermost of those files are listed.
   - Calls from the harness alone are reported as `harness`.
5. **Bucket by frame.** A frame is everything between two ticks of a `requestAnimationFrame` loop that the init script starts before upstream code.
6. **Run the scenarios:**
   - **first paint after open:** from `load()` (or the first `renderSlideStage`) until 60 frames pass with no call;
   - **steady scroll:** for xlsx and both docx views, the largest scrollable box is scrolled a fixed number of pixels per frame for 60 frames. The first 5 frames are dropped as warm-up, and the median is over every remaining frame, including frames where the row made no call.
   - **docx Print Layout:** its first paint, triggered by clicking the status bar's Print Layout button;
   - **pptx slide change:** slides 2 to 11, one scenario each.

`rows.json` maps each of the 1,462 enumerated upstream files to its painter row or exclusion. It was generated from the coverage classifier in `docs/painters.md`. Regenerate it whenever that classifier changes.

## Fixtures

No tier manifest exists yet; #4 will replace these choices. Each stand-in is a medium file of typical content from `fixtures/real/`:

| Format | File | Size | Why |
|---|---|---:|---|
| xlsx | `poi-StructuredRefs-lots-with-lookups.xlsx` | 325 KB | Five sheets and 38k cells. The active sheet ("List", 3.7k cells) has a structured table and 11 conditional-format rules, so cells, conditional formats, gridlines, headers and selection all paint. |
| docx | `poi-drawing.docx` | 95 KB | 227 paragraphs, 12 tables and 10 pictures over several pages. That is typical report content (just under 100 KB; the only larger docx files are image-only samples or a 15,935-paragraph stress file). |
| pptx | `poi-60810.pptx` | 875 KB | A 28-slide deck with 65 shapes and 19 pictures, typical slide content. |

## Limits

- **What counts as a call.** The tool counts calls, not cost. A string parsed into the DOM (`DOMParser`, `innerHTML`, `importNode`) is one call however many nodes it creates.
- **Not counted:**
  - `dataset` writes and other reflected properties (`src`, `title`, `id`);
  - geometry reads (`getBoundingClientRect`);
  - `requestAnimationFrame` itself.
- **Overhead.** Instrumentation slows the page down, because capturing a stack on every call is expensive, so frame boundaries are not real-time faithful. Work that runs in one paint callback stays in one frame, so per-paint counts are exact. Work spread over timers may span more frames than it would uninstrumented.
- **Default view options only.** Opt-in paths (pptx 3D, slide show, export, collaboration) are not driven, so their rows read "not exercised by fixture".
