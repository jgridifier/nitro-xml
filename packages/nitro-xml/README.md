<!-- SPDX-License-Identifier: MIT -->
# nitro-xml (npm)

Open, view, edit and save `.xlsx`, `.docx` and `.pptx` in the browser via
Rust/WebAssembly. This package ships the **WASM binary**, the **tiny loader**
(eager first-load budget ≤ 17 408 B brotli) and **TypeScript types**.

## Install

```bash
npm install nitro-xml
# React binding (separate package; does not grow the eager loader):
npm install @nitro-xml/react react react-dom
```

Names are recommended in [`docs/PUBLISHING_NAMES.md`](../../docs/PUBLISHING_NAMES.md);
nothing is published until Jared registers the scope and runs a release.

## ESM + lazy WASM

```js
import {
  wasmSupported,
  initCore,
  defineNitroXlsx,
  defineNitroDocx,
  defineNitroPptx,
} from "nitro-xml/loader";

if (!wasmSupported()) throw new Error("WebAssembly required");
await initCore(); // compileStreaming / instantiateStreaming; Module cached
defineNitroXlsx();
defineNitroDocx();
defineNitroPptx();
```

- Default import `"nitro-xml"` re-exports the loader (tree-shakeable; side-effect
  free except `loader/element.js` / `loader/index.js` when you call `define*`).
- WASM bytes load only when `initCore` / `WorkerDocument.open` run — not at
  module evaluate time.
- Advanced: `"nitro-xml/wasm"` is the raw wasm-bindgen glue; `"nitro-xml/nitro_xml_wasm_bg.wasm"` is the binary URL for bundlers.

## React

```tsx
import { useRef, useState } from "react";
import { NitroViewer, type NitroViewerHandle } from "@nitro-xml/react";

export function Editor({ file, fileName, persist }: {
  file: File | Uint8Array;
  fileName: string; // e.g. report.docx, budget.xlsx or slides.pptx
  persist: (bytes: Uint8Array) => Promise<void>;
}) {
  const ref = useRef<NitroViewerHandle>(null);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState("");
  async function save() {
    try {
      if (ref.current) await persist(await ref.current.save());
    } catch (e) { setError(String(e)); }
  }
  return <>
    <button disabled={!ready} onClick={save}>Save</button>
    <p role="alert">{error}</p>
    <NitroViewer key={fileName} ref={ref} file={file} fileName={fileName}
      editable mode="embedded" style={{ height: 600 }}
      onReady={() => setReady(true)} onError={e => setError(e.message)} />
  </>;
}
```

For Vite production builds, keep dynamically imported worker chunks as ES modules:

```js
// vite.config.js
import { defineConfig } from "vite";
export default defineConfig({ worker: { format: "es" } });
```


