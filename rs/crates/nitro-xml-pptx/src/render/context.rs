// SPDX-License-Identifier: MIT
//! Per-frame paint context: DOM node ids, view-IR collection, intern hook.

use std::collections::BTreeMap;

use nitro_xml_render::measure::MeasureBatch;
use nitro_xml_render::{CommandBuffer, LayoutReadBatch};

use super::view_ir::ViewIrCollector;

pub trait StringIntern {
    fn intern(&mut self, s: &str) -> u32;
}

pub struct PaintContext<'a, I> {
    pub buffer: &'a mut CommandBuffer,
    pub intern: &'a mut I,
    pub measure: &'a mut MeasureBatch,
    pub layout_reads: &'a mut LayoutReadBatch,
    pub view_ir: &'a mut ViewIrCollector,
    pub scale: f64,
    pub canvas_w: f64,
    pub canvas_h: f64,
    /// Theme scheme (`accent1` → `#156082`) for SmartArt `a:schemeClr`.
    pub scheme: &'a BTreeMap<String, String>,
    next_id: u32,
}

impl<'a, I> PaintContext<'a, I> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        buffer: &'a mut CommandBuffer,
        intern: &'a mut I,
        measure: &'a mut MeasureBatch,
        layout_reads: &'a mut LayoutReadBatch,
        view_ir: &'a mut ViewIrCollector,
        scale: f64,
        canvas_w: f64,
        canvas_h: f64,
        scheme: &'a BTreeMap<String, String>,
    ) -> Self {
        Self {
            buffer,
            intern,
            measure,
            layout_reads,
            view_ir,
            scale,
            canvas_w,
            canvas_h,
            scheme,
            next_id: 2,
        }
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
