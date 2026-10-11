// SPDX-License-Identifier: MIT
/** Bundle entry: upstream Print Layout stage for visual gate (#23). */

import { loadDocx } from '../../../upstream/src/core/docx/parse.ts';
import { layoutDocumentModel } from '../../../upstream/src/core/docx/layout/layout.ts';
import { resolveParagraphFormatting } from '../../../upstream/src/core/docx/paragraph-styles.ts';
import { createCanvasMeasurer } from '../../../upstream/src/ui/src/docx/canvas-measurer.ts';
import { renderPrintLayout } from '../../../upstream/src/ui/src/docx/print-layout.ts';
import { decoratePages } from '../../../upstream/src/ui/src/docx/print-header-footer.ts';
import { printCss } from '../generated/docx-print-css.mjs';

export { printCss };

/**
 * @param {HTMLElement} root
 * @param {Uint8Array} bytes
 * @param {{ injectCss?: boolean }} [opts]
 */
export async function mountDocxPrintLayout(root, bytes, opts = {}) {
	if (opts.injectCss !== false) {
		let style = root.querySelector('style[data-visual="docx-print-css"]');
		if (!style) {
			style = document.createElement('style');
			style.dataset.visual = 'docx-print-css';
			root.appendChild(style);
		}
		style.textContent = printCss;
	}

	if (document.fonts?.ready) await document.fonts.ready;

	const loaded = await loadDocx(bytes);
	const model = loaded.model;
	const measurer = createCanvasMeasurer();
	const result = layoutDocumentModel(model, measurer, { reviewDisplayMode: 'all' });

	const blobUrls = new Map();
	/** @type {import('../../../upstream/src/ui/src/docx/print-layout.ts').PictureUrl} */
	const pictureUrl = (partName, contentType) => {
		if (!partName) return undefined;
		const hit = loaded.media.get(partName);
		if (!hit) return undefined;
		let url = blobUrls.get(partName);
		if (!url) {
			const blob = new Blob([hit], { type: contentType || 'application/octet-stream' });
			url = URL.createObjectURL(blob);
			blobUrls.set(partName, url);
		}
		return url;
	};

	const handle = renderPrintLayout(result, pictureUrl, {
		lineNumbers: (model.sections ?? []).map((section) => section.lineNumberSettings),
		pageBorders: (model.sections ?? []).map((section) => section.pageBorders),
		suppressedLineNumberParagraphs: new Set(
			model.blocks
				.filter(
					(block) =>
						block.type === 'paragraph' &&
						(model.paragraphStyles
							? resolveParagraphFormatting(block, model.paragraphStyles).suppressLineNumbers
							: block.suppressLineNumbers),
				)
				.map((block) => block.id),
		),
	});
	if (model.pageColor) {
		handle.element.style.setProperty('--dve-page-color', `#${model.pageColor}`);
	}
	decoratePages(
		model,
		result.pages,
		[...handle.element.querySelectorAll('.dve-print-page')],
		new Date(),
		pictureUrl,
		'all',
	);

	root.replaceChildren(handle.element);
	return { element: handle.element, pageCount: handle.pageCount };
}
