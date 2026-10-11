// SPDX-License-Identifier: MIT
//! Pptx viewer state and engine API (#27).

use crate::events::Event;
use crate::transport::StringTable;
use nitro_xml_pptx::render::StringIntern;
use nitro_xml_pptx::{paint_slide_frame, Presentation, SlidePaintViewport};
use serde_json::{json, Value};

const DEFAULT_W: i32 = 800;
const DEFAULT_H: i32 = 600;

impl StringIntern for StringTable {
    fn intern(&mut self, s: &str) -> u32 {
        StringTable::intern(self, s)
    }
}

pub struct PptxState {
    pub pres: Presentation<'static>,
    pub active: usize,
    pub viewport_w: i32,
    pub viewport_h: i32,
    pub scale: f64,
}

impl PptxState {
    pub fn open(handle_bytes: &[u8]) -> Result<Self, String> {
        let leaked: &'static [u8] = Box::leak(handle_bytes.to_vec().into_boxed_slice());
        let pres = Presentation::open(leaked).map_err(|e| e.to_string())?;
        Ok(Self {
            pres,
            active: 0,
            viewport_w: DEFAULT_W,
            viewport_h: DEFAULT_H,
            scale: 0.0,
        })
    }

    pub fn set_active_slide(&mut self, index: usize) -> Result<(), String> {
        if index >= self.pres.slide_count() {
            return Err(format!("slide index {} out of range", index));
        }
        self.active = index;
        Ok(())
    }

    pub fn viewport(&self) -> SlidePaintViewport {
        SlidePaintViewport {
            slide_index: self.active,
            scale: self.scale,
            viewport_w: self.viewport_w,
            viewport_h: self.viewport_h,
        }
    }

    pub fn paint(
        &mut self,
        strings: &mut StringTable,
        commands: &mut nitro_xml_render::CommandBuffer,
        measures: &mut nitro_xml_render::measure::MeasureBatch,
        layout_reads: &mut nitro_xml_render::LayoutReadBatch,
    ) {
        let viewport = self.viewport();
        let _ = paint_slide_frame(
            commands,
            strings,
            measures,
            layout_reads,
            &mut self.pres,
            &viewport,
        );
    }

    pub fn process_events(&mut self, events: &[Event], strings: &StringTable) {
        for ev in events {
            match ev {
                Event::Resize { width, height, .. } => {
                    self.viewport_w = (*width).max(1.0) as i32;
                    self.viewport_h = (*height).max(1.0) as i32;
                }
                Event::Keyboard { type_, key_id, .. } => {
                    if *type_ != 0 {
                        continue;
                    }
                    let key = strings.resolve(*key_id).unwrap_or("");
                    if key == "ArrowRight" || key == "PageDown" {
                        let next = self.active + 1;
                        if next < self.pres.slide_count() {
                            self.active = next;
                        }
                    } else if (key == "ArrowLeft" || key == "PageUp") && self.active > 0 {
                        self.active -= 1;
                    }
                }
                _ => {}
            }
        }
    }

    pub fn slides_json(&self) -> Value {
        let slides: Vec<Value> = self
            .pres
            .slide_paths()
            .iter()
            .enumerate()
            .map(|(i, path)| {
                json!({
                    "index": i,
                    "path": path,
                    "active": i == self.active,
                })
            })
            .collect();
        let (w, h) = self.pres.canvas_size();
        json!({
            "slideCount": slides.len(),
            "activeSlide": self.active,
            "width": w,
            "height": h,
            "slides": slides,
        })
    }
}

pub fn is_pptx_extension(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "pptx" | "pptm" | "potx" | "potm")
}

pub(crate) fn process_frame(handle: &mut crate::handle::Handle) {
    let events: Vec<Event> = handle.events.pending_snapshot();
    if let Some(ref mut pptx) = handle.pptx {
        pptx.process_events(&events, &handle.strings);
    }
    handle.events.drain();
    if let Some(ref mut pptx) = handle.pptx {
        pptx.paint(
            &mut handle.strings,
            &mut handle.commands,
            &mut handle.measures,
            &mut handle.layout_reads,
        );
    }
}
