// SPDX-License-Identifier: MIT
import { useEffect, useRef, forwardRef, useImperativeHandle } from "react";
import { tagFor } from "./bench-api.js";

/**
 * Minimal React host for embedded Nitro viewers.
 * Creates <nitro-xlsx|docx|pptx mode="embedded" readonly="false">.
 */
export const NitroHost = forwardRef(function NitroHost(
  { format, onReady, onDisposed },
  ref,
) {
  const wrapRef = useRef(null);
  const elRef = useRef(null);

  useImperativeHandle(ref, () => ({
    get element() {
      return elRef.current;
    },
  }));

  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap) return undefined;
    const tag = tagFor(format);
    const el = document.createElement(tag);
    el.setAttribute("mode", "embedded");
    el.setAttribute("readonly", "false");
    el.style.display = "block";
    el.style.height = "100%";
    el.style.minHeight = "480px";
    wrap.appendChild(el);
    elRef.current = el;
    onReady?.(el);
    return () => {
      try {
        el.remove();
      } catch {
        /* ignore */
      }
      elRef.current = null;
      onDisposed?.();
    };
  }, [format, onReady, onDisposed]);

  return <div id="host" ref={wrapRef} />;
});
