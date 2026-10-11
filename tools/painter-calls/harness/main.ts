// SPDX-License-Identifier: MIT
// Mounts upstream's own painters from their TypeScript sources (`@up/` is the scratch upstream
// checkout), so call stacks carry the upstream file names that rows.json maps to painter rows.
// xlsx and docx use the product elements; pptx calls the slide-stage painter exactly as upstream's
// vanilla viewer does on open and on every slide change (`renderSlideStage` rebuilds the stage).
import { defineXlsxEditor } from '@up/src/ui/src/xlsx/index';
import '@up/src/ui/src/docx/index';
import { loadPresentation } from '@up/src/ui/src/pptx/dom/load-presentation';
import { renderSlideStage } from '@up/src/ui/src/pptx/dom/slide-stage';
import { createDefaultRegistry } from '@up/src/ui/src/pptx/dom/elements/index';
import { createTranslator } from '@up/src/ui/src/pptx/i18n/translator';

const host = document.getElementById('host') as HTMLElement;
type Loaded = Awaited<ReturnType<typeof loadPresentation>>;
let deck: Loaded | undefined;
const registry = createDefaultRegistry();
const t = createTranslator('en');

function stage(index: number): HTMLElement {
	const loaded = deck!;
	return renderSlideStage({
		document,
		slide: loaded.slides[index]!,
		canvasSize: loaded.canvasSize,
		mediaDataUrls: loaded.mediaDataUrls,
		colorScheme: loaded.colorScheme,
		fontScheme: loaded.fontScheme,
		tableStyleMap: loaded.tableStyleMap,
		registry,
		t,
		scale: 1,
		slides: loaded.slides,
		currentSlideIndex: index,
	});
}

const harness = {
	async openXlsx(bytes: Uint8Array, name: string) {
		defineXlsxEditor();
		const el = document.createElement('xlsx-editor') as HTMLElement & {
			load(b: Uint8Array, n?: string): Promise<void>;
		};
		host.append(el);
		await el.load(bytes, name);
	},
	async openDocx(bytes: Uint8Array) {
		const el = document.createElement('docx-editor') as HTMLElement & {
			load(b: Uint8Array): Promise<void>;
		};
		host.append(el);
		await el.load(bytes);
	},
	async loadPptx(bytes: Uint8Array) {
		deck = await loadPresentation(bytes.buffer.slice(0) as ArrayBuffer);
		return deck.slides.length;
	},
	showSlide(index: number) {
		host.replaceChildren(stage(index));
	},
};
(window as unknown as { harness: typeof harness }).harness = harness;
document.documentElement.dataset.ready = '1';
