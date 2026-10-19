<!-- SPDX-License-Identifier: MIT -->
# Embedded mode (design note)

Status: proposal (Stream F). Slide show stays deferred (#122); nothing here builds it.

## Goal
Host apps that embed `<nitro-xlsx>`, `<nitro-docx>` or `<nitro-pptx>` want one small, familiar toolbar, not the full viewer chrome. Embedded mode replaces the chrome with a single compact bar that looks and behaves like the PowerPoint Home ribbon (icon above label, grouped, group names underneath at wide widths). **Default mode does not change**: same DOM, same pixels, same events. Embedded is opt-in.

## API
| Surface | Form |
| --- | --- |
| Attribute | `<nitro-pptx mode="embedded" src="deck.pptx">` (`mode="full"` or absent = today's UI) |
| JS option | `el.mode = "embedded"` (property reflects the attribute; can be set before or after connect, and toggled at runtime) |
| Events | unchanged (`nitro-save`, `nitro-copy`, `nitro-find`, `nitro-hyperlink`); the toolbar only calls the existing `shellAction(op, arg)` entry point |
| Parts | `part="embedded-toolbar"`, `part="embedded-group"`, `part="embedded-button"` so hosts can theme with `::part`; CSS custom properties `--nx-accent`, `--nx-bg`, `--nx-fg`, `--nx-border` |

`readonly` keeps its meaning. With `readonly` (the default) only view/clipboard-read groups appear; editing groups appear when `readonly="false"` **and** the format's editor is wired to the WASM handle.

## Architecture (keeps the eager loader flat)
- Eager code adds only: observe `mode`, and `import("./lazy/embedded/index.js")` when it is `embedded`. Everything else lives under `loader/lazy/` (excluded from the 17 KB brotli budget; ceiling is not raised).
- The lazy module builds the toolbar DOM from a declarative manifest (`groups -> buttons`, each button = icon + label + tooltip + a `shellAction` op or a ribbon `cmd` id). One shared component for all formats; each format supplies its manifest.
- **No new editing behaviour and no Rust changes.** The Rust chrome (`shell/chrome.rs`, xlsx ribbon, formula bar, dialogs) keeps rendering into `.chrome`; embedded mode hides the parts it replaces with CSS and *mirrors* `disabled` / `aria-pressed` from the matching hidden Rust control (found by `data-a`/`data-g`) through a `MutationObserver`. Rust stays the single source of truth for enabled/pressed state, so there is nothing to keep in sync by hand.
- A button whose command is not reachable on the current format/readonly state is **omitted**, not shown dead. The manifest lists only ids that map to a real action today.

## What stays, what goes
| Control | Embedded |
| --- | --- |
| Zoom -, %, + | stays (View group) |
| Find bar | stays; opened by the toolbar Find button, styled to match |
| Copy | stays (Clipboard group) |
| Sheet tabs (xlsx) | stays |
| Slide / page tab strip (pptx, docx) | **goes** (replaced by the Slides / Pages group: Previous, `n / N`, Next) |
| Slide / page prev-next | stays (pptx "Slides" group, docx "Pages" group) |
| Formula bar (xlsx, editable) | stays, it is core Excel UI |
| Dialogs / menus opened by commands | stay |
| Old text toolbar (`.b`) | **goes** (replaced) |
| Diagnostics toggle and panel | **goes** (host can still call `shellAction("diagnostics-toggle")`) |
| Xlsx Home ribbon (tab strip, ~40 controls, galleries) | **goes** (replaced by the compact bar) |

## Toolbar layout
Every bar is a `role="toolbar"` with a roving tabindex (Left/Right move, Home/End jump, one tab stop), `aria-label`, `aria-pressed` on toggles, native `disabled`, visible `:focus-visible` ring, and a tooltip (`title` plus the `aria-label` carries the shortcut where one exists).

Order follows PowerPoint Home: **Clipboard, Slides/Pages/Sheets, Font, Paragraph/Alignment, Insert/Cells, Drawing/Arrange, Editing, View**. Groups with no wired command for the format are dropped.

### pptx
Clipboard (Copy) · Slides (Previous, Next, `n / N`) · Editing (Find) · View (Zoom out, %, Zoom in).
Font, Paragraph, Insert, Arrange exist in `nitro-xml-pptx::editor` but are **not exposed by the WASM handle yet**; wiring them is a separate engine task. Once the handle exposes them they are a manifest addition (items bound to `cmd` ids show only while the Rust chrome exposes that control), with no toolbar change.

### docx
Clipboard (Copy) · Pages (Previous, Next) · Editing (Find) · View (zoom). Font/Paragraph/Styles/Insert follow the same rule as pptx.

### xlsx (editable)
Undo/Redo/Save · Font (Bold, Italic, Underline, Strikethrough, Grow, Shrink) · Alignment (Top, Middle, Bottom, Left, Center, Right, Wrap) · Number (Percent, Comma, Accounting, General) · Cells (Insert/Delete sheet; insert/delete rows and columns if reachable through `applyEdit`) · Editing (Clear contents, Find) · View. Read-only xlsx: Clipboard (Copy), Editing (Find), View.

## Narrow widths (container query on the host, not the viewport)
1. ≥ 720 px: icon + label, group captions.
2. 480-720 px: labels hidden (icon only, tooltips remain), captions hidden.
3. < 480 px: groups are moved, from the right, into a "More" overflow menu (`aria-haspopup`, `aria-expanded`, roving focus inside, Escape returns focus). Always visible: first group and View.

## Out of scope
Slide show (#122); new editing commands; Ribbon tabs; changes to the default UI, Rust chrome output, parsers, renderers, or crash recovery.

## Test and evidence plan
- Existing gates (a11y, selection/undo/edit parity, bundle-budget) must stay green; default mode is untouched.
- One non-obvious unit test: the toolbar mirrors `aria-pressed`/`disabled` from the Rust control after the chrome is re-rendered (innerHTML replacement drops observers on the old nodes), with a negative control.
- Playwright before/after screenshots per format, including a narrow shot, under `docs/embedded-mode/`.
