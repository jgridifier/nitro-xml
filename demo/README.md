<!-- SPDX-License-Identifier: MIT -->
# Nitro XML live demos

Static GitHub Pages demos for `<nitro-xlsx>`, `<nitro-docx>` and `<nitro-pptx>` in
**full** (normal) and **embedded** modes (`docs/embedded-mode.md`).

## Local preview

```bash
bash scripts/ci/pages.sh
python3 -m http.server -d _site 4173
# open http://127.0.0.1:4173/demo/
```

The assembled site copies this folder to `_site/demo/` and vendors the WASM package
plus loader under `_site/demo/nitro-xml/`. Demo HTML/JS is **not** part of the
loader brotli budget (`scripts/ci/bundle_budget.sh` only walks `packages/nitro-xml/loader`).

## Deploy

`.github/workflows/pages.yml` already builds with `scripts/ci/pages.sh` and deploys
via `actions/deploy-pages` when the repo variable `PAGES_DEPLOY=actions` is set.
Until then Pages still serves the legacy **/docs** branch path and will not pick up
this demo (or the WASM payload).

## Theme

Demos default to the `quiet` theme; append `?theme=office` (or `dark`) to any demo URL, or use the *Office look* link in the title bar. Hosts pass a custom theme as `theme: { accent: "#0b6e4f" }`; see `docs/theme.md`.

## Editing

The **xlsx** demos open writable (`readonly="false"`): ribbon commands, typing, Save (downloads the edited
file) and an unsaved-changes indicator. pptx and docx stay read-only because their editors are not wired to the
WASM handle yet; see `docs/ux-revision.md` (Editing pass, and what was skipped).

## Samples

`samples/sample.{xlsx,docx,pptx}` are MIT copies of generated corpus fixtures
(see `fixtures/manifest.json` ids `gen-xlsx-cells-1k`, `gen-docx-pages-10`,
`gen-pptx-painter-placeholder`). Nothing is uploaded when you use the file picker;
files stay in the browser.
