<!-- SPDX-License-Identifier: MIT -->
# Theming the chrome

**Tenet: hosts restyle Nitro's chrome without forking.** Office bones, Nitro skin: the ribbon tabs, groups, formula bar, sheet tabs, slide rail and File tab stay where Office users expect them. Only colour, type, radius and shadow come from `--nx-*` CSS custom properties, so a host changes the look with a few variables (or one `theme` value) and never touches selectors.

The theme covers **chrome only** (ribbon, embedded toolbar, title bar, panes, status bar, formula bar, menus). The painted document (slide, sheet cells, page) stays document-faithful and is never recoloured by a theme. Exports never read theme tokens.

## Choosing a theme

| Name | Look |
| --- | --- |
| `office` | Familiar: per-app accent (Excel green, PowerPoint orange-red, Word blue), coloured title bar, Segoe UI. The default of the library. |
| `quiet` | Nitro Quiet: ink `#121212`, soft `#F7F7FA` surfaces, hairline borders, Inter / system sans, near-black accent, 2 px radius, light title bar. The GitHub Pages demos default to this. |
| `dark` | Dark chrome with a restrained gold accent. |

```html
<!-- custom element attribute: a name, or a JSON object of tokens -->
<nitro-xlsx src="model.xlsx" theme="quiet"></nitro-xlsx>
<nitro-pptx src="deck.pptx" theme='{"accent":"#0b6e4f","radius":"6px"}'></nitro-pptx>
```

```js
el.theme = "quiet";                                   // 'office' | 'quiet' | 'dark'
el.theme = { "--nx-accent": "#0b6e4f", fontSans: "Georgia, serif" }; // any subset of tokens
el.theme = null;                                      // clear (back to the page's own CSS / defaults)

// Office frame (demo host): same value, applied to the frame root
const office = await import("nitro-xml/loader/lazy/ribbon/index.js");
office.mountOffice({ container, el, format: "xlsx", mode: "full", theme: "quiet", /* … */ });
```

Applying a theme sets `data-theme` (`office`, `quiet`, `dark`, or `custom` for an object) and the `--nx-*` variables inline on the element. Object keys may be `--nx-accent`, `accent` or camelCase (`fontSans`); values containing `;` or `}` are ignored. The code lives under `loader/lazy/theme.js`, so the first-load budget is unchanged; the element only imports it when `theme` is set.

You can also skip the API and set variables from CSS on the element or any ancestor:

```css
.my-app nitro-xlsx { --nx-accent: #0b6e4f; --nx-bg-subtle: #f4f1ea; --nx-radius: 4px; }
```

Inline values from `theme` win over page CSS on the same element; set one or the other.

## Tokens

| Token | Role | `office` | `quiet` | `dark` |
| --- | --- | --- | --- | --- |
| `--nx-ink` | Primary text, icons | `#242424` | `#121212` | `#f2f2f5` |
| `--nx-ink-muted` | Secondary text, group labels | `#605e5c` | `#5d6068` | `#a4a7af` |
| `--nx-bg` | Ribbon / panel / popup surface | `#fafafa` | `#ffffff` | `#161618` |
| `--nx-bg-subtle` | Tab strip, status bar, embedded toolbar, formula bar | `#f3f2f1` | `#f7f7fa` | `#0b0b0c` |
| `--nx-border` | Hairlines | `#e1dfdd` | `#dcdee3` | `#2e2e31` |
| `--nx-accent` | Active tab, File tab, focus ring, pressed state | per app | `#121212` | `#bd8c00` |
| `--nx-accent-contrast` | Text on accent | `#ffffff` | `#ffffff` | `#121212` |
| `--nx-sel` | Selected / open-menu background | derived from accent | `#e6e7eb` | `#2a2a2e` |
| `--nx-font-sans` | Chrome typeface | Segoe UI stack | Inter / system | Inter / system |
| `--nx-radius` | Control corner radius (popups use +2 px) | `4px` | `2px` | `2px` |
| `--nx-titlebar`, `--nx-titlebar-ink` | Title bar fill and text (default: accent / accent-contrast) | accent | `#fff` / `#121212` | `#161618` / `#f2f2f5` |
| `--nx-canvas` | Backdrop behind pages and slides | `#e6e6e6` | `#eceef2` | `#0b0b0c` |
| `--nx-shadow` | Popup shadow (optional) | soft | soft | soft |

Hover fills derive from `--nx-ink` and `--nx-bg` with `color-mix()`, so the minimal custom theme is `accent`, `ink`, `bg` and `font-sans`. The old embedded-toolbar variable `--nx-fg` still works as a fallback for `--nx-ink`; `--nx-bg` for the embedded bar is now `--nx-bg-subtle`.

## Rules for contributors

1. No literal chrome colours, fonts or radii in `loader/lazy/ribbon/style.js` or `embedded/style.js`; read a `--nx-*` token (with a fallback equal to the Office look).
2. Never theme the document paint. Chrome tokens must not reach canvas content.
3. A new token is added to `loader/lazy/theme.js` (`TOKENS`, presets), this table and the theme test.
4. UI PRs attach a screenshot under the default and one non-default theme.

See also [embedded-mode.md](embedded-mode.md) (toolbar parts: `::part(embedded-toolbar|embedded-group|embedded-button)`) and [ux-revision.md](ux-revision.md).
