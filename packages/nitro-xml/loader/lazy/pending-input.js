// SPDX-License-Identifier: MIT
/** Serialize input through its paint readback, and expose the drain barrier to save. */
export function attach(el) {
  const d = el.document;
  if (!d || d._hostInput) return;
  d._hostInput = Promise.resolve();
  const key = d.editorKey.bind(d);
  d.editorKey = (...args) => {
    const pending = d._hostInput.then(async () => {
      if (el.localName === "nitro-pptx") await d.tick();
      const result = await key(...args);
      if (el.localName === "nitro-pptx") await d.tick();
      return result;
    });
    d._hostInput = pending;
    return pending;
  };
}
