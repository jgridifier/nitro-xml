// SPDX-License-Identifier: MIT
//! nitro-xml-pptx behind wasm-bindgen for the scoreboard parse race. `parse` (timed) is
//! `Presentation::open` then `ensure_all_slides`; `extract` (not timed) returns inspect JSON for
//! the JS adapter to walk.
use nitro_xml_pptx::Presentation;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Model {
    bytes: Vec<u8>,
}

#[wasm_bindgen]
pub fn parse(bytes: &[u8]) -> Result<Model, JsError> {
    let owned = bytes.to_vec();
    let mut pres = Presentation::open(&owned).map_err(|e| JsError::new(&e.to_string()))?;
    pres.ensure_all_slides()
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(Model { bytes: owned })
}

#[wasm_bindgen]
pub fn extract(model: &Model) -> String {
    let mut pres = Presentation::open(&model.bytes).expect("open");
    pres.inspect_json().expect("inspect")
}
