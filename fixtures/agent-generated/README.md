<!-- SPDX-License-Identifier: MIT -->
# Agent-generated Office corpus

All content and the PNG are original, self-generated MIT fixtures. Office files
are registered with source, generator, hash and licence in `../manifest.json`.

```sh
python3 -m venv /workspace/host-api-fixtures-venv
/workspace/host-api-fixtures-venv/bin/pip install -r fixtures/agent-generated/requirements.txt
/workspace/host-api-fixtures-venv/bin/python fixtures/agent-generated/generate.py
npm install --prefix /workspace/host-api-fixtures-npm docx@9.5.1 exceljs@4.4.0 pptxgenjs@4.0.1
NODE_PATH=/workspace/host-api-fixtures-npm/node_modules node fixtures/agent-generated/generate.cjs
/workspace/pw-venv/bin/python scripts/e2e/host-api/agent_fixtures.py
```

Regeneration may change ZIP timestamps; refresh manifest hashes/byte counts when
committing regenerated outputs. Dependencies live outside the repository.
Python DOCX/PPTX and npm DOCX/PPTX include text, headings/titles, lists/bullets,
tables and an image. Both spreadsheets include formulas, styling and two sheets.
Python-generated files also contain a real external link for the CSP click guard.

The browser check requires real focus and typing, a dirty transition, save,
source-library reopen for Python files, then browser reopen with the edit present.
The npm `docx` and `pptxgenjs` libraries are writers with no package-reading API;
their outputs are re-read with python-docx/python-pptx. ExcelJS output is also
re-read with ExcelJS. There are no skipped/demoted files.
