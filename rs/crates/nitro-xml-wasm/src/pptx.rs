// SPDX-License-Identifier: MIT
//! Pptx viewer state and engine API (#27).

use std::collections::BTreeSet;

use crate::events::{Event, MOD_CTRL, MOD_META, MOD_SHIFT};
use crate::shell::{push_text_hits, FindMatch};
use crate::transport::StringTable;
use nitro_xml_pptx::render::{
    contains_id, copy_text, fit_slide_scale, hit_top_level, top_level_ids, SpAutofitPaintState,
    StringIntern,
};
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
    pub sp_autofit: SpAutofitPaintState,
    pub active: usize,
    pub viewport_w: i32,
    pub viewport_h: i32,
    pub scale: f64,
    /// Top-level element ids selected on the active slide, in click / select-all order.
    pub selected_ids: Vec<String>,
}

impl PptxState {
    pub fn open(handle_bytes: &[u8]) -> Result<Self, String> {
        let leaked: &'static [u8] = Box::leak(handle_bytes.to_vec().into_boxed_slice());
        let pres = Presentation::open(leaked).map_err(|e| e.to_string())?;
        Ok(Self {
            pres,
            sp_autofit: SpAutofitPaintState::default(),
            active: 0,
            viewport_w: DEFAULT_W,
            viewport_h: DEFAULT_H,
            scale: 0.0,
            selected_ids: Vec::new(),
        })
    }

    pub fn set_active_slide(&mut self, index: usize) -> Result<(), String> {
        if index >= self.pres.slide_count() {
            return Err(format!("slide index {} out of range", index));
        }
        if self.active != index {
            self.selected_ids.clear();
        }
        self.active = index;
        Ok(())
    }

    /// Select one top-level element id on the active slide.
    pub fn select_element_id(&mut self, id: &str) -> bool {
        let active = self.active;
        let ok = self
            .pres
            .slide(active)
            .map(|slide| contains_id(slide, id))
            .unwrap_or(false);
        if ok {
            self.selected_ids = vec![id.to_owned()];
        }
        ok
    }

    /// Joined text of the selected shapes, paint order, `\n` between elements.
    pub fn selected_text(&mut self) -> String {
        let ids = self.selected_ids.clone();
        let active = self.active;
        self.pres
            .slide(active)
            .map(|slide| copy_text(slide, &ids))
            .unwrap_or_default()
    }

    fn select_all(&mut self) {
        let active = self.active;
        self.selected_ids = self
            .pres
            .slide(active)
            .map(top_level_ids)
            .unwrap_or_default();
    }

    fn label(&self) -> String {
        if self.selected_ids.is_empty() {
            "A1".into()
        } else {
            self.selected_ids.join(",")
        }
    }

    fn effective_scale(&self) -> f64 {
        if self.scale > 0.0 {
            return self.scale;
        }
        let (w, h) = self.pres.canvas_size();
        fit_slide_scale(w, h, self.viewport_w, self.viewport_h)
    }

    fn hit_id(&mut self, x: f32, y: f32) -> Option<String> {
        let scale = self.effective_scale();
        if scale <= f64::EPSILON {
            return None;
        }
        let sx = f64::from(x) / scale;
        let sy = f64::from(y) / scale;
        let active = self.active;
        let slide = self.pres.slide(active).ok()?;
        hit_top_level(slide, sx, sy)
    }

    pub fn viewport(&self) -> SlidePaintViewport {
        SlidePaintViewport {
            slide_index: self.active,
            scale: self.scale,
            viewport_w: self.viewport_w,
            viewport_h: self.viewport_h,
            selected_ids: self.selected_ids.clone(),
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
            &mut self.sp_autofit,
            &viewport,
        );
    }

    /// Returns the selection label when the painted selection changed (`A1` when cleared).
    pub fn process_events(&mut self, events: &[Event], strings: &StringTable) -> Option<String> {
        let mut changed = false;
        for ev in events {
            if self.apply_event(ev, strings) {
                changed = true;
            }
        }
        if changed {
            Some(self.label())
        } else {
            None
        }
    }

