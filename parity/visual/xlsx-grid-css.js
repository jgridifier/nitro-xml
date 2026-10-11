// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/styles.ts; changes listed in PROVENANCE.md.

/** Shadow-grid stylesheet (upstream GRID_CSS). */
export const GRID_CSS = `
.xg-root [hidden]{display:none!important}
.xg-root{position:relative;overflow:hidden;width:100%;height:100%;min-height:0;outline:none;
	background:var(--xve-sheet-bg,#fff);color:#000;font-family:"Segoe UI",system-ui,sans-serif;
	-webkit-user-select:none;user-select:none;contain:strict}
.xg-scroller{position:absolute;inset:0;overflow:auto;overscroll-behavior:contain;touch-action:pan-x pan-y}
.xg-space{position:relative}
.xg-view{position:sticky;left:0;top:0;overflow:hidden;cursor:cell;touch-action:pan-x pan-y}
.xg-q,.xg-hbox{position:absolute;overflow:hidden}
.xg-plane{position:absolute;left:0;top:0;will-change:transform}
.xg-cells,.xg-lines,.xg-overlay,.xg-drawings{position:absolute;left:0;top:0}
.xg-gl{position:absolute;background:var(--xve-grid-line,#e1e1e1)}
.xg-c{position:absolute;box-sizing:border-box;overflow:visible}
.xg-c[hidden],.xg-gl[hidden],.xg-hd[hidden]{display:none}
.xg-c-over{z-index:1}
.xg-t{position:absolute;top:0;bottom:0;display:flex;box-sizing:border-box;overflow:hidden;line-height:1.15}
.xg-t.xg-wrap{white-space:pre-wrap;overflow-wrap:anywhere}
.xg-t.xg-wrap>.xg-tx{display:block;width:100%}
.xg-t.xg-vert{writing-mode:vertical-rl;text-orientation:upright}
.xg-t.xg-over{background:var(--xve-sheet-bg,#fff)}
.xg-c[style*="background-color"]>.xg-t.xg-over,.xg-c-fill>.xg-t.xg-over{background:transparent}
.xg-tx{display:inline-block;white-space:inherit}
.xg-link{cursor:pointer}
.xg-b{position:absolute;left:-1px;top:-1px;box-sizing:border-box;pointer-events:none}
.xg-diag{position:absolute;left:0;top:0;pointer-events:none}
.xg-spark{position:absolute;left:0;top:0;pointer-events:none;overflow:hidden}
.xg-db{position:absolute;left:2px;top:2px;bottom:2px;border:1px solid;box-sizing:border-box}
.xg-ic{position:absolute;left:2px;top:50%;transform:translateY(-50%)}
.xg-cm{position:absolute;right:0;top:0;width:0;height:0;border-left:6px solid transparent;border-top:6px solid #d13438;z-index:2}
.xg-er{position:absolute;left:0;top:0;width:0;height:0;border-right:6px solid transparent;border-top:6px solid #107c41;z-index:2}
.xg-overlay{pointer-events:none;z-index:3}
.xg-shade{position:absolute;background:var(--xve-selection,rgba(33,115,70,.12))}
.xg-outline{position:absolute;box-sizing:border-box;border:2px solid var(--xve-selection-border,#217346)}
.xg-active{position:absolute;box-sizing:border-box;border:1px solid var(--xve-selection-border,#217346)}
.xg-handle{position:absolute;box-sizing:border-box;background:var(--xve-selection-border,#217346);border:1px solid #fff;pointer-events:auto;cursor:crosshair}
.xg-ants{position:absolute;box-sizing:border-box;--a:var(--xve-selection-border,#217346);
	background-image:linear-gradient(90deg,var(--a) 50%,transparent 50%),linear-gradient(90deg,var(--a) 50%,transparent 50%),linear-gradient(0deg,var(--a) 50%,transparent 50%),linear-gradient(0deg,var(--a) 50%,transparent 50%);
	background-repeat:repeat-x,repeat-x,repeat-y,repeat-y;background-size:8px 2px,8px 2px,2px 8px,2px 8px;
	background-position:0 0,0 100%,0 0,100% 0;animation:xg-ants .5s linear infinite}
@keyframes xg-ants{to{background-position:8px 0,-8px 100%,0 -8px,100% 8px}}
@media (prefers-reduced-motion:reduce){.xg-ants{animation:none}}
.xg-fill-preview{position:absolute;box-sizing:border-box;border:1px dashed #666}
.xg-refbox{position:absolute;box-sizing:border-box;border:2px solid}
.xg-remote-layer{position:absolute;left:0;top:0;pointer-events:none;z-index:3}
.xg-remote{position:absolute;box-sizing:border-box;border:2px solid}
.xg-remote[hidden]{display:none}
.xg-remote-tag{position:absolute;left:-2px;bottom:100%;max-width:160px;padding:0 4px;height:16px;line-height:16px;
	font:600 11px/16px var(--xve-font,system-ui,sans-serif);color:#fff;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;border-radius:2px 2px 2px 0}
.xg-remote[data-tag="inside"] .xg-remote-tag{bottom:auto;top:0;left:0;border-radius:0 0 2px 0}
.xg-hbox{background:var(--xve-header-bg,#f3f3f3);z-index:4}
.xg-hdr-plane{position:absolute;left:0;top:0;will-change:transform}
.xg-hd{position:absolute;box-sizing:border-box;display:flex;align-items:center;justify-content:center;
	color:var(--xve-header-fg,#444);border-right:1px solid var(--xve-header-line,#d4d4d4);border-bottom:1px solid var(--xve-header-line,#d4d4d4);overflow:hidden;white-space:nowrap}
.xg-hdr-col .xg-hd{top:0;height:100%}
.xg-hdr-row .xg-hd{left:0;width:100%}
.xg-hd-sel{background:var(--xve-header-active-bg,#e1e1e1);color:var(--xve-selection-border,#217346);font-weight:600}
.xg-hdr-col .xg-hd-sel{box-shadow:inset 0 -2px 0 var(--xve-selection-border,#217346)}
.xg-hdr-row .xg-hd-sel{box-shadow:inset -2px 0 0 var(--xve-selection-border,#217346)}
.xg-hd-full{background:var(--xve-header-full-bg,#cde6d7);color:var(--xve-selection-border,#185c37);font-weight:600}
.xg-hdr-col{cursor:s-resize}.xg-hdr-row{cursor:e-resize}
.xg-hdr-col .xg-hd,.xg-hdr-row .xg-hd{cursor:inherit}
.xg-corner{position:absolute;left:0;top:0;background:var(--xve-header-bg,#f3f3f3);z-index:5;box-sizing:border-box;
	border-right:1px solid var(--xve-header-line,#d4d4d4);border-bottom:1px solid var(--xve-header-line,#d4d4d4);cursor:default}
.xg-corner::after{content:"";position:absolute;right:3px;bottom:3px;border-left:9px solid transparent;border-bottom:9px solid #b4b4b4}
.xg-freeze-h,.xg-freeze-v{position:absolute;background:var(--xve-freeze-line,#9b9b9b);z-index:6;pointer-events:none}
.xg-col-resize{cursor:col-resize!important}.xg-row-resize{cursor:row-resize!important}
.xg-resize-guide{position:absolute;z-index:8;border:0 dashed #666;pointer-events:none}
.xg-editor-host{position:absolute;z-index:7;box-sizing:border-box;opacity:0;overflow:hidden;pointer-events:none;background:#fff;color:#000;line-height:1.25}
.xg-editor-host.xg-editing{opacity:1;pointer-events:auto;overflow:visible;cursor:text;
	box-shadow:0 0 0 2px var(--xve-selection-border,#217346),0 2px 6px rgba(0,0,0,.25)}
.xg-editor-host .xg-field-backdrop,.xg-editor-host .xg-field-input{padding:1px 3px}
.xg-sink{position:absolute;left:0;top:0;width:1px;height:1px;opacity:0;padding:0;border:0;resize:none;overflow:hidden;pointer-events:none}
.xg-field{position:relative}
.xg-field-backdrop,.xg-field-input{margin:0;border:0;box-sizing:border-box;font:inherit;line-height:inherit;letter-spacing:inherit;
	padding:inherit;white-space:pre-wrap;overflow-wrap:anywhere;word-break:normal;tab-size:4}
.xg-field-backdrop{position:absolute;inset:0;overflow:hidden;color:inherit;pointer-events:none;visibility:hidden}
.xg-field.xg-formula .xg-field-backdrop{visibility:visible}
.xg-field-input{position:relative;display:block;width:100%;height:100%;resize:none;outline:none;background:transparent;color:inherit;overflow:hidden}
.xg-field.xg-formula .xg-field-input{color:transparent;caret-color:#000}
.xg-ref{font:inherit}
.xg-editor-host .xg-field{width:100%;height:100%}
.xg-dd{position:absolute;z-index:6;width:17px;height:17px;box-sizing:border-box;border:1px solid #c8c8c8;background:#f3f3f3;
	display:flex;align-items:center;justify-content:center;cursor:default;pointer-events:auto}
.xg-dd::before{content:"";border-left:4px solid transparent;border-right:4px solid transparent;border-top:5px solid #444}
.xg-popup{position:absolute;z-index:20;background:var(--xve-popover,#fff);color:var(--xve-popover-foreground,#222);
	border:1px solid var(--xve-border,#c8c8c8);box-shadow:0 4px 12px rgba(0,0,0,.18);font-size:13px;border-radius:4px}
.xg-list{max-height:220px;overflow:auto;min-width:120px;padding:2px 0;margin:0;list-style:none}
.xg-list [role=option]{padding:3px 10px;cursor:default;white-space:nowrap}
.xg-list [aria-selected=true]{background:var(--xve-accent,#e1efe6)}
.xg-tip{padding:6px 8px;max-width:260px;white-space:pre-wrap;background:#fffbe6;color:#222;pointer-events:none}
.xg-tip b{display:block;margin-bottom:2px}
.xg-alert{padding:12px 14px;max-width:340px}
.xg-alert h3{margin:0 0 6px;font-size:14px}
.xg-alert p{margin:0 0 10px;white-space:pre-wrap}
.xg-alert .xg-actions{display:flex;gap:6px;justify-content:flex-end}
.xg-alert button{min-width:64px;padding:4px 10px;border:1px solid var(--xve-border,#c8c8c8);border-radius:3px;background:var(--xve-secondary,#f3f3f3);color:inherit;font:inherit}
.xg-alert button:first-child{background:var(--xve-selection-border,#217346);color:#fff;border-color:transparent}
.xg-drawings{z-index:4}
.xg-obj{position:absolute;box-sizing:border-box;pointer-events:auto;cursor:default}
.xg-obj img,.xg-obj svg{display:block;width:100%;height:100%;pointer-events:none}
.xg-chart svg [data-chart-series],.xg-chart svg [data-chart-part]{pointer-events:auto}
.xg-obj.xg-chart{background:#fff;outline:1px solid #d9d9d9;outline-offset:-1px}
.xg-obj.xg-obj-sel{outline:1px solid #8a8a8a;outline-offset:0}
.xg-obj .xg-grip{position:absolute;width:8px;height:8px;background:#fff;border:1px solid #555;box-sizing:border-box;border-radius:50%}
.xg-obj-unsupported{background:repeating-linear-gradient(45deg,#f6f6f6,#f6f6f6 6px,#ececec 6px,#ececec 12px);border:1px dashed #aaa;
	font-size:11px;color:#555;display:flex;align-items:center;justify-content:center;text-align:center}
.xg-obj.xg-smartart{overflow:hidden}
.xg-obj.xg-smartart office-ui-smartart{display:block;pointer-events:none}
.xg-smartart-placeholder{background:#fafafa;border:1px dashed #aaa;font-size:11px;color:#333;padding:4px;overflow:auto}
.xg-smartart-nodes{margin:0;padding-left:16px}
.xg-smartart-nodes .xg-smartart-child{margin-left:12px}
.xg-smartart-notice{display:none;position:absolute;left:0;bottom:0;padding:1px 4px;font-size:10px;background:rgba(255,255,255,.9);color:#555;border-top-right-radius:3px}
.xg-obj-sel .xg-smartart-notice,.xg-smartart-placeholder .xg-smartart-notice{display:block}
.xg-live{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
`;
