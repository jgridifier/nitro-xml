// SPDX-License-Identifier: MIT
/**
 * Bundle entry for the upstream `<docx-editor>` visual host (#150 / #83).
 * Prepared by prepare.mjs into out/bundles/docx-editor.js (test-only).
 */
import { registerDocxEditor } from '../../../upstream/src/ui/dist/docx.js';

export function defineDocxEditor() {
	registerDocxEditor();
}
