// SPDX-License-Identifier: MIT
// Embedded toolbar skin. Theme with --nx-* custom properties or ::part(embedded-*).
export const CSS = `
.nx{--a:var(--nx-accent,#c43e1c);--bg:var(--nx-bg,#f3f2f1);--fg:var(--nx-fg,#242424);--bd:var(--nx-border,#d1d1d1);position:relative;display:flex;align-items:stretch;flex:none;overflow:visible;background:var(--bg);color:var(--fg);border-bottom:1px solid var(--bd);font:12px/1.2 "Segoe UI",system-ui,-apple-system,sans-serif;min-width:0}
.nx[data-fmt=docx]{--a:#185abd}.nx[data-fmt=xlsx]{--a:#107c41}
.nx-g{display:flex;flex-direction:column;flex:none;justify-content:space-between;padding:4px 6px 2px;border-right:1px solid var(--bd)}
.nx-g[hidden],.nx-r[hidden],.nx-b[hidden],.nx-more[hidden],.nx-pop[hidden]{display:none}
.nx-row{display:flex;align-items:flex-start;gap:2px}
.nx-cap{text-align:center;color:#616161;font-size:10.5px;padding-top:2px}
.nx-b{all:unset;box-sizing:border-box;display:flex;flex-direction:column;align-items:center;gap:2px;min-width:44px;padding:4px 6px;border:1px solid transparent;border-radius:3px;cursor:pointer;color:inherit;font:inherit}
.nx-b svg{width:22px;height:22px;fill:none;stroke:currentColor;stroke-width:1.7;stroke-linecap:round;stroke-linejoin:round}
.nx-b:hover:not(:disabled){background:#e6e6e6;border-color:#c8c8c8}
.nx-b[aria-pressed=true]{background:color-mix(in srgb,var(--a) 16%,#fff);border-color:var(--a)}
.nx-b:focus-visible{outline:2px solid var(--a);outline-offset:1px}
.nx-b:disabled{opacity:.4;cursor:default}
.nx-r{align-self:center;min-width:3.4em;text-align:center;font-variant-numeric:tabular-nums;padding:0 2px}
.nx[data-size=compact] .nx-l,.nx[data-size=compact] .nx-cap{display:none}
.nx[data-size=compact] .nx-b{min-width:28px;padding:4px 3px}
.nx[data-size=compact] .nx-g{padding-left:4px;padding-right:4px}
.nx[data-size=compact] .nx-b svg{width:20px;height:20px}
.nx[data-size=compact] .nx-r{min-width:3em}
.nx[data-size=compact] .nx-g{justify-content:center;padding-bottom:4px}
.nx-more{position:relative;display:flex;align-items:center;margin-left:auto;padding:4px}
.nx-mb .nx-l{display:none}
.nx-pop{position:absolute;right:0;top:100%;z-index:10;display:flex;flex-direction:column;gap:6px;padding:6px;background:var(--bg);border:1px solid var(--bd);box-shadow:0 4px 14px rgba(0,0,0,.2)}
.nx-pop .nx-g{border:0;padding:0}
.nx-pop .nx-cap{display:block;text-align:left;padding:0 0 2px}
.nx-pop .nx-row{flex-wrap:wrap}
.nx-pop .nx-b{flex-direction:row;gap:6px}
.nx-pop .nx-l{display:inline}
:host([data-nx]) .chrome .b,:host([data-nx]) .chrome .d,:host([data-nx]) .chrome .xve-ribbon{display:none!important}
:host([data-nx=pptx]) .chrome .t,:host([data-nx=docx]) .chrome .t{display:none}
.chrome .f{background:var(--nx-bg,#f3f2f1)}
`;
