// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/docx/styles/print.css; changes listed in PROVENANCE.md.

use super::dom::{Dom, StringIntern};

/// Print Layout stylesheet, injected once per frame (the host clears the DOM root first).
const PRINT_CSS: &str = r#"
.dve-print-pages {
	display: flex;
	flex-direction: column;
	align-items: center;
	gap: 24px;
	margin: 0 auto;
	zoom: var(--dve-zoom, 1);
}
.dve-print-pages[hidden] { display: none; }
.dve-print-page {
	position: relative;
	flex: none;
	background: var(--dve-page-color, #fff);
	color: #202124;
	box-shadow: 0 1px 3px #20212430, 0 4px 12px #20212420;
	overflow: hidden;
}
.dve-print-column { position: absolute; z-index: 1; }
.dve-print-float { z-index: 2; }
.dve-print-float-behind { z-index: 0; }
.dve-print-picture-missing {
	box-sizing: border-box;
	border: 1px dashed #8a8886;
	background: #f3f2f1;
}
.dve-print-chart { overflow: hidden; background: #fff; }
.dve-print-chart > svg { display: block; width: 100%; height: 100%; }
.dve-print-chart-frame {
	display: grid;
	place-items: center;
	color: #605e5c;
	font-size: 11px;
}
.dve-print-block { position: absolute; left: 0; width: 100%; }
.dve-print-line {
	position: absolute;
	left: 0;
	width: 100%;
	font: 11pt/1.2 Calibri, Arial, sans-serif;
	cursor: text;
}
.dve-print-line span { position: absolute; top: 0; }
.dve-print-row { position: absolute; left: 0; width: 100%; }
.dve-print-cell { position: absolute; top: 0; box-sizing: border-box; overflow: hidden; }
.dve-print-cell-content { position: absolute; bottom: 0; }
.dve-print-header, .dve-print-footer {
	position: absolute;
	color: var(--ink, #202124);
	font-size: 11pt;
	pointer-events: none;
}
.dve-print-header p, .dve-print-footer p { margin: 0; }
.dve-print-header table, .dve-print-footer table { width: 100%; border-collapse: collapse; }
.dve-print-footnotes { position: absolute; z-index: 1; }
.dve-print-footnote-separator {
	position: absolute;
	top: -7px;
	left: 0;
	width: 192px;
	border-top: 1px solid currentColor;
}
.dve-print-footnote { position: absolute; left: 0; width: 100%; }
.dve-print-paragraph-frame { position: absolute; top: 0; box-sizing: border-box; }
.dve-print-line-number {
	position: absolute;
	width: 40px;
	text-align: right;
	font: 10px 'Segoe UI', Arial, sans-serif;
	color: #444;
	pointer-events: none;
}
.dve-print-watermark { z-index: 0; }
@media print {
	.dve-print-pages { gap: 0 !important; zoom: 1 !important; }
	.dve-print-page {
		background: #fff !important;
		box-shadow: none !important;
		break-after: page;
	}
}
"#;

pub fn inject_print_styles<I: StringIntern>(dom: &mut Dom<'_, I>, parent: u32) -> u32 {
    let style = dom.el(parent, "style");
    dom.text(style, PRINT_CSS);
    style
}
