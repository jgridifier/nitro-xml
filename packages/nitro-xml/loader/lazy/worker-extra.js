// SPDX-License-Identifier: MIT
export function v(handle, msg) {
  if (msg.type === "cell-text") {
    return { type: "cell-text", result: handle.cellText(msg.sheet | 0, msg.range || "") };
  }
  if (msg.type === "editor-key") {
    return { type: "editor-key", result: handle.editorKey(msg.key || "", !!msg.ctrl, !!msg.meta, !!msg.alt, !!msg.shift, !!msg.mac) };
  }
  return { type: "match-shortcut", result: handle.matchShortcut(msg.key || "", msg.code || "", !!msg.ctrl, !!msg.meta, !!msg.alt, !!msg.shift, !!msg.mac) };
}
