// SPDX-License-Identifier: MIT
// Office-style frame for a Nitro viewer element: title bar, tabs, ribbon, panes, status bar.
// Chrome is themed by --nx-* tokens (docs/theme.md; presets in ../theme.js); every colour, the font, radius
// and shadow below derives from them. Document/paint canvases are never recoloured.
export const CSS = `
.ox{container:ox/inline-size;position:relative;min-width:0;overflow:hidden;--acc:var(--nx-accent,#185abd);--acf:var(--nx-accent-contrast,#fff);--tb:var(--nx-titlebar,var(--acc));--tbf:var(--nx-titlebar-ink,var(--acf));--bg:var(--nx-bg,#fff);--rbn:var(--nx-bg,#fafafa);--tabs:var(--nx-bg-subtle,#f3f2f1);--fg:var(--nx-ink,#242424);--mut:var(--nx-ink-muted,#605e5c);--bd:var(--nx-border,#e1dfdd);--hov:color-mix(in srgb,var(--fg) 8%,var(--bg));--sel:var(--nx-sel,color-mix(in srgb,var(--acc) 14%,var(--bg)));--canvas:var(--nx-canvas,#e6e6e6);--st:var(--tabs);--ico:var(--fg);--r:var(--nx-radius,4px);--r2:calc(var(--r) + 2px);--sh:var(--nx-shadow,0 8px 24px rgba(0,0,0,.22));--fnt:var(--nx-font-sans,"Segoe UI","Segoe UI Variable Text",system-ui,-apple-system,"Helvetica Neue",Arial,sans-serif);--ox-st:var(--st);--ox-bd:var(--bd);--ox-fg:var(--fg);--ox-bg:var(--bg);--ox-canvas:var(--canvas);--ox-font:var(--fnt);
display:flex;flex-direction:column;width:100%;height:100%;min-height:0;background:var(--bg);color:var(--fg);font:12px/1.3 var(--fnt)}
.ox[data-app=pptx]{--acc:var(--nx-accent,#c43e1c)}.ox[data-app=xlsx]{--acc:var(--nx-accent,#107c41)}
.ox button{font:inherit;color:inherit}
.ox svg{width:20px;height:20px;fill:none;stroke:var(--ico);stroke-width:1.5;stroke-linecap:round;stroke-linejoin:round;flex:none}
.ox svg .a{stroke:var(--acc)}
.ox :focus-visible{outline:2px solid var(--acc);outline-offset:-2px}
.ox .tb :focus-visible{outline-color:var(--tbf)}
/* title bar */
.tb{display:flex;align-items:center;gap:6px;height:40px;padding:0 8px 0 10px;background:var(--tb);color:var(--tbf)}
.tb .ib svg,.tb .ib svg .a{stroke:var(--tbf)}
.tile{display:grid;place-items:center;width:24px;height:24px;border-radius:var(--r);background:var(--tbf);color:var(--tb);font:700 14px/1 var(--fnt);flex:none}
.tb .ib{all:unset;box-sizing:border-box;display:grid;place-items:center;width:30px;height:30px;border-radius:var(--r);cursor:pointer}
.tb .ib:hover:not(:disabled){background:color-mix(in srgb,var(--tbf) 18%,transparent)}
.tb .ib:disabled{opacity:.5;cursor:default}
.tb .ib[aria-pressed=true]{background:color-mix(in srgb,var(--tbf) 28%,transparent)}
.tb .qat{display:flex;gap:1px}
.tb .qat .ib{width:28px;height:28px}
.tb .qat .ib{position:relative}
.tb .qat .ib svg{width:18px;height:18px}
.ox[data-dirty] .qat .ib:first-child::after{content:"";position:absolute;top:3px;right:3px;width:8px;height:8px;border-radius:50%;background:#ffd166;box-shadow:0 0 0 2px var(--tb)}
.tb .sep{width:1px;height:18px;background:color-mix(in srgb,var(--tbf) 35%,transparent);margin:0 4px}
.tb .doc{display:flex;align-items:baseline;gap:8px;min-width:0;font-size:13px}
.tb .doc h1{margin:0;font:inherit;font-size:13px;font-weight:600;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:26vw}
.tb .doc i{font-style:normal;font-size:12px;white-space:nowrap;opacity:.92}
.tb .doc i[data-s=dirty]::before,.tb .doc i[data-s=clean]::before{content:"";display:inline-block;width:7px;height:7px;margin-right:6px;border-radius:50%;background:#9be4b4;vertical-align:1px}
.tb .doc i[data-s=dirty]::before{background:#ffd166;box-shadow:0 0 0 2px rgba(255,209,102,.35)}
.tb .doc i[data-s=dirty]{font-weight:600}
.tb .doc i[data-s=ro]{padding:1px 8px;border:1px solid color-mix(in srgb,var(--tbf) 55%,transparent);border-radius:10px;font-size:11px;cursor:help}
.tb .grow{flex:1 1 0}
.tb .srch{flex:0 1 420px;position:relative;display:flex;align-items:center;margin:0 auto;width:100%}
.tb .srch svg{position:absolute;left:8px;width:16px;height:16px;stroke:var(--mut);pointer-events:none}.tb .srch svg .a{stroke:var(--mut)}
.tb .srch input{all:unset;box-sizing:border-box;width:100%;height:28px;padding:0 10px 0 30px;border-radius:var(--r);background:color-mix(in srgb,var(--bg) 94%,transparent);color:var(--fg);border:1px solid color-mix(in srgb,var(--tbf) 14%,transparent);font-size:12.5px;cursor:text}
.tb .srch input::placeholder{color:var(--mut)}
.tb .srch input:focus{background:var(--bg);outline:2px solid var(--tbf);outline-offset:1px}
.tb .seg{flex:none;display:flex;border:1px solid color-mix(in srgb,var(--tbf) 55%,transparent);border-radius:var(--r);overflow:hidden}
.tb .seg button{all:unset;box-sizing:border-box;padding:4px 10px;font-size:12px;cursor:pointer}
.tb .seg button:hover{background:color-mix(in srgb,var(--tbf) 18%,transparent)}
.tb .seg button[aria-pressed=true]{background:var(--tbf);color:var(--tb)}
.tb a{white-space:nowrap;color:inherit;text-decoration:none;font-size:12px;padding:6px 8px;border-radius:var(--r)}
.tb a:hover{background:color-mix(in srgb,var(--tbf) 18%,transparent)}
/* tabs */
.tabs{display:flex;align-items:stretch;height:34px;padding:0 8px;background:var(--tabs);border-bottom:1px solid var(--bd)}
.tabs [role=tab],.tabs .file{all:unset;box-sizing:border-box;position:relative;display:flex;align-items:center;padding:0 12px;font-size:13px;cursor:pointer;color:var(--fg)}
.tabs [role=tab]:hover,.tabs .file:hover{background:var(--hov)}
.tabs [role=tab][aria-selected=true]{color:var(--acc);font-weight:600}
.tabs [role=tab][aria-selected=true]::after{content:"";position:absolute;left:10px;right:10px;bottom:0;height:3px;border-radius:2px 2px 0 0;background:var(--acc)}
.tabs .file{font-weight:600;color:var(--acf);background:var(--acc);margin-right:6px;padding:0 16px;border-radius:0}
.tabs .file:hover{background:color-mix(in srgb,var(--acc) 88%,#000)}
.tabs .file[aria-expanded=true]{background:color-mix(in srgb,var(--acc) 80%,#000)}
.tabs .grow{flex:1}
.tabs .col{all:unset;display:grid;place-items:center;width:30px;cursor:pointer;border-radius:var(--r);align-self:center;height:28px}
.tabs .col:hover{background:var(--hov)}
.tabs .col svg{width:16px;height:16px}
.fm{position:absolute;z-index:30;min-width:240px;padding:4px 0;background:var(--bg);border:1px solid var(--bd);border-radius:var(--r2);box-shadow:var(--sh)}
.fm[hidden]{display:none}
.dm{position:absolute;z-index:30;min-width:176px;padding:4px 0;background:var(--bg);border:1px solid var(--bd);border-radius:var(--r2);box-shadow:var(--sh)}
.dm[hidden]{display:none}
.dm button{all:unset;box-sizing:border-box;display:block;width:100%;padding:6px 14px;font-size:12.5px;cursor:pointer}
.dm button:hover{background:var(--hov)}
.dm button:focus-visible{outline:0;background:var(--sel);box-shadow:inset 3px 0 0 var(--acc)}
.fm button{all:unset;box-sizing:border-box;display:flex;align-items:center;gap:12px;width:100%;padding:8px 16px;font-size:13px;cursor:pointer}
.fm button:hover,.fm button:focus-visible{background:var(--hov)}
.fm small{margin-left:auto;color:var(--mut);font-size:11px}
/* ribbon */
.rbn{display:flex;align-items:stretch;gap:0;min-height:92px;padding:4px 8px 0;background:var(--rbn);border-bottom:1px solid var(--bd);overflow-x:auto;overflow-y:hidden;scrollbar-width:thin}
.rbn[hidden]{display:none}
.rg{display:flex;flex-direction:column;flex:none;padding:0 6px;border-right:1px solid var(--bd)}
.rg:last-child{border-right:0}
.rg[hidden]{display:none}
.rg .rr{display:flex;align-items:flex-start;gap:2px;flex:1;padding-top:2px}
.rg .gl{text-align:center;color:var(--mut);font-size:11.5px;padding:3px 4px 4px;white-space:nowrap}
.rg .col2{display:flex;flex-direction:column;align-self:stretch}
.rb{all:unset;box-sizing:border-box;display:flex;align-items:center;border:1px solid transparent;border-radius:var(--r);cursor:pointer;color:var(--fg)}
.rb.lg{flex-direction:column;justify-content:flex-start;gap:3px;min-width:54px;height:66px;padding:5px 6px 2px;text-align:center}
.rb.lg svg{width:32px;height:32px;stroke-width:1.3}
.rb.sm{justify-content:center;gap:4px;min-width:26px;height:26px;padding:0 3px}
.rb.sm svg{width:18px;height:18px}
.rb.sm.wide{justify-content:space-between;width:100%;min-width:118px;padding:0 6px 0 8px;border-color:var(--bd);background:var(--bg)}
.rb.sm.wide span{flex:1;text-align:left}
.rb.dd svg:last-child{width:12px;height:12px;stroke-width:2}
.rb.dd[aria-expanded=true]{background:var(--sel);border-color:var(--acc)}
.rg .col2{gap:2px;justify-content:flex-start;padding-top:1px}
.rg .r1{display:flex;align-items:center;gap:1px;min-height:26px}
.rb span{white-space:nowrap;font-size:12px}
.rb:hover:not(:disabled){background:var(--hov);border-color:var(--bd)}
.rb:active:not(:disabled){background:color-mix(in srgb,var(--hov) 70%,var(--acc))}
.rb[aria-pressed=true]{background:var(--sel);border-color:var(--acc)}
.rb:disabled{opacity:.42;cursor:default}
.rb[hidden]{display:none}
.rd{align-self:center;min-width:44px;padding:0 6px;text-align:center;font-variant-numeric:tabular-nums;color:var(--fg);font-size:12px}
.rd[hidden]{display:none}
.rn{display:flex;align-items:center;gap:8px;align-self:center;margin-left:auto;max-width:340px;padding:8px 12px;border:1px solid var(--bd);border-radius:var(--r2);background:color-mix(in srgb,var(--acc) 6%,var(--bg));color:var(--mut);font-size:12px;line-height:1.35}
.rn b{color:var(--fg);font-weight:600}
.rn svg{width:18px;height:18px}
.rn a{color:var(--acc);text-decoration:underline}
/* body */
.body{flex:1;display:flex;min-height:0;min-width:0;background:var(--canvas);position:relative}
.pane{flex:none;width:176px;overflow-y:auto;background:var(--tabs);border-right:1px solid var(--bd);padding:8px 0}
.pane[hidden]{display:none}
.pane h2{margin:0 12px 6px;font-size:11px;font-weight:600;letter-spacing:.04em;text-transform:uppercase;color:var(--mut)}
.pane button{all:unset;box-sizing:border-box;display:flex;align-items:flex-start;gap:8px;width:100%;padding:6px 10px;cursor:pointer}
.pane button:hover{background:var(--hov)}
.pane button .n{width:18px;text-align:right;color:var(--mut);padding-top:2px;font-variant-numeric:tabular-nums}
.pane button .th{position:relative;flex:1;min-width:0;aspect-ratio:16/9;display:grid;place-items:center;background:#fff;color:#605e5c;border:1px solid #c8c6c4;border-radius:2px;font-size:11px;overflow:hidden;white-space:nowrap;box-shadow:0 1px 2px rgba(0,0,0,.12)}
.pane button .ph{padding:0 6px;overflow:hidden;text-overflow:ellipsis;max-width:100%}
.pane .pl{user-select:none}.pane button[data-drag]{opacity:.55}.pane button[data-ins=before]{box-shadow:inset 0 3px 0 var(--acc)}.pane button[data-ins=after]{box-shadow:inset 0 -3px 0 var(--acc)}
.pane button[aria-current=true]{background:color-mix(in srgb,var(--acc) 12%,transparent)}
.pane button[aria-current=true] .th{border:2px solid var(--acc)}
.pane button[aria-current=true] .n{color:var(--acc);font-weight:600}
.stage{flex:1;min-width:0;min-height:0;display:flex;position:relative}
.stage>*:not(.over){flex:1;min-width:0}
.over{position:absolute;inset:0;z-index:20;display:grid;place-items:center;padding:24px;background:var(--canvas);overflow:auto}
.over[hidden]{display:none}
.drop{width:min(640px,100%);display:flex;flex-direction:column;align-items:center;gap:10px;padding:44px 24px 36px;text-align:center;background:var(--bg);border:1.5px dashed var(--bd);border-radius:12px;transition:border-color .15s,background .15s}
.over[data-drag] .drop{border-color:var(--acc);background:var(--sel)}
.drop .tile{width:52px;height:52px;border-radius:10px;font-size:28px;background:var(--acc);color:var(--acf)}
.drop h1{margin:10px 0 0;font:600 18px/1.3 inherit}
.drop p{margin:0;color:var(--mut);font-size:13px}
.drop .row{display:flex;flex-wrap:wrap;gap:10px;justify-content:center;margin-top:12px}
.btn{all:unset;box-sizing:border-box;padding:8px 18px;border-radius:var(--r);border:1px solid var(--bd);background:var(--bg);font-size:13px;cursor:pointer;color:var(--fg)}
.btn:hover{background:var(--hov)}
.btn.pri{background:var(--acc);border-color:var(--acc);color:var(--acf)}
.btn.pri:hover{background:color-mix(in srgb,var(--acc) 85%,#000)}
.drop a,.drop .lnk{all:unset;color:var(--acc);font-size:13px;text-decoration:underline;cursor:pointer;margin-top:4px}
.drop .err{margin:0;padding:8px 12px;border-radius:var(--r);background:color-mix(in srgb,#d13438 14%,var(--bg));border:1px solid #d13438;color:var(--fg);font-size:13px;max-width:100%;overflow-wrap:anywhere}
.drop .err[hidden]{display:none}
.spin{width:28px;height:28px;border-radius:50%;border:3px solid var(--bd);border-top-color:var(--acc);animation:sp .8s linear infinite}
@keyframes sp{to{transform:rotate(360deg)}}
@media(prefers-reduced-motion:reduce){.spin{animation-duration:2.4s}}
/* sheet tabs (xlsx) */
.sheets{display:flex;align-items:flex-end;gap:2px;padding:0 8px;height:30px;background:var(--st);border-top:1px solid var(--bd);overflow-x:auto}
.sheets[hidden]{display:none}
.sheets button{all:unset;box-sizing:border-box;position:relative;padding:5px 14px 6px;font-size:12.5px;cursor:pointer;color:var(--fg);white-space:nowrap}
.sheets button:hover{background:var(--hov)}
.sheets button[aria-current=true]{background:var(--bg);color:var(--acc);font-weight:600;box-shadow:inset 0 -2px 0 var(--acc)}
/* status bar */
.sb{display:flex;align-items:center;gap:14px;height:26px;padding:0 10px;background:var(--st);border-top:1px solid var(--bd);color:var(--mut);font-size:12px}
.sb .grow{flex:1}
.sb .ib{all:unset;display:grid;place-items:center;width:22px;height:22px;border-radius:3px;cursor:pointer}
.sb .ib:hover{background:var(--hov)}
.sb svg{width:14px;height:14px}
.sb input[type=range]{width:120px;accent-color:var(--acc);margin:0}
.sb .pct{min-width:40px;text-align:right;color:var(--fg);font-variant-numeric:tabular-nums}
.sb .zl{all:unset;cursor:pointer;min-width:40px;text-align:right;color:var(--fg);font-variant-numeric:tabular-nums;padding:2px 4px;border-radius:3px}
.sb .zl:hover{background:var(--hov)}
.sb .zr{display:flex;align-items:center;gap:6px}
.rgc{display:none}
@media(max-width:820px){.tb .srch,.tb .doc i,.tb .lnks{display:none}.sb input[type=range]{width:70px}.pane{width:132px}}
/* narrow frames: ribbon groups collapse to chips that open as popups (like Office's collapsed groups) */
@container ox (max-width:900px){
.rbn{min-height:0;padding:4px 6px;gap:4px;align-items:center;flex-wrap:wrap;overflow:visible}
.rbn[hidden]{display:none}
.rg{position:relative;padding:0;border:0}
.rgc{all:unset;box-sizing:border-box;display:flex;align-items:center;gap:4px;height:32px;padding:0 8px 0 10px;border:1px solid var(--bd);border-radius:var(--r);background:var(--bg);font-size:12px;white-space:nowrap;cursor:pointer;color:var(--fg)}
.rgc svg{width:12px;height:12px;stroke-width:2}
.rgc:hover,.rg[data-open] .rgc{background:var(--hov)}
.rg[data-open] .rgc{border-color:var(--acc);background:var(--sel)}
.rgc:focus-visible{outline:2px solid var(--acc);outline-offset:1px}
.rg .gl{display:none}
.rg .rr{display:none}
.rg[data-open] .rr{display:flex;position:fixed;z-index:40;max-width:calc(100vw - 8px);flex-wrap:wrap;padding:6px;background:var(--bg);border:1px solid var(--bd);border-radius:var(--r2);box-shadow:var(--sh)}
.rn{display:none}
}
@media(max-height:500px){.rbn{flex-wrap:nowrap;overflow-x:auto}}
@container ox (max-width:900px){.tb .lnks{display:none}}
@container ox (max-width:560px){.pane{display:none}.tb .seg button{padding:4px 6px}.tb .tile,.tb .sep{display:none}.tb .doc h1{max-width:30vw}.tb{gap:2px;padding:0 4px}}
@media(pointer:coarse){
.tb{height:52px}
.tb .ib,.tb .qat .ib{width:44px;height:44px}
.tb .seg button{min-height:34px;display:grid;align-items:center;padding:0 12px}
.tb a{padding:12px 10px}
.tabs{height:44px}
.tabs [role=tab],.tabs .file{padding:0 14px}
.tabs .col{width:44px;height:44px}
.rb.sm{min-width:44px;height:44px}
.rb.lg{min-width:56px;height:72px}
.rb.sm.wide{min-width:132px}
.rgc{height:44px}
.dm button,.fm button{padding:12px 16px}
.btn{min-height:44px;display:inline-flex;align-items:center}
.drop a,.drop .lnk{padding:12px 4px}
.sb{height:44px}
.sb .ib{width:44px;height:44px}
.sb .zl{min-width:48px;padding:12px 4px}
.sb input[type=range]{height:44px}
.sheets{height:44px}
.sheets button{padding:12px 16px}
.pane button{min-height:44px}
}
`;

