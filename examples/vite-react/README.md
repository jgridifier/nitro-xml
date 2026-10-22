<!-- SPDX-License-Identifier: MIT -->
# Vite + React example (`nitro-xml` / `@nitro-xml/react`)

Minimal host Jared can copy when embedding Nitro XML for real-time viewing and
editing. Startup uses the same lazy WASM path as production (`initCore` inside
the viewer effect); the eager loader brotli budget is unchanged.

## Setup

```bash
# from repo root
bash scripts/build-wasm.sh
cd packages/nitro-xml-react && npm install && npm run build
cd ../../examples/vite-react
mkdir -p public/fixtures
cp ../../fixtures/generated/xlsx/cells_1k.xlsx public/fixtures/
cp ../../fixtures/generated/docx/pages_10.docx public/fixtures/
cp ../../fixtures/generated/pptx/slides_10.pptx public/fixtures/
npm install
npm run test:e2e:install   # once
npm run dev                # http://127.0.0.1:5180
```

## Playwright (open → edit → save per format)

```bash
npm run test:e2e
```

Opens xlsx, docx and pptx, applies one edit, asserts `save()` returns a ZIP
(OOXML) payload.
