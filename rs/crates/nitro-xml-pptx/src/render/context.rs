// SPDX-License-Identifier: MIT
//! Per-frame paint context: DOM node ids, view-IR collection, intern hook.

use std::collections::BTreeMap;

use nitro_xml_render::measure::{LayoutReadBatch, LayoutReadKind, MeasureBatch, OutlineBatch};
use nitro_xml_render::CommandBuffer;

use super::field_substitution::FieldSubstitutionContext;
use super::shape_autofit_resize::{LayoutReadRecord, SpAutofitSlideState};
use super::view_ir::ViewIrCollector;
use crate::text_layout::TextAutoFitMode;

pub trait StringIntern {
    fn intern(&mut self, s: &str) -> u32;
}

pub struct PaintContext<'a, I> {
    pub buffer: &'a mut CommandBuffer,
    pub intern: &'a mut I,
    pub measure: &'a mut MeasureBatch,
    pub outlines: &'a mut OutlineBatch,
    pub layout_reads: &'a mut LayoutReadBatch,
    pub view_ir: &'a mut ViewIrCollector,
    pub scale: f64,
    pub canvas_w: f64,
    pub canvas_h: f64,
    /// Theme scheme (`accent1` → `#156082`) for SmartArt `a:schemeClr`.
    pub scheme: &'a BTreeMap<String, String>,
    pub slide_index: usize,
    pub field_ctx: FieldSubstitutionContext,
    pub sp_autofit: &'a mut SpAutofitSlideState,
    pub(crate) layout_read_records: Vec<LayoutReadRecord>,
    next_id: u32,
}

impl<'a, I> PaintContext<'a, I> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        buffer: &'a mut CommandBuffer,
        intern: &'a mut I,
        measure: &'a mut MeasureBatch,
        outlines: &'a mut OutlineBatch,
        layout_reads: &'a mut LayoutReadBatch,
        view_ir: &'a mut ViewIrCollector,
        scale: f64,
        canvas_w: f64,
        canvas_h: f64,
        scheme: &'a BTreeMap<String, String>,
        slide_index: usize,
        field_ctx: FieldSubstitutionContext,
        sp_autofit: &'a mut SpAutofitSlideState,
    ) -> Self {
        Self {
            buffer,
            intern,
            measure,
            outlines,
            layout_reads,
            view_ir,
            scale,
            canvas_w,
            canvas_h,
            scheme,
            slide_index,
            field_ctx,
            sp_autofit,
            layout_read_records: Vec::new(),
            next_id: 2,
        }
    }

    pub fn push_layout_read(
        &mut self,
        node_id: u32,
        kind: LayoutReadKind,
        prop_id: u32,
        autofit: Option<(String, f64, TextAutoFitMode)>,
    ) {
        self.layout_reads.push(node_id, kind, prop_id);
        self.layout_read_records.push(LayoutReadRecord { autofit });
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
