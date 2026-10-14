// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/editor/render/slide-operations.ts,
// src/core/pptx/core/builders/sdk/Presentation-slides.ts and
// viewers/pptx/packages/svelte/src/viewer/editor/editor-slide-ops.ts;
// changes listed in PROVENANCE.md.

//! Slide command runners: add, delete, duplicate, reorder, notes text (#86).
//!
//! TODO: package-level rels, shared media and `[Content_Types].xml` are updated
//! by the TS `PptxPresentationSlidesReconciler` on save. This host mutates the
//! slide model (order, notes text, thumbnail rail) only; a pptx package writer
//! has to replay that reconciler before slide ops round-trip bytes.

use std::any::Any;

use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry, EditError};
use serde_json::Value;

use super::host::PptxHost;

fn host_mut(host: &mut dyn Any) -> Result<&mut PptxHost, EditError> {
    host.downcast_mut::<PptxHost>()
        .ok_or_else(|| EditError::Other {
            message: "host is not PptxHost".into(),
        })
}

fn map_err(err: String) -> EditError {
    EditError::Other { message: err }
}

fn arg_u32_opt(args: &Value, key: &str) -> Option<u32> {
    args.get(key).and_then(|v| v.as_u64()).map(|n| n as u32)
}

fn arg_u32(args: &Value, key: &str) -> Result<u32, EditError> {
    arg_u32_opt(args, key).ok_or_else(|| EditError::Other {
        message: format!("missing or invalid `{key}`"),
    })
}

fn change(kind: ChangeKind, label: &str) -> ChangeSet {
    ChangeSet::new(kind, label)
}

/// Register slide-list and notes ops from `edits/schema.json`.
pub fn register_slide_commands(registry: &mut CommandRegistry) {
    registry.register(
        Command::stub("addSlide", "addSlide").with_runner(|host, args| {
            let h = host_mut(host)?;
            h.add_slide(arg_u32_opt(args, "at")).map_err(map_err)?;
            Ok(change(ChangeKind::Structure, "addSlide"))
        }),
    );
    registry.register(
        Command::stub("deleteSlide", "deleteSlide").with_runner(|host, args| {
            let h = host_mut(host)?;
            let index = arg_u32(args, "index")?;
            h.delete_slide(index).map_err(map_err)?;
            Ok(change(ChangeKind::Structure, "deleteSlide"))
        }),
    );
    registry.register(
        Command::stub("duplicateSlide", "duplicateSlide").with_runner(|host, args| {
            let h = host_mut(host)?;
            let index = arg_u32(args, "index")?;
            h.duplicate_slide(index).map_err(map_err)?;
            Ok(change(ChangeKind::Structure, "duplicateSlide"))
        }),
    );
    registry.register(
        Command::stub("reorderSlide", "reorderSlide").with_runner(|host, args| {
            let h = host_mut(host)?;
            let from = arg_u32(args, "from")?;
            let to = arg_u32(args, "to")?;
            h.reorder_slide(from, to).map_err(map_err)?;
            Ok(change(ChangeKind::Structure, "reorderSlide"))
        }),
    );
    registry.register(
        Command::stub("setNotesText", "setNotesText").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            let text =
                args.get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EditError::Other {
                        message: "missing `text`".into(),
                    })?;
            h.set_notes_text(slide, text).map_err(map_err)?;
            Ok(change(ChangeKind::Other, "setNotesText"))
        }),
    );
}