/** Rules injected into the viewer's shadow root while the Office frame owns the chrome. */
export const SHADOW_CSS = `
.chrome .b,.chrome .t,.chrome .xve-ribbon,.chrome .xfb-save{display:none!important}
.chrome .xfb{background:var(--ox-bg,#fff);color:var(--ox-fg,#242424);border-bottom:1px solid var(--ox-bd,#e1dfdd)}
.chrome .f{display:flex;gap:6px;padding:6px 12px;background:var(--ox-st,#f3f2f1);border-bottom:1px solid var(--ox-bd,#e1dfdd);font:12px/1.3 var(--ox-font,"Segoe UI",system-ui,sans-serif);color:var(--ox-fg,#242424)}
.chrome .f input{height:26px;padding:0 8px;border:1px solid var(--ox-bd,#c8c6c4);border-radius:var(--ox-radius,4px);background:var(--ox-bg,#fff);color:inherit}
.chrome .f button{height:26px;min-width:26px;border:1px solid var(--ox-bd,#c8c6c4);border-radius:var(--ox-radius,4px);background:var(--ox-bg,#fff);color:inherit;cursor:pointer}
.chrome .d{max-height:7rem;font:11px/1.4 ui-monospace,Consolas,monospace;background:var(--ox-st,#f3f2f1);color:var(--ox-fg,#242424);border-bottom:1px solid var(--ox-bd,#e1dfdd)}
.chrome{background:var(--ox-st,#f3f2f1)}
:host{background:var(--ox-bg,#fff)}
.v{background:var(--ox-bg,#fff)}
`;
/** Page-style formats sit on a gray canvas like Word's; slides and grids keep the document background. */
export const CANVAS_CSS = ":host{background:var(--ox-canvas,#e6e6e6)}.v{background:var(--ox-canvas,#e6e6e6)}canvas{background:transparent!important}";
