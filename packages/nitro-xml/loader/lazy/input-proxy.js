// SPDX-License-Identifier: MIT
/**
 * Text-input proxy (lazy; off the eager budget). A visually hidden <textarea> inside `.v` is the
 * real focus target of an editable viewer, so the browser's text-input stack reaches the editor:
 * IME composition, dead keys / AltGr, mobile virtual keyboards, dictation, and native
 * copy / cut / paste (including the context menu and the iOS/Android selection toolbar).
 *
 * Plain printable keys and Backspace/Delete/Enter still take the existing keydown -> `editorKey`
 * path (the proxy cancels their default so the textarea never sees them twice). Everything the
 * keydown path cannot express arrives here as `beforeinput` / `compositionend` / `paste` and is
 * replayed as `editorKey` calls through the same `document.editorKey` the keydown path uses (so
 * the docx caret wrapper sees it). The textarea is parked at the caret / active cell so the IME
 * candidate window opens next to the text being edited.
 */
import { applyChromePatch } from "./cp.js";

const STYLE =
  ".nxp{position:absolute;z-index:6;margin:0;padding:0;border:0;outline:0;resize:none;overflow:hidden;" +
  "width:2px;height:1.2em;opacity:0;background:transparent;color:transparent;caret-color:transparent;" +
  "font:16px/1.2 system-ui,sans-serif;white-space:pre;pointer-events:none;left:0;top:0}";

// Keys the keydown path owns; the proxy must not also turn their input events into edits.
const OWNED = { Backspace: 1, Delete: 1, Enter: 1 };
const CMD_KEY = (e) => (e.ctrlKey || e.metaKey) && !e.altKey;

/** Split pasted/committed text into editor keys: one per code point, line breaks become Enter. */
export function keysOf(text) {
  const out = [];
  for (const ch of String(text).replace(/\r\n?/g, "\n")) out.push(ch === "\n" ? "Enter" : ch);
  return out;
}

/** Parse clipboard text as a TSV block (Excel's text/plain flavour); trailing newline ignored. */
export function parseTsv(text) {
  const t = String(text).replace(/\r\n?/g, "\n").replace(/\n$/, "");
  return t.split("\n").map((r) => r.split("\t"));
}

/** A1 / A1:B2 -> { row, col } of the top-left cell (0-based). */
export function topLeft(ref) {
  const m = /^\$?([A-Za-z]{1,3})\$?(\d+)/.exec(ref || "");
  if (!m) return null;
  let col = 0;
  for (const c of m[1].toUpperCase()) col = col * 26 + c.charCodeAt(0) - 64;
  return { row: +m[2] - 1, col: col - 1 };
}

