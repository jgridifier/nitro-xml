// SPDX-License-Identifier: MIT
//! WASM entry for the shared edit core (#63).
//!
//! Replays `edits/*.json` through [`nitro_xml::opc::edit`]. Format handlers land in #66+;
//! until then every known op is structurally recognized and returns `unsupported` (not ok).

use nitro_xml::opc::edit::{replay_script_json, CommandRegistry, EditError, SCHEMA_OP_IDS};
use wasm_bindgen::prelude::*;

use crate::handle::OoxmlError;

fn map_err(err: EditError) -> OoxmlError {
    OoxmlError::new(err.kind().to_owned(), err.to_string(), None)
}

/// Parse and structurally replay an edit-parity script (`edits/*.json`).
///
/// Returns a JSON [`ReplayReport`](nitro_xml::opc::edit::ReplayReport). With the stub
/// registry, `status` is `unsupported` for every corpus script — never a silent pass.
#[wasm_bindgen(js_name = replayEditScript)]
pub fn replay_edit_script(script_json: &str) -> Result<String, OoxmlError> {
    let report = replay_script_json(script_json).map_err(map_err)?;
    serde_json::to_string(&report)
        .map_err(|e| OoxmlError::new("edit_error".into(), e.to_string(), None))
}

/// Schema op ids the registry understands (`edits/schema.json`).
#[wasm_bindgen(js_name = editScriptOpIds)]
pub fn edit_script_op_ids() -> String {
    serde_json::to_string(&SCHEMA_OP_IDS).unwrap_or_else(|_| "[]".into())
}

/// Whether a command id is registered (stub or real). Does not mean it is implemented.
#[wasm_bindgen(js_name = editCommandRegistered)]
pub fn edit_command_registered(id: &str) -> bool {
    CommandRegistry::with_schema_stubs().get(id).is_some()
}
