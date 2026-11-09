// SPDX-License-Identifier: MIT
// A clean save preserves the original ZIP, already retained by the WASM handle.
// Once an edit-capable command runs, serialize the model, including undo/redo.
// Keep byte copying and transfer off the initial viewer load path.
export function postSave(handle, worker, type, dirty) {
  const original = type === "original-bytes";
  const bytes = new Uint8Array(
    original || !dirty ? handle.takePackageBytes() : handle.saveDocument(),
  );
  worker.postMessage({ type: original ? type : "saved", bytes }, [bytes.buffer]);
}
