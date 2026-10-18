// SPDX-License-Identifier: MIT
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

/** Shown when zip / WASM size limits reject an open (PRODUCT_SPEC memory ceiling). */
export const PACKAGE_TOO_LARGE_MESSAGE =
  "This document is too large to open. Nitro XML limits package size to protect memory.";

/** Stable open-limit codes from the wasm engine (`ZipError` / `OpcError` kinds). */
export const PACKAGE_LIMIT_CODES = Object.freeze([
  "total_too_large",
  "input_too_large",
  "part_too_large",
  "too_many_parts",
  "compression_ratio",
]);

/** Map a thrown open error to the viewer message (never empty). */
export function viewerOpenErrorMessage(err) {
  const code = err && err.code;
  if (code === "unsupported_browser") return UNSUPPORTED_BROWSER_MESSAGE;
  if (code === "unsupported_format") return UNSUPPORTED_FORMAT_MESSAGE;
  if (code && PACKAGE_LIMIT_CODES.includes(code)) return PACKAGE_TOO_LARGE_MESSAGE;
  return (err && err.message) || String(err);
}

export function extensionOf(nameOrExt) {
  if (!nameOrExt) return "";
  const s = String(nameOrExt).trim().toLowerCase();
  const i = s.lastIndexOf(".");
  return (i >= 0 ? s.slice(i + 1) : s).replace(/^\./, "");
}

export function isSupportedExtension(nameOrExt) {
  return SUPPORTED_EXTENSIONS.includes(extensionOf(nameOrExt));
}
