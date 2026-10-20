// SPDX-License-Identifier: MIT
let defaultFontSource = undefined;
export function setDefaultFontSource(fontSource) {
  defaultFontSource = fontSource;
}
export function getDefaultFontSource() {
  return defaultFontSource;
}
export function applyFontSourceToHandle(handle, fontSource) {
  const value = fontSource !== undefined ? fontSource : defaultFontSource;
  handle.setFontSource(value);
}
