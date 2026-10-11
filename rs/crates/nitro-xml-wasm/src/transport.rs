// SPDX-License-Identifier: MIT
//! Bulk transfer, interned strings, per-frame command buffer, measure batches (#14).
//!
//! Decision (see `docs/boundary.md`): struct-of-arrays typed-array views over wasm
//! memory for bulk numeric data; never per-cell JS↔WASM calls on the render path.
//! Canvas draw calls and DOM-building ops cross as one command buffer per frame.
//! Text measurement follows upstream and is batched per layout pass (loader applies
//! `measureText` in #37).

use crate::handle::OoxmlError;

/// Per-document interned string table. Id `0` is always the empty string.
///
/// Backed by a `Vec` (linear scan on intern) so the shipped wasm does not pull
/// in `hashbrown` / `RandomState`. Document string tables stay small; bulk
/// traffic uses the numeric ids.
#[derive(Debug, Default)]
pub struct StringTable {
    by_id: Vec<String>,
}

impl StringTable {
    pub fn new() -> Self {
        let mut t = Self { by_id: Vec::new() };
        t.intern("");
        t
    }

    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(id) = self.by_id.iter().position(|x| x == s) {
            return id as u32;
        }
        let id = self.by_id.len() as u32;
        self.by_id.push(s.to_owned());
        id
    }

    pub fn resolve(&self, id: u32) -> Option<&str> {
        self.by_id.get(id as usize).map(String::as_str)
    }

    pub fn len(&self) -> u32 {
        self.by_id.len() as u32
    }
}

/// Compact per-frame command buffer opcodes (little-endian payloads).
///
/// This is the **seed set** for Stage 2. Upstream painters also emit SVG NS
/// creates, text nodes, style property writes, `classList`, insert/remove,
/// markup parsing and canvas state — listed as gaps in `docs/boundary.md`
/// (TODO(#36)/TODO(#37)).
///
/// | Op | Tag | Payload |
/// |---|---|---|
/// | FillRect | 1 | x:i32 y:i32 w:i32 h:i32 color:u32 |
/// | StrokeRect | 2 | x:i32 y:i32 w:i32 h:i32 color:u32 line:f32 |
/// | DrawImage | 3 | img:u32 x:i32 y:i32 w:i32 h:i32 |
/// | DrawText | 4 | x:i32 y:i32 font:u32 size:f32 text:u32 color:u32 |
/// | CreateElement | 5 | id:u32 tag:u32 parent:u32 |
/// | SetAttr | 6 | id:u32 key:u32 val:u32 |
/// | AppendChild | 7 | parent:u32 child:u32 |
// Seed protocol for painters / the loader (#37). Not called from the shipped
// path yet; Stage 3+ ports will encode through these tags.
#[allow(dead_code)]
pub mod opcode {
    pub const FILL_RECT: u8 = 1;
    pub const STROKE_RECT: u8 = 2;
    pub const DRAW_IMAGE: u8 = 3;
    pub const DRAW_TEXT: u8 = 4;
    pub const CREATE_ELEMENT: u8 = 5;
    pub const SET_ATTR: u8 = 6;
    pub const APPEND_CHILD: u8 = 7;
}

#[derive(Debug, Default)]
pub struct CommandBuffer {
    buf: Vec<u8>,
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn begin(&mut self) {
        self.buf.clear();
    }

