// SPDX-License-Identifier: MIT
//! docx-rs behind wasm-bindgen for the scoreboard parse race. `parse` (timed) is the documented
//! reader `read_docx`; `extract` (not timed) returns docx-rs's own JSON (`Docx::json`), which the
//! JS adapter walks for paragraph text.
use docx_rs::{read_docx, Docx};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Model {
    docx: Docx,
}

#[wasm_bindgen]
pub fn parse(bytes: &[u8]) -> Result<Model, JsError> {
    let docx = read_docx(bytes).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(Model { docx })
}

#[wasm_bindgen]
pub fn extract(model: &Model) -> String {
    model.docx.json()
}
