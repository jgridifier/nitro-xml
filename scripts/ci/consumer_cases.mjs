// SPDX-License-Identifier: MIT
import { mkdirSync, readFileSync, writeFileSync, copyFileSync } from 'node:fs';
import { resolve, join, basename } from 'node:path';
import { loadEditScripts, resolveFixture } from '../../parity/edit/scripts.mjs';
const [app, out] = process.argv.slice(2).map(p => resolve(p));
const ids = process.env.NITRO_HANDCHECK === '1' ? [
  'docx-text-and-format', 'docx-pages10-multi', 'docx-pages10-insert-row',
  'xlsx-set-cell-text', 'xlsx-insert-row-formula', 'xlsx-value-and-style',
  'pptx-text-and-style', 'pptx-move-resize-s0', 'pptx-reorder-slide',
] : ['docx-pages10-para-text', 'xlsx-set-cell-text', 'pptx-set-text-s0'];
const scripts = loadEditScripts();
const cases = ids.map(id => {
  const script = scripts.find(s => s.id === id);
  const fixture = resolveFixture(script);
  const file = basename(fixture.path);
  copyFileSync(fixture.path, join(app, 'public/fixtures', file));
  // The UI routes slide reorder through this editor command, not the TS oracle op.
  const ops = script.ops.map(op => op.op === 'reorderSlide' ? { ...op, op: 'insert.move-slide-to' } : op);
  return { ...script, ops, path: undefined, file, source: fixture.path, licence: fixture.licence };
});
if (process.env.NITRO_HANDCHECK === '1') {
  const base = cases.find(c => c.id === 'xlsx-set-cell-text');
  cases.push({ ...base, id: 'xlsx-undo-redo', description: 'Set B1 text; undo; redo. B1 must contain the replacement after reopen.',
    ops: [...base.ops, { op: 'edit.undo' }, { op: 'edit.redo' }] });
}
mkdirSync(join(out, 'originals'), { recursive: true });
for (const entry of cases) copyFileSync(entry.source, join(out, 'originals', `${entry.id}.${entry.format}`));
writeFileSync(join(app, 'cases.json'), JSON.stringify(cases, null, 2));
writeFileSync(join(out, 'cases.json'), JSON.stringify(cases, null, 2));
writeFileSync(join(out, 'README.md'), `# Nitro XML 0.1.0 Office hand-check\n\nGenerated through packed @nitro-xml/react applyEdit() and save() in production Vite/Chromium.\nOriginals are in originals/ with matching names. cases.json records the exact commands and fixture sources (see repository fixtures/manifest.json for licences).\n\nOpen each original and saved file in Microsoft Office. Confirm no repair dialog, the edit below, and unchanged surrounding text, formatting, images, tables, sheet/slide order except where intentionally edited. Save and reopen once more. Validators are recorded in validation.json; they do not replace human Office review.\n\n| Saved file | Edit / what to inspect |\n|---|---|\n` + cases.map(c => `| ${c.id}.${c.format} | ${c.description} |`).join('\n') + '\n\nFor DOCX check paragraph/run formatting and table geometry; XLSX check formula references after row insertion, value/style and redo persistence; PPTX check text, shape position/size and the first slide moved to the end. These cover risky save operations from docs/ux-interaction-audit-2026-11-10.md, not a re-run of pointer/IME/selection UX.\n');