    pub fn take(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.buf)
    }

    fn put_u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    fn put_u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    fn put_i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    fn put_f32(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    #[allow(dead_code)] // painter-facing; Stage 3+
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        self.put_u8(opcode::FILL_RECT);
        self.put_i32(x);
        self.put_i32(y);
        self.put_i32(w);
        self.put_i32(h);
        self.put_u32(color);
    }

    #[allow(dead_code)] // painter-facing; Stage 3+
    pub fn stroke_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32, line: f32) {
        self.put_u8(opcode::STROKE_RECT);
        self.put_i32(x);
        self.put_i32(y);
        self.put_i32(w);
        self.put_i32(h);
        self.put_u32(color);
        self.put_f32(line);
    }

    #[allow(dead_code)] // painter-facing; Stage 3+
    pub fn draw_image(&mut self, img: u32, x: i32, y: i32, w: i32, h: i32) {
        self.put_u8(opcode::DRAW_IMAGE);
        self.put_u32(img);
        self.put_i32(x);
        self.put_i32(y);
        self.put_i32(w);
        self.put_i32(h);
    }

    #[allow(dead_code)] // painter-facing; Stage 3+
    pub fn draw_text(&mut self, x: i32, y: i32, font: u32, size: f32, text: u32, color: u32) {
        self.put_u8(opcode::DRAW_TEXT);
        self.put_i32(x);
        self.put_i32(y);
        self.put_u32(font);
        self.put_f32(size);
        self.put_u32(text);
        self.put_u32(color);
    }

    #[allow(dead_code)] // painter-facing; Stage 3+
    pub fn create_element(&mut self, id: u32, tag: u32, parent: u32) {
        self.put_u8(opcode::CREATE_ELEMENT);
        self.put_u32(id);
        self.put_u32(tag);
        self.put_u32(parent);
    }

    #[allow(dead_code)] // painter-facing; Stage 3+
    pub fn set_attr(&mut self, id: u32, key: u32, val: u32) {
        self.put_u8(opcode::SET_ATTR);
        self.put_u32(id);
        self.put_u32(key);
        self.put_u32(val);
    }

    #[allow(dead_code)] // painter-facing; Stage 3+
    pub fn append_child(&mut self, parent: u32, child: u32) {
        self.put_u8(opcode::APPEND_CHILD);
        self.put_u32(parent);
        self.put_u32(child);
    }

    /// Mixed canvas/DOM ops matching the seed opcode set (tests + `bench` feature).
    #[allow(dead_code)]
    pub fn encode_demo(&mut self, n_ops: u32) {
        self.begin();
        for i in 0..n_ops {
            match i % 7 {
                0 => self.fill_rect((i % 100) as i32, (i % 80) as i32, 10, 12, 0xff00_0000 | i),
                1 => self.stroke_rect((i % 100) as i32, (i % 80) as i32, 10, 12, 0xff00_00ff, 1.0),
                2 => self.draw_image(i % 8, (i % 50) as i32, (i % 40) as i32, 32, 32),
                3 => self.draw_text(
                    (i % 200) as i32,
                    (i % 100) as i32,
                    1,
                    11.0,
                    i % 64,
                    0xff00_0000,
                ),
                4 => self.create_element(i + 1, i % 5, i),
                5 => self.set_attr(i + 1, i % 3, i % 16),
                _ => self.append_child(i, i + 1),
            }
        }
    }
}

/// Batched text-measurement requests (fonts/measurement follow upstream).
///
/// TODO(#37): loader applies `measureText` for each pending request and calls
/// [`MeasureBatch::set_results`].
#[derive(Debug, Default)]
pub struct MeasureBatch {
    pub font_id: Vec<u32>,
    pub size_px: Vec<f32>,
    pub text_id: Vec<u32>,
    pub widths: Vec<f64>,
}

impl MeasureBatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.font_id.len()
    }

    pub fn push(&mut self, font_id: u32, size_px: f32, text_id: u32) {
        self.font_id.push(font_id);
        self.size_px.push(size_px);
        self.text_id.push(text_id);
    }

    pub fn set_results(&mut self, widths: &[f64]) -> Result<(), OoxmlError> {
        if widths.len() != self.font_id.len() {
            return Err(OoxmlError::with_code(
                "measure_len_mismatch",
                format!(
                    "expected {} widths, got {}",
                    self.font_id.len(),
                    widths.len()
                ),
            ));
        }
        self.widths.clear();
        self.widths.extend_from_slice(widths);
        self.font_id.clear();
        self.size_px.clear();
        self.text_id.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_buffer_demo_encodes_expected_byte_length() {
        // Catches: opcode payload sizes drifting so the loader (#37) reads past the buffer.
        let mut buf = CommandBuffer::new();
        buf.encode_demo(7);
        let expected = (1 + 20) + (1 + 24) + (1 + 20) + (1 + 24) + (1 + 12) + (1 + 12) + (1 + 8);
        let bytes = buf.take();
        assert_eq!(bytes.len(), expected);
        assert_eq!(bytes[0], opcode::FILL_RECT);
    }

    #[test]
    fn measure_results_reject_length_mismatch() {
        // Catches: a short measure reply must error, not silently truncate layout widths.
        let mut m = MeasureBatch::new();
        m.push(1, 11.0, 2);
        m.push(1, 11.0, 3);
        assert!(m.set_results(&[1.0]).is_err());
        assert!(m.set_results(&[1.0, 2.0]).is_ok());
        assert_eq!(m.widths, vec![1.0, 2.0]);
        assert_eq!(m.len(), 0);
    }
}
