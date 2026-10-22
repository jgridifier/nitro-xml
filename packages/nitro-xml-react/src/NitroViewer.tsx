// SPDX-License-Identifier: MIT
import {
  useEffect,
  useRef,
  forwardRef,
  useImperativeHandle,
  type ReactElement,
} from "react";

export type NitroViewerMode = "embedded" | "full";

export type NitroViewerProps = {
  /** File bytes, Blob, File, or URL string. */
  file: ArrayBuffer | Uint8Array | Blob | File | string | null | undefined;
  /** Hint name when `file` is bytes (used for format detection). */
  fileName?: string;
  /** When true, sets `readonly="false"` on the custom element. */
  editable?: boolean;
  mode?: NitroViewerMode;
  /** Fired after open + first paint attempt with the host element. */
  onReady?: (el: HTMLElement) => void;
  /** Fired when `nitro-save` / programmatic save produces bytes. */
  onChange?: (bytes: Uint8Array) => void;
  className?: string;
  style?: React.CSSProperties;
};

export type NitroViewerHandle = {
  /** Underlying `<nitro-xlsx|docx|pptx>` element, or null. */
  element: HTMLElement | null;
  save: () => Promise<Uint8Array>;
  applyEdit: (spec: string | object) => Promise<string>;
};

type LoaderMod = typeof import("nitro-xml/loader");

function extOf(name: string): string {
  const i = name.lastIndexOf(".");
  return i >= 0 ? name.slice(i + 1).toLowerCase() : "";
}

function tagForName(name: string): "nitro-xlsx" | "nitro-docx" | "nitro-pptx" | null {
  const e = extOf(name);
  if (["xlsx", "xlsm", "xltx", "xltm"].includes(e)) return "nitro-xlsx";
  if (["docx", "docm", "dotx", "dotm"].includes(e)) return "nitro-docx";
  if (["pptx", "pptm", "potx", "potm"].includes(e)) return "nitro-pptx";
  return null;
}

async function bytesFromFile(
  file: NonNullable<NitroViewerProps["file"]>,
  fileName?: string,
): Promise<{ bytes: ArrayBuffer; name: string }> {
  if (typeof file === "string") {
    const res = await fetch(file);
    if (!res.ok) throw new Error(`fetch failed: ${res.status}`);
    const name =
      fileName ||
      file.split("?")[0]?.split("#")[0]?.split("/").pop() ||
      "document.xlsx";
    return { bytes: await res.arrayBuffer(), name };
  }
  if (file instanceof Blob) {
    const name =
      fileName || (file instanceof File ? file.name : "document.xlsx");
    return { bytes: await file.arrayBuffer(), name };
  }
  const name = fileName || "document.xlsx";
  if (file instanceof ArrayBuffer) return { bytes: file, name };
  const copy = new Uint8Array(file.byteLength);
  copy.set(file);
  return { bytes: copy.buffer, name };
}

/**
 * SSR-safe React host for Nitro XML custom elements.
 * Module evaluate does not touch `window` / `document`; custom elements and
 * WASM load only inside `useEffect` on the client.
 */
export const NitroViewer = forwardRef<NitroViewerHandle, NitroViewerProps>(
  function NitroViewer(
    {
      file,
      fileName,
      editable = false,
      mode = "embedded",
      onReady,
      onChange,
      className,
      style,
    },
    ref,
  ): ReactElement {
    const wrapRef = useRef<HTMLDivElement | null>(null);
    const elRef = useRef<HTMLElement | null>(null);
    const onReadyRef = useRef(onReady);
    const onChangeRef = useRef(onChange);
    onReadyRef.current = onReady;
    onChangeRef.current = onChange;

    useImperativeHandle(ref, () => ({
      get element() {
        return elRef.current;
      },
      async save() {
        const el = elRef.current as HTMLElement & {
          save?: () => Promise<Uint8Array>;
        };
        if (!el?.save) throw new Error("NitroViewer: not ready");
        return el.save();
      },
      async applyEdit(spec) {
        const el = elRef.current as HTMLElement & {
          document?: { applyEdit?: (s: string) => Promise<string> };
        };
        const d = el?.document;
        if (!d?.applyEdit) throw new Error("NitroViewer: not ready");
        const s = typeof spec === "string" ? spec : JSON.stringify(spec);
        return d.applyEdit(s);
      },
    }));

    useEffect(() => {
      if (file == null) return undefined;
      let cancelled = false;
      let el: HTMLElement | null = null;
      let onSave: ((ev: Event) => void) | null = null;

      (async () => {
        const wrap = wrapRef.current;
        if (!wrap || typeof document === "undefined") return;

        const loader: LoaderMod = await import("nitro-xml/loader");
        if (cancelled) return;

        loader.defineNitroXlsx();
        loader.defineNitroDocx();
        loader.defineNitroPptx();

        const { bytes, name } = await bytesFromFile(file, fileName);
        if (cancelled) return;

        const tag = tagForName(name);
        if (!tag) throw new Error(`unsupported format: ${name}`);

        el = document.createElement(tag);
        el.setAttribute("mode", mode === "full" ? "full" : "embedded");
        el.setAttribute("readonly", editable ? "false" : "true");
        el.style.display = "block";
        el.style.height = "100%";
        el.style.minHeight = "320px";
        wrap.replaceChildren(el);
        elRef.current = el;

        onSave = (ev: Event) => {
          const detail = (ev as CustomEvent<Uint8Array>).detail;
          if (detail) onChangeRef.current?.(detail);
        };
        el.addEventListener("nitro-save", onSave);

        const viewer = el as HTMLElement & {
          loadBuffer: (b: ArrayBuffer, n: string) => Promise<void>;
          document?: { tick?: () => Promise<unknown> };
        };
        await viewer.loadBuffer(bytes, name);
        if (cancelled) return;
        try {
          await viewer.document?.tick?.();
        } catch {
          /* paint best-effort */
        }
        if (!cancelled) onReadyRef.current?.(el);
      })().catch((err) => {
        if (!cancelled) console.error("NitroViewer:", err);
      });

      return () => {
        cancelled = true;
        if (el && onSave) el.removeEventListener("nitro-save", onSave);
        try {
          el?.remove();
        } catch {
          /* ignore */
        }
        elRef.current = null;
        if (wrapRef.current) wrapRef.current.replaceChildren();
      };
    }, [file, fileName, editable, mode]);

    return <div className={className} style={style} ref={wrapRef} />;
  },
);

NitroViewer.displayName = "NitroViewer";
