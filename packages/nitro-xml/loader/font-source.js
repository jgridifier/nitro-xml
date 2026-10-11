// SPDX-License-Identifier: MIT
/** Thin fontSource forwarder; policy lives in Rust Handle.setFontSource. */

let defaultFontSource = undefined;

export function setDefaultFontSource(fontSource) {
  defaultFontSource = fontSource;
}

export function getDefaultFontSource() {
  return defaultFontSource;
}

/** Apply host fontSource to a wasm Handle (omitted → process default → Google Fonts). */
export function applyFontSourceToHandle(handle, fontSource) {
  const value = fontSource !== undefined ? fontSource : defaultFontSource;
  handle.setFontSource(value);
}
