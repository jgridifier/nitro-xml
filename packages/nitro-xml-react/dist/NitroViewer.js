import { jsx as _jsx } from "react/jsx-runtime";
// SPDX-License-Identifier: MIT
import { useEffect, useRef, forwardRef, useImperativeHandle, } from "react";
function extOf(name) {
    const i = name.lastIndexOf(".");
    return i >= 0 ? name.slice(i + 1).toLowerCase() : "";
}
function tagForName(name) {
    const e = extOf(name);
    if (["xlsx", "xlsm", "xltx", "xltm"].includes(e))
        return "nitro-xlsx";
    if (["docx", "docm", "dotx", "dotm"].includes(e))
        return "nitro-docx";
    if (["pptx", "pptm", "potx", "potm"].includes(e))
        return "nitro-pptx";
    return null;
}
async function bytesFromFile(file, fileName) {
    if (typeof file === "string") {
        const res = await fetch(file);
        if (!res.ok)
            throw new Error(`fetch failed: ${res.status}`);
        const name = fileName ||
            file.split("?")[0]?.split("#")[0]?.split("/").pop() ||
            "document.xlsx";
        return { bytes: await res.arrayBuffer(), name };
    }
    if (file instanceof Blob) {
        const name = fileName || (file instanceof File ? file.name : "document.xlsx");
        return { bytes: await file.arrayBuffer(), name };
    }
    const name = fileName || "document.xlsx";
    if (file instanceof ArrayBuffer)
        return { bytes: file, name };
    const copy = new Uint8Array(file.byteLength);
    copy.set(file);
    return { bytes: copy.buffer, name };
}
/**
 * SSR-safe React host for Nitro XML custom elements.
 * Module evaluate does not touch `window` / `document`; custom elements and
 * WASM load only inside `useEffect` on the client.
 */
export const NitroViewer = forwardRef(function NitroViewer({ file, fileName, editable = false, mode = "embedded", onReady, onChange, className, style, }, ref) {
    const wrapRef = useRef(null);
    const elRef = useRef(null);
    const onReadyRef = useRef(onReady);
    const onChangeRef = useRef(onChange);
    onReadyRef.current = onReady;
    onChangeRef.current = onChange;
    useImperativeHandle(ref, () => ({
        get element() {
            return elRef.current;
        },
        async save() {
            const el = elRef.current;
            if (!el?.save)
                throw new Error("NitroViewer: not ready");
            return el.save();
        },
        async applyEdit(spec) {
            const el = elRef.current;
            const d = el?.document;
            if (!d?.applyEdit)
                throw new Error("NitroViewer: not ready");
            const s = typeof spec === "string" ? spec : JSON.stringify(spec);
            return d.applyEdit(s);
        },
    }));
    useEffect(() => {
        if (file == null)
            return undefined;
        let cancelled = false;
        let el = null;
        let onSave = null;
        (async () => {
            const wrap = wrapRef.current;
            if (!wrap || typeof document === "undefined")
                return;
            const loader = await import("nitro-xml/loader");
            if (cancelled)
                return;
            loader.defineNitroXlsx();
            loader.defineNitroDocx();
            loader.defineNitroPptx();
            const { bytes, name } = await bytesFromFile(file, fileName);
            if (cancelled)
                return;
            const tag = tagForName(name);
            if (!tag)
                throw new Error(`unsupported format: ${name}`);
            el = document.createElement(tag);
            el.setAttribute("mode", mode === "full" ? "full" : "embedded");
            el.setAttribute("readonly", editable ? "false" : "true");
            el.style.display = "block";
            el.style.height = "100%";
            el.style.minHeight = "320px";
            wrap.replaceChildren(el);
            elRef.current = el;
            onSave = (ev) => {
                const detail = ev.detail;
                if (detail)
                    onChangeRef.current?.(detail);
            };
            el.addEventListener("nitro-save", onSave);
            const viewer = el;
            await viewer.loadBuffer(bytes, name);
            if (cancelled)
                return;
            try {
                await viewer.document?.tick?.();
            }
            catch {
                /* paint best-effort */
            }
            if (!cancelled)
                onReadyRef.current?.(el);
        })().catch((err) => {
            if (!cancelled)
                console.error("NitroViewer:", err);
        });
        return () => {
            cancelled = true;
            if (el && onSave)
                el.removeEventListener("nitro-save", onSave);
            try {
                el?.remove();
            }
            catch {
                /* ignore */
            }
            elRef.current = null;
            if (wrapRef.current)
                wrapRef.current.replaceChildren();
        };
    }, [file, fileName, editable, mode]);
    return _jsx("div", { className: className, style: style, ref: wrapRef });
});
NitroViewer.displayName = "NitroViewer";
