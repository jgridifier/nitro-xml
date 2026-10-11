// SPDX-License-Identifier: MIT
// PPTXjs v1.21.1 (meshesha/PPTXjs @ 1a9260b, jQuery plugin, not on npm). Its only public API is
// $(el).pptxToHtml({ pptxFileUrl }) which unzips, parses AND generates/inserts HTML in one
// synchronous pass once the bytes arrive (https://github.com/meshesha/PPTXjs#usage). There is no
// parse-only entry, so the timed span is: pptxToHtml() on an in-memory blob: URL until the plugin
// has inserted every slide. This is MORE work than a parse; it is flagged on the scoreboard.
// Options: mediaProcess:false (documented; skips audio/video extraction, no text effect).
// The vendor scripts (jQuery 1.11.3, JSZip 2, d3, nv.d3, pptxjs.js) are loaded as classic scripts.

export const OPTIONS = { mediaProcess: false, slideMode: false, keyBoardShortCut: false };

let blobUrl = null;
let host = null;

export function prepare(bytes) {
	if (blobUrl) URL.revokeObjectURL(blobUrl);
	blobUrl = URL.createObjectURL(new Blob([bytes]));
	if (host) host.remove();
	host = document.createElement('div');
	host.id = 'pptxjs-host';
	document.body.appendChild(host);
}

export function parse() {
	return new Promise((resolve, reject) => {
		const timer = setTimeout(() => reject(new Error('PPTXjs did not finish within 60 s')), 60000);
		const done = () => {
			const wrapper = host.querySelector('#all_slides_warpper');
			if (wrapper && wrapper.hasAttribute('style')) {
				observer.disconnect();
				clearTimeout(timer);
				resolve(host);
				return true;
			}
			return false;
		};
		const observer = new MutationObserver(done);
		observer.observe(host, { subtree: true, childList: true, attributes: true });
		const onError = (e) => {
			observer.disconnect();
			clearTimeout(timer);
			reject(e.error ?? new Error(e.message));
		};
		window.addEventListener('error', onError, { once: true });
		window.jQuery(host).pptxToHtml({ ...OPTIONS, pptxFileUrl: blobUrl });
	});
}

export function extract(root) {
	const slides = [];
	for (const slide of root.querySelectorAll('.slide')) {
		const paras = [];
		for (const p of slide.querySelectorAll('.slide-prgrph')) {
			// Runs are .text-block; a soft line break (<a:br/>) is an empty .line-break-br between them.
			let text = '';
			for (const el of p.querySelectorAll('.text-block, .line-break-br, br')) {
				if (el.tagName === 'BR' || el.classList.contains('line-break-br')) text += '\n';
				else if (!el.parentElement.closest('.text-block')) text += el.textContent;
			}
			paras.push(text);
		}
		slides.push(paras);
	}
	return { slides };
}

export function cleanup() {
	if (host) {
		host.remove();
		host = null;
	}
}
