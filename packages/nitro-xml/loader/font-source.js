// SPDX-License-Identifier: MIT
/**
 * Thin forwarder for the host `fontSource` option (#121).
 * All policy (default Google Fonts base, mirror URL, fetch-off, substitutions)
 * lives in Rust/WASM (`nitro_xml::opc::fonts` + Handle.planWebfontStylesheet).
 */

/** @type {string | false | ((families: string[]) => string | null | undefined) | undefined} */
let defaultFontSource = undefined;

/**
 * Remember the process-wide default from `initCore({ fontSource })`.
 * Per-document `openDocument` / worker `open` options override this.
 * @param {unknown} fontSource
 */
export function setDefaultFontSource(fontSource) {
  defaultFontSource = /** @type {any} */ (fontSource);
}

/** @returns {typeof defaultFontSource} */
export function getDefaultFontSource() {
  return defaultFontSource;
}

/**
 * Apply a host fontSource option to a wasm Handle.
 * @param {any} handle
 * @param {unknown} [fontSource] omitted → process default → upstream Google Fonts
 */
export function applyFontSourceToHandle(handle, fontSource) {
  const value = fontSource !== undefined ? fontSource : defaultFontSource;
  if (value === undefined) {
    handle.setFontSourceDefault();
    return;
  }
  if (value === false || value === "off") {
    handle.setFontSourceOff();
    return;
  }
  if (typeof value === "string") {
    if (!value) {
      handle.setFontSourceOff();
      return;
    }
    handle.setFontSourceBaseUrl(value);
    return;
  }
  if (typeof value === "function") {
    handle.setFontSourceResolver(value);
    return;
  }
  throw new Error(
    "fontSource must be a CSS2 base URL string, false/\"off\", a resolver function, or omitted",
  );
}
