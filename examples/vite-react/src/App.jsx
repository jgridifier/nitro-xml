// SPDX-License-Identifier: MIT
import { useCallback, useMemo, useRef, useState } from "react";
import { NitroViewer } from "@nitro-xml/react";

const FORMATS = [
  { id: "xlsx", name: "cells_1k.xlsx", url: "/fixtures/cells_1k.xlsx" },
  { id: "docx", name: "pages_10.docx", url: "/fixtures/pages_10.docx" },
  { id: "pptx", name: "slides_10.pptx", url: "/fixtures/slides_10.pptx" },
];

export function App() {
  const [format, setFormat] = useState("xlsx");
  const [ready, setReady] = useState(false);
  const [savedBytes, setSavedBytes] = useState(0);
  const [status, setStatus] = useState("idle");
  const viewerRef = useRef(null);

  const entry = useMemo(
    () => FORMATS.find((f) => f.id === format) || FORMATS[0],
    [format],
  );

  const onReady = useCallback(() => {
    setReady(true);
    setStatus("ready");
  }, []);

  const onChange = useCallback((bytes) => {
    setSavedBytes(bytes.byteLength);
    setStatus(`saved:${bytes.byteLength}`);
  }, []);

  async function runEditAndSave() {
    const v = viewerRef.current;
    if (!v) return;
    setStatus("editing");
    if (format === "xlsx") {
      await v.applyEdit({
        command: "setCellInput",
        sheet: 0,
        row: 0,
        col: 0,
        text: "NxNpm",
      });
    } else {
      // docx / pptx: type a marker via editorKey (printable path)
      const el = v.element;
      el?.focus?.();
      await el?.document?.editorKey?.(
        { key: "N", ctrlKey: false, metaKey: false, altKey: false, shiftKey: false },
        false,
      );
      await el?.document?.editorKey?.(
        { key: "x", ctrlKey: false, metaKey: false, altKey: false, shiftKey: false },
        false,
      );
    }
    const bytes = await v.save();
    setSavedBytes(bytes.byteLength);
    setStatus(`saved:${bytes.byteLength}`);
    // also surface for Playwright
    window.__nitroLastSave = bytes;
  }

  return (
    <div style={{ fontFamily: "system-ui, sans-serif", padding: 16 }}>
      <h1>Nitro XML · Vite + React</h1>
      <p>
        Example host for <code>@nitro-xml/react</code>. Eager loader budget is
        unchanged (React lives in a separate package).
      </p>
      <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
        {FORMATS.map((f) => (
          <button
            key={f.id}
            type="button"
            data-testid={`format-${f.id}`}
            aria-pressed={format === f.id}
            onClick={() => {
              setReady(false);
              setSavedBytes(0);
              setStatus("loading");
              setFormat(f.id);
            }}
          >
            {f.id}
          </button>
        ))}
        <button
          type="button"
          data-testid="edit-save"
          disabled={!ready}
          onClick={() => void runEditAndSave()}
        >
          Edit + save
        </button>
      </div>
      <div data-testid="status">
        {status} · savedBytes={savedBytes} · ready={ready ? "yes" : "no"}
      </div>
      <div style={{ height: 520, border: "1px solid #ccc", marginTop: 12 }}>
        <NitroViewer
          key={format}
          ref={viewerRef}
          file={entry.url}
          fileName={entry.name}
          editable
          mode="embedded"
          onReady={onReady}
          onChange={onChange}
          style={{ height: "100%" }}
        />
      </div>
    </div>
  );
}