    fn apply_event(&mut self, ev: &Event, strings: &StringTable) -> bool {
        match ev {
            Event::Resize { width, height, .. } => {
                self.viewport_w = (*width).max(1.0) as i32;
                self.viewport_h = (*height).max(1.0) as i32;
                false
            }
            Event::Pointer {
                type_,
                button,
                x,
                y,
                ..
            } if *type_ == 0 && *button == 0 => {
                match self.hit_id(*x, *y) {
                    Some(id) => self.selected_ids = vec![id],
                    None => self.selected_ids.clear(),
                }
                true
            }
            Event::Keyboard {
                type_,
                key_id,
                mods,
                ..
            } => {
                if *type_ != 0 {
                    return false;
                }
                let key = strings.resolve(*key_id).unwrap_or("");
                let ctrl = mods & (MOD_CTRL | MOD_META) != 0;
                let shift = mods & MOD_SHIFT != 0;
                if ctrl && !shift && (key == "a" || key == "A") {
                    self.select_all();
                    return true;
                }
                let before = self.active;
                if key == "ArrowRight" || key == "PageDown" {
                    let next = self.active + 1;
                    if next < self.pres.slide_count() {
                        self.active = next;
                    }
                } else if (key == "ArrowLeft" || key == "PageUp") && self.active > 0 {
                    self.active -= 1;
                }
                if self.active != before {
                    self.selected_ids.clear();
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Shell zoom multiplies the fit-to-viewport scale (100% is fit).
    pub fn apply_shell_zoom(&mut self, zoom_pct: f64) {
        let (w, h) = self.pres.canvas_size();
        let fit = fit_slide_scale(w, h, self.viewport_w, self.viewport_h);
        let factor = (zoom_pct / 100.0).clamp(0.1, 4.0);
        self.scale = fit * factor;
    }

    /// First top-level shape on the active slide whose visible text is non-empty.
    pub fn first_text_element_id(&mut self) -> Option<String> {
        let active = self.active;
        let slide = self.pres.slide(active).ok()?;
        let ids = top_level_ids(slide);
        ids.into_iter().find(|id| {
            let text = copy_text(slide, std::slice::from_ref(id));
            !text.is_empty()
        })
    }

    /// Visible shape text on every slide (`copy_text` per element).
    /// Table cell text and speaker notes are not on that path — TODO(#27).
    pub fn search(
        &mut self,
        query: &str,
        case_sensitive: bool,
        match_entire: bool,
    ) -> Result<Vec<FindMatch>, String> {
        self.pres.ensure_all_slides().map_err(|e| e.to_string())?;
        let mut matches = Vec::new();
        let n = self.pres.slide_count();
        for i in 0..n {
            let slide = self.pres.slide(i).map_err(|e| e.to_string())?;
            let ids = top_level_ids(slide);
            for (ei, id) in ids.iter().enumerate() {
                let text = copy_text(slide, std::slice::from_ref(id));
                push_text_hits(
                    &mut matches,
                    i as u32,
                    ei as u32,
                    &text,
                    query,
                    case_sensitive,
                    match_entire,
                );
            }
        }
        Ok(matches)
    }

    /// Kinds the slide parser did not turn into elements (unsupported content).
    pub fn unsupported_labels(&mut self) -> Result<Vec<String>, String> {
        self.pres.ensure_all_slides().map_err(|e| e.to_string())?;
        let mut set = BTreeSet::new();
        let n = self.pres.slide_count();
        for i in 0..n {
            let slide = self.pres.slide(i).map_err(|e| e.to_string())?;
            for kind in &slide.other_kinds {
                set.insert(kind.clone());
            }
        }
        Ok(set.into_iter().collect())
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
    let selection = if let Some(ref mut pptx) = handle.pptx {
        pptx.process_events(&events, &handle.strings)
    } else {
        None
    };
    if let Some(r) = selection {
        if handle.shell.selection_ref != r {
            handle.shell.selection_ref = r;
            handle.shell.dirty_ui = true;
        }
    }
    handle.events.drain();
    if let Some(ref mut pptx) = handle.pptx {
        pptx.apply_shell_zoom(handle.shell.zoom_pct);
        pptx.paint(
            &mut handle.strings,
            &mut handle.commands,
            &mut handle.measures,
            &mut handle.layout_reads,
        );
    }
}
