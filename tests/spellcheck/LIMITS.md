# Spellcheck limits (#58)

Nitro does **not** bundle a dictionary or rank suggestions in Rust/WASM. Spellchecking is delegated to the **browser/OS** via a hidden `<textarea>` or `contenteditable` bridge that mirrors plain text and UTF-16 selection (`SpellBridgeSnapshot`).

## Supported behaviour

- Toggling spellcheck sets the DOM `spellcheck` property the same way upstream does on inline editors and docx surfaces (`spellCheck` / `surface.spellcheck`).
- Misspelled UTF-16 ranges reported by the host are drawn as underline rects using the same geometry as selection highlights (±1 CSS px tolerance in the gate).
- `lang` is forwarded to the bridge for `lang` / `xml:lang` on the hidden surface when the host wires it (loader integration is **TODO #69**).

## Known limits

| Area | Limit |
|------|--------|
| **Headless CI** | Chromium headless often has **no spell dictionaries**; suggestion smoke tests are optional and skip when dictionaries are unavailable. |
| **Firefox / Safari** | Gate includes manual smoke notes only; OS checker quality and suggestion UI differ by engine. |
| **xlsx formula bar** | Upstream keeps `spellcheck: false` on the formula field and name box; Nitro must match when the cell editor chrome lands (**TODO #69**). |
| **Rich text** | Single-line surface (#55) is gated here; multi-paragraph rich spellcheck follows the same bridge pattern before E3. |
| **Custom dictionaries** | Not supported; no Hunspell/LanguageTool in-tree. |

## Negative control

When `enabled` is `false`, the bridge sets `spellcheck="false"` and Rust returns **zero** underline rects even if nonsense text or injected misspell ranges are present.
