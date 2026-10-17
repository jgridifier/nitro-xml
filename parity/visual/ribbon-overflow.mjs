// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/ribbon/overflow.ts; changes in PROVENANCE.md.
/** Collapse `.ribbon-group` from the right when a panel is too narrow (#150). */

/** Upstream GROUP_ICONS → ICON_PATHS (24×24, stroke 1.7). */
const GROUP_ICON = {
	Number: 'M4 5h16v14H4z M8 9v6 M7 10l1-1 M12 9h3l-3 6h3 M17 9h2',
	Styles: 'M4 6h7v5H4z M13 6h7v5h-7z M4 13h7v5H4z M13 13h7v5h-7z',
	Cells: 'M4 4h10v10H4z M4 9h10 M9 4v10 M18 13v8 M14 17h8',
	Editing: 'M10 4a6 6 0 1 0 0 12 6 6 0 0 0 0-12z M14.5 14.5 20 20',
};

function pathSvg(doc, d, size, sw = '1.7') {
	const s = doc.createElementNS('http://www.w3.org/2000/svg', 'svg');
	s.setAttribute('width', String(size));
	s.setAttribute('height', String(size));
	s.setAttribute('viewBox', '0 0 24 24');
	s.setAttribute('aria-hidden', 'true');
	s.classList.add('ribbon-icon');
	const p = doc.createElementNS('http://www.w3.org/2000/svg', 'path');
	p.setAttribute('d', d);
	p.setAttribute('fill', 'none');
	p.setAttribute('stroke', 'currentColor');
	p.setAttribute('stroke-width', sw);
	p.setAttribute('stroke-linecap', 'round');
	p.setAttribute('stroke-linejoin', 'round');
	s.appendChild(p);
	return s;
}

function caretSvg(doc) {
	const s = pathSvg(doc, 'M7 10l5 5 5-5', 12);
	s.dataset.icon = 'caret';
	return s;
}

/** Upstream setLargeCaption: prefer a two-line break near the middle. */
function setLargeCaption(span, label) {
	span.classList.add('ribbon-caption');
	const words = label.trim().split(/\s+/);
	if (words.length < 2) {
		span.textContent = label;
		return;
	}
	let best = 1;
	let bestScore = Infinity;
	for (let i = 1; i < words.length; i++) {
		const a = words.slice(0, i).join(' ');
		const b = words.slice(i).join(' ');
		const score = Math.abs(a.length - b.length);
		if (score < bestScore) {
			bestScore = score;
			best = i;
		}
	}
	span.textContent = `${words.slice(0, best).join(' ')}\n${words.slice(best).join(' ')}`;
}

function overflowButton(group) {
	const d = group.ownerDocument;
	const b = d.createElement('button');
	b.type = 'button';
	b.className = 'ribbon-overflow-button ribbon-large';
	b.setAttribute('aria-haspopup', 'true');
	const dPath = GROUP_ICON[group.dataset.label];
	if (dPath) b.appendChild(pathSvg(d, dPath, 28));
	else {
		const ic = group.querySelector('.ribbon-icon');
		if (ic) {
			const c = ic.cloneNode(true);
			c.setAttribute('width', '28');
			c.setAttribute('height', '28');
			b.appendChild(c);
		}
	}
	const span = d.createElement('span');
	setLargeCaption(span, group.dataset.caption || group.dataset.label || '');
	b.appendChild(span);
	b.appendChild(caretSvg(d));
	const label = group.dataset.caption || group.dataset.label || '';
	b.setAttribute('aria-label', label);
	b.title = label;
	return b;
}

export function fitPanel(p) {
	if (!p || p.hidden || !p.clientWidth) return;
	const d = p.ownerDocument;
	const z = d.createElement('span');
	z.className = 'ribbon-caption-sizer';
	p.appendChild(z);
	for (const g of p.querySelectorAll(':scope > .ribbon-group')) {
		z.textContent = g.dataset.caption || '';
		g.style.minWidth =
			Math.ceil(z.offsetWidth) + (g.querySelector(':scope > .ribbon-launcher') ? 32 : 12) + 'px';
	}
	z.remove();
	for (const g of p.querySelectorAll('.ribbon-group[data-collapsed]')) {
		g.removeAttribute('data-collapsed');
	}
	const gs = [...p.querySelectorAll(':scope > .ribbon-group')].filter((g) => g.offsetWidth > 0);
	for (let i = gs.length - 1; i >= 0 && p.scrollWidth > p.clientWidth + 1; i--) {
		const g = gs[i];
		let b = g.querySelector(':scope > .ribbon-overflow-button');
		if (!b) {
			b = overflowButton(g);
			g.appendChild(b);
		}
		const L = g.dataset.caption || g.dataset.label || '';
		const s = b.querySelector('span');
		if (s) setLargeCaption(s, L);
		b.setAttribute('aria-label', L);
		b.title = L;
		g.setAttribute('data-collapsed', '');
	}
}

export function attachRibbonOverflow(root) {
	if (!root || typeof ResizeObserver == 'undefined') return () => {};
	const ro = new ResizeObserver((es) => {
		for (const e of es) fitPanel(e.target);
	});
	for (const p of root.querySelectorAll('.ribbon-panel')) {
		fitPanel(p);
		ro.observe(p);
	}
	return () => ro.disconnect();
}

export function refitRibbon(root) {
	if (root) for (const p of root.querySelectorAll('.ribbon-panel')) fitPanel(p);
}