The host owns persistence; `save()` returns bytes and `onChange` reports saves.
See the [0.2.0 changes and known limits](https://github.com/jgridifier/nitro-xml/blob/main/CHANGELOG.md#020--2026-11-20) in the CHANGELOG, and the [0.1.0 release notes](https://github.com/jgridifier/nitro-xml/blob/main/docs/releases/0.1.0.md) for earlier limits and known losses.

## Styling

`<nitro-docx>`, `<nitro-xlsx>` and `<nitro-pptx>` style themselves inside an
open shadow root. There is no stylesheet the eager loader imports, and you do
not need one for the element chrome to appear:

- Base CSS is inlined from `_css()` in `loader/element.js` (shell elements use
  `:host { display: flex; flex-direction: column; min-height: 12rem; }`).
- The Rust/WASM renderer emits grid classes (`xg-*`) and inline styles.
- The ribbon skin is compiled into the WASM chrome from
  `rs/crates/nitro-xml-wasm/src/chrome/ribbon_skin.css`.

Give the element an explicit height (or a flex parent). The `:host` rule only
sets a `min-height` of `12rem`.

### Excel grid stylesheet (optional export)

The full `.xg-*` grid stylesheet is **not** in the eager loader (the 17 408 B
brotli budget must stay). It is shipped as a separate package export, matching
the visual-gate / demo `GRID_CSS` string:

```js
import { GRID_CSS } from "nitro-xml/xlsx-grid-css.js";

const el = document.querySelector("nitro-xlsx");
if (el?.shadowRoot && !el.shadowRoot.querySelector("style[data-nitro-grid]")) {
  const style = document.createElement("style");
  style.dataset.nitroGrid = "";
  style.textContent = GRID_CSS;
  el.shadowRoot.appendChild(style);
}
```

```js
import "nitro-xml/xlsx-grid.css"; // bundler / document stylesheet
```

A document-level import does **not** pierce the shadow root. For `<nitro-xlsx>`,
inject `GRID_CSS` into `el.shadowRoot` as in the first example (the demo does
the same). `demo/xlsx-grid-css.js` and `parity/visual/xlsx-grid-css.js` are
demo and test copies of the same stylesheet; they are not the package path.

### Theming

Set the `theme` attribute or property to `"office"`, `"quiet"`, `"dark"`, or a
token object. That sets `data-theme` and the `--nx-*` variables on the element
(lazy-loaded; see [`docs/theme.md`](../../docs/theme.md)). Themes restyle
**chrome only** — the painted document stays document-faithful.

```html
<style>
  nitro-xlsx {
    height: 600px;
    --nx-accent: #0b6e4f;
    --xve-sheet-bg: #fafafa;
  }
  nitro-xlsx::part(formula-bar) { font-size: 13px; }
</style>
<nitro-xlsx src="model.xlsx" theme="quiet"></nitro-xlsx>
```

You can also set tokens in CSS on the element or an ancestor, or assign
`el.theme = { "--nx-accent": "#0b6e4f", fontSans: "Georgia, serif" }`.

Chrome tokens (`--nx-*`): `--nx-ink`, `--nx-ink-muted`, `--nx-bg`,
`--nx-bg-subtle`, `--nx-border`, `--nx-accent`, `--nx-accent-contrast`,
`--nx-sel`, `--nx-font-sans`, `--nx-radius`, `--nx-space`, `--nx-titlebar`,
`--nx-titlebar-ink`, `--nx-canvas`, `--nx-shadow`. `--nx-fg` still works as a
fallback for `--nx-ink`.

Excel grid tokens (`--xve-*`, used by the shipped grid stylesheet and a cell
paint fallback; they inherit into the shadow tree): `--xve-sheet-bg`,
`--xve-grid-line`, `--xve-selection`, `--xve-selection-border`, `--xve-font`,
`--xve-header-bg`, `--xve-header-fg`, `--xve-header-line`,
`--xve-header-active-bg`, `--xve-header-full-bg`, `--xve-freeze-line`,
`--xve-popover`, `--xve-popover-foreground`, `--xve-border`, `--xve-accent`,
`--xve-secondary`.

`::part()` names that exist in the shipped tree:

- Always: `canvas`, `dom`, `message`
- Embedded mode: `embedded-toolbar`, `embedded-group`, `embedded-button`
- Shell chrome (when it is rendered): `toolbar`, `zoom`, `find`, `find-status`,
  `tabs`, `diagnostics`
- Excel formula bar: `formula-bar`, `name-box`, `formula-input`

## Host API

For hosts embedding Nitro inside a chat UI or agent harness — full docs in
[`loader/README.md#host-api`](loader/README.md#host-api).

### `getSelection(): Promise<NitroSelection | null>`

Returns a snapshot of the current selection (#561). Call after loading
completes and after the user's selection gesture. No `nitro-selectionchange`
event is emitted, and reading the selection never marks the document dirty.

- **DOCX**: the caret only. Returns `start`/`end` `{paragraph, run, offset}` at
  the caret (`offset` counts code points from the paragraph start, `run` is `0`),
  `text: ""` and `collapsed: true`. With no caret, `start`/`end` are `null`.
- **XLSX**: `{format, sheet, sheetIndex, range, anchor, values, truncated}`.
  `values` holds the stored cell values row by row: numbers, strings, booleans,
  error text such as `"#DIV/0!"`, or `null` for an empty cell. At most 10,000
  cells are returned; past that, whole rows are dropped and `truncated` is `true`.
- **PPTX**: the clicked shape only. Returns its `shapeId`, `shapeName` and whole
  `text`, with `textRange: null` and `collapsed: true`. With nothing selected,
  the shape fields are `null`.

**v0.2.0 limit:** in Word and PowerPoint, `getSelection()` returns the caret or the
clicked shape only. It never reports a selected text range. Text-range selection
is tracked in #561.

```js
const selection = await viewer.getSelection();
console.log(selection); // e.g. { format: "xlsx", range: "B2:C3", values: [[4, 6], [6, 9]], ... }
```

### `dirty: boolean` getter + `nitro-change` event

For `readonly="false"`, real typing (and committed XLSX cell edits) sets the
read-only getter to `true`. The bubbling, composed event has
`detail: { dirty: boolean }`. `save(): Promise<Uint8Array>` returns the package
bytes and resets dirty after success.

```js
el.addEventListener("nitro-change", (e) => {
  console.log("dirty:", e.detail.dirty, el.dirty); // true on first edit, false after save()
});
await el.save(); // resets dirty → false, fires nitro-change {dirty:false}
```

Fires only when `dirty` flips. Never fires during load, scroll, zoom, navigation,
or `save()` on an unedited document.

### Position, reload, and live readonly

The following APIs apply to `<nitro-docx>`, `<nitro-xlsx>` and `<nitro-pptx>`.
Indices are zero-based, zoom is a percentage, and scroll offsets use CSS pixels.

- `getPosition(): NitroPosition` synchronously returns the last painted
  `{page?, sheet?, sheetName?, slide?, scrollTop, scrollLeft, zoom?}`. Only the
  current format's index is present. Before open it returns zero scroll offsets.
- `setPosition(pos: NitroPositionInput): Promise<void>`
  applies supplied fields, clamps numeric indices/scroll/zoom, and leaves omitted
  fields unchanged. `sheet` accepts an index or exact sheet name; an unknown name
  or non-finite number rejects. It neither focuses the editor nor emits `nitro-change`.
- `loadBuffer(bytes: ArrayBuffer | Uint8Array, name?: string, opts?: {position?: NitroPositionInput}): Promise<void>`,
  `loadFile(file: Blob, name?: string, opts?: {position?: NitroPositionInput}): Promise<void>`,
  and `loadUrl(url: string, opts?: {position?: NitroPositionInput}): Promise<void>`
  apply the optional position at the opening frame. Omit `position` for the file's
  default location (xlsx `topLeftCell`, saved scroll/zoom). The viewport is stabilised
  before the saved position is applied, so chrome layout during open does not shift it.
  These calls do not emit `nitro-change` or request focus.
- `reload(bytes: ArrayBuffer | Uint8Array, opts?: {keepPosition?: boolean, name?: string}): Promise<void>`
  replaces the file in the existing worker and retains the element and shadowRoot.
  `keepPosition` defaults to `true`; restored indices and scroll clamp to the new
  file. `false` opens at the default location. `name` defaults to the previous name.
  Mode stays unchanged; dirty resets silently to false and the edit journal starts clean. No `nitro-change` or focus change.
- `readonly: boolean` is writable. `el.readonly = false` enables editing in place;
  `true` disables editing and makes `save()` reject. Attribute changes work too:
  `readonly="false"` enables editing; absent, empty and other values mean readonly
  (the existing default). Changes preserve the document, position, and host focus.
- `nitro-mode: CustomEvent<{readonly: boolean}>` bubbles and is composed. It fires
  after a live mode transition finishes, not on initial open or a redundant assignment.
  Mode switches do not emit `nitro-change`.

```js
const editor = document.querySelector('nitro-docx');
await editor.loadUrl('/report.docx', { position: { page: 1, zoom: 100 } });
const position = editor.getPosition();
await editor.setPosition({ page: 2, scrollTop: 40 });
await editor.loadFile(fileInput.files[0], 'report.docx', { position });
await editor.loadBuffer(bytes, 'report.docx', { position });
await editor.reload(updatedBytes); // keep position and mode
await editor.reload(updatedBytes, { keepPosition: false, name: 'updated.docx' });
editor.addEventListener('nitro-mode', ({ detail }) => console.log(detail.readonly));
editor.readonly = false; // async transition; nitro-mode signals completion
editor.setAttribute('readonly', 'true');
// Spreadsheet example: await sheetEditor.setPosition({ sheet: 'Budget' });
```

### Font supply without Google Fonts (`fonts`, `fontSource`) — #566

By default Nitro may request webfont stylesheets from `fonts.googleapis.com`.
To avoid that, supply fonts explicitly:

```js
// Global default — applies to all future WorkerDocument / element loads.
await initCore({
  fontSource: "off",          // stop any Google Fonts fallback
  fonts: [
    // Load from an explicit URL (served from your CDN / bundle):
    { family: "Inter", url: "/fonts/Inter.woff2", weight: "400", style: "normal" },
    // Or reuse a system font under a custom family name:
    { family: "NitroMono", url: "local(monospace)" },
    // Stylesheet URL — all @font-face rules inside are injected:
    "/fonts/custom.css",
  ],
});
```

**Per-element override** (set before `loadBuffer`/`loadFile`/`loadUrl`):

```js
el.fonts = [{ family: "Inter", url: "/fonts/Inter.woff2" }];
await el.loadBuffer(buf, "doc.docx");
```

**Reuse fonts the host page already loaded** (`fontSource: "page"`):

```js
await initCore({ fontSource: "page" });
// Nitro forwards families from document.fonts to the WASM worker — no Google request.
```

**`fontSource` values:**

| Value | Behaviour |
|---|---|
| omitted | CSS2 base URL (`https://fonts.googleapis.com/css2`) |
| `"off"` / `false` | No remote fetch; unknown families get substitution diagnostics |
| `"page"` | Reuse families already in `document.fonts`; forward to worker |
| `string` (URL) | CSS2 mirror base URL (alternative CDN) |

The `fonts` array is loaded via `FontFace` on the main thread **and** forwarded to
the WASM worker (where text is measured). No request to `fonts.googleapis.com` or
`fonts.gstatic.com` is made when `fonts` is set or `fontSource` is `"off"` / `"page"`.

### Ribbon option, `execCommand`, and `nitro-command` event — #590

#### `el.ribbon = { hide?, add? }`

Control which commands appear in the ribbon and inject custom host buttons.

```js
el.ribbon = {
  // Hide built-in commands by id (removes button from ribbon, shortcuts still work
  // unless the host also cancels the event).
  hide: ["file.open", "file.new"],

  // Inject host-defined buttons. These fire `nitro-command` only; Rust has no
  // handler for them. SVG icon is a path `d` string (strict-CSP safe, no innerHTML).
  add: [
    {
      id:      "host.review",        // must be unique; returned in el.commands
      label:   "Send for review",    // visible label
      icon:    "M12 2L2 7l10 5…",    // optional SVG path data
      tab:     "Home",               // optional (reserved for future full-mode tab routing)
      group:   "Actions",            // optional
      editing: false,                // true = disabled when el.readonly; default true
    },
  ],
};

// Clear all ribbon overrides:
el.ribbon = null;
```

The React `NitroViewer` component accepts a matching `ribbon` prop and a `onCommand`
handler; both are synchronised without remounting:

```tsx
<NitroViewer
  file={buf} fileName="report.xlsx" editable
  ribbon={{ hide: ["file.open"], add: [{ id: "host.review", label: "Review" }] }}
  onCommand={e => { if (e.detail.id === "host.review") openReviewPanel(); }}
/>
```

#### `el.commands → NitroCommandEntry[]`

Returns a snapshot of all commands (built-in + host-added) with their current state:

```ts
interface NitroCommandEntry {
  id:        string;         // e.g. "home.bold"
  label:     string;         // e.g. "Bold"
  shortcut:  string | null;  // e.g. "Ctrl+B"
  enabled:   boolean;        // false when el.readonly and editing:true
}
```

#### `el.execCommand(id: NitroCommandId | string): boolean`

Programmatically dispatch any registered command. Returns `false` for unknown ids
and logs the full list to the console.

```js
el.execCommand("home.bold");        // true — dispatches the command
el.execCommand("host.review");      // true — fires nitro-command only (host-added)
el.execCommand("bad.id");           // false — console.error with valid ids listed
```

#### `nitro-command` event (cancelable)

Fires **before** every command dispatch: ribbon clicks, keyboard shortcuts,
`execCommand`, and host-added button clicks.

```js
el.addEventListener("nitro-command", (e) => {
  console.log(e.detail.id, e.detail.format); // e.g. "home.bold", "xlsx"

  // Prevent the built-in handler (e.g. replace file.save with your own):
  if (e.detail.id === "file.save") {
    e.preventDefault();
    myCustomSave(el);
  }
});
```

`e.detail.format` is `"xlsx"`, `"docx"`, or `"pptx"`.

#### Built-in command ids (`NitroCommandId`)

A representative sample — the full union is in `loader/index.d.ts`:

| id | Label | Shortcut | editing |
|----|-------|----------|---------|
| `home.bold` | Bold | Ctrl+B | ✓ |
| `home.italic` | Italic | Ctrl+I | ✓ |
| `home.copy` | Copy | Ctrl+C | — |
| `home.find` | Find… | Ctrl+F | — |
| `edit.undo` | Undo | Ctrl+Z | ✓ |
| `edit.redo` | Redo | Ctrl+Y | ✓ |
| `file.save` | Save | Ctrl+S | ✓ |
| `insert.link` | Insert link | Ctrl+K | ✓ |

Editing commands (`editing: true`) are **disabled** (`enabled: false` in `el.commands`)
while `el.readonly` is `true`. Host-added buttons with `editing: true` follow the same
rule and are rendered as `disabled` in the toolbar.

### Focus — load never steals focus (`autofocus`, `{focus:true}`) — #567

By default, `loadBuffer` / `loadFile` / `loadUrl` **never move focus** away from
whatever the host page has focused (e.g. a chat textarea). The editor only receives
focus on explicit user interaction (pointer click on the document area).

**Opt-in focus after load:**

```js
// Option A: attribute (declarative)
<nitro-docx autofocus></nitro-docx>

// Option B: loadBuffer option
await el.loadBuffer(buf, "doc.docx", { focus: true });
```

User clicks on the document always focus the editor regardless of these options.

## Loader size gate

```bash
bash scripts/ci/bundle_budget.sh   # must stay ≤ 17408 B brotli (eager path)
```

`loader/lazy/**` is excluded from the eager measure (dynamic `import()` only).
