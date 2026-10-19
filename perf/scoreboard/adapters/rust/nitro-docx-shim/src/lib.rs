// SPDX-License-Identifier: MIT
//! nitro-xml-docx behind wasm-bindgen for the scoreboard parse race. `parse` (timed) is the public
//! `read_package`; `extract` (not timed) returns model blocks JSON for the JS adapter.
use nitro_xml_docx::read_package;
use nitro_xml_docx::DocumentModel;
use serde_json::json;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Model {
    model: DocumentModel,
}

#[wasm_bindgen]
pub fn parse(bytes: &[u8]) -> Result<Model, JsError> {
    let loaded = read_package(bytes).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(Model {
        model: loaded.model,
    })
}

#[wasm_bindgen]
pub fn extract(model: &Model) -> String {
    json!({ "blocks": model.model.blocks }).to_string()
}