export function attach(el) {
  if (el._nxp) return;
  const sr = el.shadowRoot;
  const v = sr && sr.querySelector(".v");
  if (!v) return;
  el._nxp = 1;
  const st = document.createElement("style");
  st.textContent = STYLE;
  sr.appendChild(st);
  const ta = document.createElement("textarea");
  ta.className = "nxp";
  ta.setAttribute("part", "input");
  ta.setAttribute("aria-label", "Document text input");
  ta.setAttribute("autocomplete", "off");
  ta.setAttribute("autocorrect", "off");
  ta.setAttribute("autocapitalize", "off");
  ta.setAttribute("spellcheck", "false");
  ta.rows = 1;
  v.appendChild(ta);
  el._nxpTa = ta;

  const editable = () => el.readonly === false && el.document;
  const mac =
    typeof navigator !== "undefined" &&
    /mac|iphone|ipad|ipod/i.test((navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "");
  let composing = false;
  let keyOwned = false; // the last keydown was a key the keydown path handles itself
  let cutKeyAt = 0;
  let queue = Promise.resolve();

  /** Where the text cursor is, in client px: docx caret bar, xlsx active cell, else top-left of the view. */
  const anchor = () => {
    const bar = sr.querySelector(".nxc");
    if (bar && bar.style.display === "block") return bar.getBoundingClientRect();
    const cell = sr.querySelector(".xg-active");
    if (cell) return cell.getBoundingClientRect();
    return v.getBoundingClientRect();
  };
  const park = () => {
    const a = anchor();
    const vr = v.getBoundingClientRect();
    ta.style.left = Math.max(0, Math.min(a.left - vr.left, vr.width - 4)) + "px";
    ta.style.top = Math.max(0, Math.min(a.top - vr.top, vr.height - 4)) + "px";
    ta.style.height = Math.max(16, a.height || 16) + "px";
  };

  /** One editor key, painted like the keydown path does (chrome refresh + frame). */
  const key = async (k, mods) => {
    const d = el.document;
    if (!d || !d.editorKey) return null;
    const r = await d.editorKey({ key: k, ...mods }, mac);
    if (r && r.handled && el._ch) applyChromePatch(el._ch, r);
    return r;
  };
  const enqueue = (fn) => {
    queue = queue.then(fn, fn).catch(() => {});
    return queue;
  };
  const typeText = (text) =>
    enqueue(async () => {
      for (const k of keysOf(text)) await key(k);
      el._tick();
    });

  // ---- focus ----
  const focus = (touch) => {
    if (!editable()) return;
    ta.inputMode = touch && el.localName === "nitro-xlsx" ? "none" : "text";
    park();
    if (el.shadowRoot.activeElement !== ta) ta.focus({ preventScroll: true });
  };
  v.addEventListener("pointerdown", (e) => e.button === 0 && e.pointerType !== "touch" && focus(false));
  // Mobile keyboards only open from a gesture end (click), not from pointerdown.
  v.addEventListener("click", (e) => focus(e.pointerType === "touch"));
  v.addEventListener("focus", () => focus(false));

  // ---- keys: stop the textarea double-inserting what the keydown path already handles ----
  ta.addEventListener("keydown", (e) => {
    if (composing || e.isComposing || e.keyCode === 229) {
      keyOwned = false;
      return;
    }
    keyOwned = !!OWNED[e.key];
    if (CMD_KEY(e) && (e.key === "x" || e.key === "X")) cutKeyAt = performance.now();
    // Printable keys are inserted by the keydown -> editorKey path; cancel the native insert
    // synchronously (the async handler cannot). Dead keys have key "Dead" and keep composing.
    // Alt / AltGr / Option chords produce text the editor path ignores: leave those to beforeinput.
    const plain = !e.ctrlKey && !e.metaKey && !e.altKey;
    if (plain && [...e.key].length === 1) {
      e.preventDefault();
      e.stopPropagation();
      if (!composing) void enqueue(() => key(e.key).then(() => el._tick()));
      return;
    }
    if (keyOwned) e.preventDefault();
  });

  // ---- beforeinput: soft keyboards, dictation, autocorrect, native edit commands ----
  ta.addEventListener("beforeinput", (e) => {
    if (!editable()) return;
    const t = e.inputType;
    if (e.isComposing || composing) return; // composition is committed at compositionend
    if (t === "insertText" || t === "insertReplacementText" || t === "insertFromDrop") {
      e.preventDefault();
      if (e.data != null) void typeText(e.data);
    } else if (t === "insertLineBreak" || t === "insertParagraph") {
      e.preventDefault();
      if (!keyOwned) void typeText("\n");
    } else if (t === "deleteContentBackward" || t === "deleteContentForward") {
      e.preventDefault();
      if (!keyOwned) void enqueue(() => key(t === "deleteContentBackward" ? "Backspace" : "Delete").then(() => el._tick()));
    } else if (t === "insertFromPaste" || t === "deleteByCut") {
      e.preventDefault(); // paste/cut events own these
    }
    keyOwned = false;
  });

  // ---- IME composition: the preedit is written into the document live (like Word) and replaced
  // by diff on every update, so Android keyboards (which compose every word) look immediate. ----
  let pre = ""; // preedit text currently in the document
  const replacePre = (next) =>
    enqueue(async () => {
      const a = Array.from(pre);
      const b = Array.from(next);
      let n = 0;
      while (n < a.length && n < b.length && a[n] === b[n]) n++;
      for (let i = n; i < a.length; i++) await key("Backspace");
      for (const k of keysOf(b.slice(n).join(""))) await key(k);
      pre = next;
      el._tick();
    });
  ta.addEventListener("compositionstart", () => {
    composing = true;
    pre = "";
    park();
  });
  ta.addEventListener("compositionupdate", (e) => {
    if (!editable()) return;
    park();
    void replacePre(e.data || "");
  });
  ta.addEventListener("compositionend", (e) => {
    composing = false;
    ta.value = "";
    if (editable()) void replacePre(e.data || "").then(() => (pre = ""));
  });

  // ---- clipboard ----
  const copyOut = () => el.shellAction("copy");
  ta.addEventListener("copy", (e) => {
    if (!editable()) return;
    e.preventDefault();
    void copyOut();
  });
  ta.addEventListener("cut", (e) => {
    if (!editable()) return;
    e.preventDefault();
    const viaKey = performance.now() - cutKeyAt < 400; // Ctrl+X already reached the keydown path
    void copyOut().then(() => !viaKey && enqueue(() => key("Delete").then(() => el._tick())));
  });
  ta.addEventListener("paste", (e) => {
    if (!editable()) return;
    e.preventDefault();
    const text = e.clipboardData && e.clipboardData.getData("text/plain");
    if (text) void pasteText(text);
  });
  const pasteText = (text) =>
    enqueue(async () => {
      const d = el.document;
      if (el.localName === "nitro-xlsx") {
        const ui = (await el.shellAction("ui")).ui || {};
        const at = topLeft(ui.selection);
        const tab = (ui.tabs || []).find((t) => t.active);
        if (!at || !d.applyEdit) return;
        const rows = parseTsv(text);
        let last = null;
        for (let r = 0; r < rows.length; r++) {
          for (let c = 0; c < rows[r].length; c++) {
            last = await d.applyEdit(
              JSON.stringify({ command: "setCellInput", sheet: tab ? tab.index : 0, row: at.row + r, col: at.col + c, text: rows[r][c] }),
            );
          }
        }
        let patch = null;
        try {
          patch = last && JSON.parse(last);
        } catch {
          /* chrome payload is optional */
        }
        if (patch && el._ch) applyChromePatch(el._ch, patch);
        el._tick();
        return;
      }
      for (const k of keysOf(text)) await key(k);
      el._tick();
    });
  el._nxpPaste = pasteText;
}
