# Changelog

All notable changes to `nitro-xml` are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **Excel grid stylesheet export** (`nitro-xml/xlsx-grid.css` and
  `import { GRID_CSS } from "nitro-xml/xlsx-grid-css.js"`). Same rules as the
  visual-gate `GRID_CSS` string. Not imported by the eager loader, so the
  17 408 B brotli budget is unchanged. Document-level CSS does not pierce the
  shadow root; inject `GRID_CSS` into `el.shadowRoot` for `<nitro-xlsx>`.

- **Font supply without Google Fonts** (`fonts`, `fontSource: "page"`) — #566
  - `initCore({ fonts: [...], fontSource: "off" })` accepts an explicit font list
    (`{ family, url, weight?, style? }` entries or stylesheet URLs). Fonts are
    loaded via `FontFace` on the main thread and forwarded to the WASM worker.
  - `el.fonts = [...]` per-element override (set before `loadBuffer`/`loadFile`/`loadUrl`).
  - `fontSource: "page"` reuses font families already present in `document.fonts`
    (no Google Fonts request).
  - No request to `fonts.googleapis.com` or `fonts.gstatic.com` is made when
    `fonts` is supplied or `fontSource` is `"off"` / `"page"`.

- **No focus steal on open** (`autofocus`, `{focus:true}`) — #567
  - `loadBuffer` / `loadFile` / `loadUrl` never move focus away from whatever
    the host page had focused (e.g. a chat textarea).
  - Opt-in: `<nitro-docx autofocus>` attribute or `loadBuffer(buf, name, { focus: true })`.
  - User pointer clicks on the document area always focus the editor.

### Docs

- README **Styling** section: elements style themselves in a shadow root;
  theming via the `theme` attribute/property, `--nx-*` / `--xve-*` variables
  and `::part()`; how to import the optional grid stylesheet.
