// SPDX-License-Identifier: MIT
// Chrome theme: `--nx-*` custom properties on the host element. Loaded by dynamic import() when a
// theme is requested (attribute/property/mount option); lives under loader/lazy/ so it never counts
// toward the first-load budget. Themes restyle chrome only; the paint canvas stays document-faithful.

/** Documented token names (without the `--nx-` prefix). See docs/theme.md. */
export const TOKENS = [
  "ink", "ink-muted", "bg", "bg-subtle", "border", "accent", "accent-contrast", "sel",
  "font-sans", "radius", "space", "titlebar", "titlebar-ink", "canvas",
];

const SANS = 'Inter,"Segoe UI Variable Text","Segoe UI",system-ui,-apple-system,"Helvetica Neue",Arial,sans-serif';

/** Built-in themes. `office` leaves accent unset so each app keeps its familiar Office colour. */
export const THEMES = {
  office: {
    ink: "#242424", "ink-muted": "#605e5c", bg: "#ffffff", "bg-subtle": "#f3f2f1", border: "#e1dfdd",
    "accent-contrast": "#ffffff", sel: "color-mix(in srgb,var(--acc,#185abd) 14%,#fff)",
    "font-sans": '"Segoe UI","Segoe UI Variable Text",system-ui,-apple-system,"Helvetica Neue",Arial,sans-serif',
    radius: "4px", space: "8px", canvas: "#e6e6e6",
  },
  quiet: {
    ink: "#121212", "ink-muted": "#5d6068", bg: "#ffffff", "bg-subtle": "#f7f7fa", border: "#dcdee3",
    accent: "#121212", "accent-contrast": "#ffffff", sel: "#e6e7eb", "font-sans": SANS,
    radius: "2px", space: "8px", titlebar: "#ffffff", "titlebar-ink": "#121212", canvas: "#eceef2",
  },
  dark: {
    ink: "#f2f2f5", "ink-muted": "#a4a7af", bg: "#161618", "bg-subtle": "#0b0b0c", border: "#2e2e31",
    accent: "#bd8c00", "accent-contrast": "#121212", sel: "#2a2a2e", "font-sans": SANS,
    radius: "2px", space: "8px", titlebar: "#161618", "titlebar-ink": "#f2f2f5", canvas: "#0b0b0c",
  },
};

const norm = (k) => (k.startsWith("--nx-") ? k.slice(5) : k.replace(/[A-Z]/g, (c) => "-" + c.toLowerCase()));

/** Resolve `'office'|'quiet'|'dark'|object|JSON string` to `{ name, vars }` (vars keyed by token, no prefix). */
export function resolveTheme(t) {
  if (typeof t === "string" && t.trim().startsWith("{")) t = JSON.parse(t);
  if (t == null || t === "") return { name: "", vars: {} };
  if (typeof t === "string") {
    if (!THEMES[t]) throw new Error(`nitro-xml: unknown theme "${t}" (use ${Object.keys(THEMES).join(", ")} or an object of --nx-* tokens)`);
    return { name: t, vars: THEMES[t] };
  }
  const vars = {};
  for (const [k, v] of Object.entries(t)) if (typeof v === "string" && v && !/[;}]/.test(v)) vars[norm(k)] = v;
  return { name: "custom", vars };
}

/**
 * Apply a theme to a host element (or the Office frame root): sets `data-theme` and the `--nx-*`
 * variables inline. An object theme sets only the tokens it names, on top of whatever the page's
 * CSS provides. Pass a falsy theme to clear. Returns the applied token map.
 */
export function applyTheme(el, theme) {
  for (const k of el._nxTheme || []) el.style.removeProperty("--nx-" + k);
  const { name, vars } = resolveTheme(theme);
  el._nxTheme = Object.keys(vars);
  for (const [k, v] of Object.entries(vars)) el.style.setProperty("--nx-" + k, v);
  if (name) el.setAttribute("data-theme", name);
  else el.removeAttribute("data-theme");
  return vars;
}
