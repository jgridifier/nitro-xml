// SPDX-License-Identifier: MIT
// Embedded toolbar skin: a single-row "simplified ribbon" (icon buttons, thin dividers, More menu).
// Theme with --nx-* custom properties (docs/theme.md) or ::part(embedded-*).
export const CSS = `
.nx{--a:var(--nx-accent,#c43e1c);--bg:var(--nx-bg-subtle,#f3f2f1);--fg:var(--nx-ink,var(--nx-fg,#242424));--bd:var(--nx-border,#d1d1d1);--r:var(--nx-radius,4px);--sh:var(--nx-shadow,0 8px 24px rgba(0,0,0,.22));--hv:color-mix(in srgb,var(--fg) 9%,transparent);position:relative;display:flex;align-items:center;flex:none;overflow:visible;gap:2px;height:40px;padding:0 6px;background:var(--bg);color:var(--fg);border-bottom:1px solid var(--bd);font:12px/1.2 var(--nx-font-sans,"Segoe UI","Segoe UI Variable Text",system-ui,-apple-system,sans-serif);min-width:0}
.nx[data-fmt=docx]{--a:var(--nx-accent,#185abd)}.nx[data-fmt=xlsx]{--a:var(--nx-accent,#107c41)}
.nx-g{display:flex;align-items:center;flex:none;gap:1px;padding:0 4px 0 0;margin-right:4px;position:relative}
.nx-g:not(:last-of-type)::after{content:"";position:absolute;right:0;top:9px;bottom:9px;width:1px;background:var(--bd)}
.nx-g[hidden],.nx-r[hidden],.nx-b[hidden],.nx-more[hidden],.nx-pop[hidden]{display:none}
.nx-row{display:flex;align-items:center;gap:1px}
.nx-cap{display:none}
.nx-b{all:unset;box-sizing:border-box;display:flex;align-items:center;justify-content:center;gap:6px;min-width:32px;height:32px;padding:0 6px;border:1px solid transparent;border-radius:var(--r);cursor:pointer;color:inherit;font:inherit}
.nx-b svg{width:20px;height:20px;fill:none;stroke:currentColor;stroke-width:1.5;stroke-linecap:round;stroke-linejoin:round}
.nx-b svg .a{stroke:var(--a)}
.nx-l{display:none}
.nx-b:hover:not(:disabled){background:var(--hv)}
.nx-b:active:not(:disabled){background:color-mix(in srgb,var(--fg) 16%,transparent)}
.nx-b[aria-pressed=true]{background:color-mix(in srgb,var(--a) 16%,var(--bg));border-color:var(--a)}
.nx-b:focus-visible{outline:2px solid var(--a);outline-offset:-1px}
.nx-b:disabled{opacity:.4;cursor:default}
.nx-r{min-width:3.2em;text-align:center;font-variant-numeric:tabular-nums;padding:0 2px}
.nx[data-size=compact] .nx-b{min-width:28px;padding:0 4px}
.nx[data-size=compact] .nx-r{min-width:2.8em}
.nx-more{position:relative;display:flex;align-items:center;margin-left:auto}
.nx-pop{position:absolute;right:0;top:100%;z-index:10;display:flex;flex-direction:column;gap:6px;padding:6px;background:var(--bg);border:1px solid var(--bd);border-radius:calc(var(--r) + 2px);box-shadow:var(--sh)}
.nx-pop .nx-g{padding:0;margin:0;flex-direction:column;align-items:stretch}
.nx-pop .nx-g::after{display:none}
.nx-pop .nx-cap{display:block;text-align:left;color:color-mix(in srgb,var(--fg) 65%,transparent);font-size:10.5px;padding:0 0 2px}
.nx-pop .nx-row{flex-wrap:wrap}
.nx-pop .nx-b{justify-content:flex-start}
.nx-pop .nx-l{display:inline}
:host([data-nx]) .chrome .b,:host([data-nx]) .chrome .d,:host([data-nx]) .chrome .xve-ribbon,:host([data-nx]) .chrome .xfb-save{display:none!important}
:host([data-nx=pptx]) .chrome .t,:host([data-nx=docx]) .chrome .t{display:none}
@media(pointer:coarse){
.nx{height:52px}
.nx-b{min-width:44px;height:44px}
.nx[data-size=compact] .nx-b{min-width:40px}
.nx-g:not(:last-of-type)::after{top:12px;bottom:12px}
.chrome .t button{padding:13px 14px}
.chrome .f input,.chrome .f button{height:44px}
.chrome .f button{min-width:44px}
}
@media(max-width:480px){.nx{padding:0 4px}.nx-g{margin-right:2px;padding-right:2px}}
.chrome .f{background:var(--nx-bg-subtle,#f3f2f1)}
.chrome .t{gap:2px;padding:0 6px;border-top:1px solid var(--nx-border,#d1d1d1);border-bottom:0;background:var(--nx-bg-subtle,#f3f2f1);font:12px/1.2 var(--nx-font-sans,"Segoe UI",system-ui,sans-serif)}
.chrome .t button{font:inherit;padding:5px 12px;border:0;border-radius:0;background:transparent;color:var(--nx-ink,var(--nx-fg,#242424));cursor:pointer}
.chrome .t button:hover{background:color-mix(in srgb,var(--nx-ink,var(--nx-fg,#242424)) 9%,transparent)}
.chrome .t button[aria-current=true]{color:var(--nx-accent,#107c41);box-shadow:inset 0 -2px 0 var(--nx-accent,#107c41)}
`;
