// SPDX-License-Identifier: MIT
/** Format allow-list shared with #13 (extension is a hint; engine checks content types). */

export const SUPPORTED_EXTENSIONS = Object.freeze([
  "docx",
  "docm",
  "dotx",
  "dotm",
  "xlsx",
  "xlsm",
  "xltx",
  "xltm",
  "pptx",
  "pptm",
  "potx",
  "potm",
]);

export const ACCEPT =
  ".docx,.docm,.dotx,.dotm,.xlsx,.xlsm,.xltx,.xltm,.pptx,.pptm,.potx,.potm";

export const UNSUPPORTED_BROWSER_MESSAGE =
  "Unsupported browser: Nitro XML needs WebAssembly with 'wasm-unsafe-eval' in Content-Security-Policy.";

export const UNSUPPORTED_FORMAT_MESSAGE =
  "Unsupported format. Nitro XML opens Word, Excel and PowerPoint documents (.docx, .xlsx, .pptx), including macro-enabled and template files (.docm, .xlsm, .pptm, .dotx, .xltx, .potx, .dotm, .xltm, .potm). Legacy .doc/.xls/.ppt, encrypted files, CSV and other formats are not supported.";

export function extensionOf(nameOrExt) {
  if (!nameOrExt) return "";
  const s = String(nameOrExt).trim().toLowerCase();
  const i = s.lastIndexOf(".");
  return (i >= 0 ? s.slice(i + 1) : s).replace(/^\./, "");
}

export function isSupportedExtension(nameOrExt) {
  return SUPPORTED_EXTENSIONS.includes(extensionOf(nameOrExt));
}
