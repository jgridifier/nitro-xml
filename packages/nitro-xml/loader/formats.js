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

export function extensionOf(nameOrExt) {
  if (!nameOrExt) return "";
  const s = String(nameOrExt).trim().toLowerCase();
  const i = s.lastIndexOf(".");
  return (i >= 0 ? s.slice(i + 1) : s).replace(/^\./, "");
}

export function isSupportedExtension(nameOrExt) {
  return SUPPORTED_EXTENSIONS.includes(extensionOf(nameOrExt));
}

export const PACKAGE_TOO_LARGE_MESSAGE = "Package too large.";
export const PACKAGE_LIMIT_CODES = "total_too_large,input_too_large,part_too_large,too_many_parts,compression_ratio".split(",");
export const VIEWER_RECOVERY_FAILED_MESSAGE =
  "Could not reopen this document after a crash.";
export const EDITOR_RECOVERY_FAILED_MESSAGE =
  "Could not restore edits after a crash.";
export const EDITOR_RECOVERY_LAST_SAVE_MESSAGE =
  "Could not restore edits after a crash. A previous save is available.";
export function viewerOpenErrorMessage(err) {
  const c = err && err.code;
  if (c === "unsupported_browser") return UNSUPPORTED_BROWSER_MESSAGE;
  if (c === "unsupported_format") return UNSUPPORTED_FORMAT_MESSAGE;
  if (c && PACKAGE_LIMIT_CODES.includes(c)) return PACKAGE_TOO_LARGE_MESSAGE;
  if (c === "viewer_recovery_exhausted" || c === "viewer_recovery_failed") {
    return VIEWER_RECOVERY_FAILED_MESSAGE;
  }
  if (c === "editor_recovery_last_save") return EDITOR_RECOVERY_LAST_SAVE_MESSAGE;
  if (c === "editor_recovery_exhausted" || c === "editor_recovery_failed") {
    return EDITOR_RECOVERY_FAILED_MESSAGE;
  }
  return (err && err.message) || String(err);
}
