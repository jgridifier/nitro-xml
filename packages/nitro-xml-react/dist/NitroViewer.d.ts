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
/**
 * SSR-safe React host for Nitro XML custom elements.
 * Module evaluate does not touch `window` / `document`; custom elements and
 * WASM load only inside `useEffect` on the client.
 */
export declare const NitroViewer: import("react").ForwardRefExoticComponent<NitroViewerProps & import("react").RefAttributes<NitroViewerHandle>>;
//# sourceMappingURL=NitroViewer.d.ts.map