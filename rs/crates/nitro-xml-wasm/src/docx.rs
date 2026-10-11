// SPDX-License-Identifier: MIT
//! Docx Print Layout viewer state (#23).

use crate::events::Event;
use crate::transport::StringTable;
use nitro_xml_docx::{PrintNow, PrintSession, PrintStringIntern};

impl PrintStringIntern for StringTable {
    fn intern(&mut self, s: &str) -> u32 {
        StringTable::intern(self, s)
    }
}

pub struct DocxState {
    pub session: PrintSession,
    pub active: usize,
    pub zoom: f64,
}

impl DocxState {
    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        let leaked: &'static [u8] = Box::leak(bytes.to_vec().into_boxed_slice());
        let session = PrintSession::open(leaked)?;
        Ok(Self {
            session,
            active: 0,
            zoom: 1.0,
        })
    }

    pub fn page_count(&self) -> usize {
        self.session.result.pages.len()
    }

    pub fn set_active_page(&mut self, index: usize) -> Result<(), String> {
        let n = self.page_count();
        if n == 0 {
            self.active = 0;
            return Ok(());
        }
        if index >= n {
            return Err(format!("page index {index} out of range"));
        }
        self.active = index;
        Ok(())
    }

    pub fn set_zoom(&mut self, zoom_pct: f64) {
        self.zoom = (zoom_pct / 100.0).clamp(0.1, 4.0);
    }

    pub fn paint(
        &mut self,
        strings: &mut StringTable,
        commands: &mut nitro_xml_render::CommandBuffer,
    ) {
        self.session
            .paint(commands, strings, self.zoom, Some(self.active), print_now());
    }

    pub fn process_events(&mut self, events: &[Event], strings: &StringTable) {
        let n = self.page_count();
        for ev in events {
            if let Event::Keyboard { type_, key_id, .. } = ev {
                if *type_ != 0 || n == 0 {
                    continue;
                }
                let key = strings.resolve(*key_id).unwrap_or("");
                if key == "ArrowRight" || key == "PageDown" {
                    let next = self.active + 1;
                    if next < n {
                        self.active = next;
                    }
                } else if (key == "ArrowLeft" || key == "PageUp") && self.active > 0 {
                    self.active -= 1;
                }
            }
        }
    }

    pub fn page_tabs(&self) -> Vec<(u32, String)> {
        let n = self.page_count().max(1);
        (0..n)
            .map(|i| (i as u32, format!("Page {}", i + 1)))
            .collect()
    }
}

pub fn is_docx_extension(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "docx" | "docm" | "dotx" | "dotm")
}

fn print_now() -> PrintNow {
    #[cfg(target_arch = "wasm32")]
    {
        let d = js_sys::Date::new_0();
        return PrintNow {
            year: d.get_full_year() as i32,
            month: d.get_month() as u32 + 1,
            day: d.get_date() as u32,
            hour: d.get_hours() as u32,
            minute: d.get_minutes() as u32,
        };
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        PrintNow::default()
    }
}

pub(crate) fn process_frame(handle: &mut crate::handle::Handle) {
    let events: Vec<Event> = handle.events.pending_snapshot();
    if let Some(ref mut docx) = handle.docx {
        docx.set_zoom(handle.shell.zoom_pct);
        docx.process_events(&events, &handle.strings);
    }
    handle.events.drain();
    if let Some(ref mut docx) = handle.docx {
        docx.paint(&mut handle.strings, &mut handle.commands);
    }
}
