// SPDX-License-Identifier: MIT
//! Command-buffer writer: encodes seed opcodes into a little-endian byte stream.
//!
//! Wire format must stay byte-identical to `packages/nitro-xml/loader/apply.js`.

use super::opcode;

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

    /// Grow capacity without changing length (paint frames know approximate size).
    pub fn reserve(&mut self, additional: usize) {
        self.buf.reserve(additional);
    }

    pub fn take(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.buf)
    }

    #[inline]
    fn put_bytes<const N: usize>(&mut self, bytes: [u8; N]) {
        let len = self.buf.len();
        self.buf.reserve(N);
        // SAFETY: reserve(N) guarantees capacity for len+N; copy then set_len.
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.buf.as_mut_ptr().add(len), N);
            self.buf.set_len(len + N);
        }
    }

    #[inline]
    fn put_u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    #[inline]
    fn put_u32(&mut self, v: u32) {
        self.put_bytes(v.to_le_bytes());
    }

    #[inline]
    fn put_i32(&mut self, v: i32) {
        self.put_bytes(v.to_le_bytes());
    }

    #[inline]
    fn put_f32(&mut self, v: f32) {
        self.put_bytes(v.to_le_bytes());
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

    #[allow(dead_code)]
    pub fn set_text(&mut self, id: u32, text_id: u32) {
        self.put_u8(opcode::SET_TEXT);
        self.put_u32(id);
        self.put_u32(text_id);
    }

    #[allow(dead_code)]
    pub fn set_style(&mut self, id: u32, prop_id: u32, val_id: u32) {
        self.put_u8(opcode::SET_STYLE);
        self.put_u32(id);
        self.put_u32(prop_id);
        self.put_u32(val_id);
    }

    /// Attach SVG (or other) markup under a parent node id (string-table markup id).
    pub fn set_inner_markup(&mut self, parent: u32, markup: u32) {
        self.put_u8(opcode::SET_INNER_MARKUP);
        self.put_u32(parent);
        self.put_u32(markup);
    }

    /// Place a DOM node via CSSOM `left/top/width/height` (no style-string intern; #30).
    pub fn place_box(&mut self, id: u32, x: i32, y: i32, w: i32, h: i32) {
        self.put_u8(opcode::PLACE_BOX);
        self.put_u32(id);
        self.put_i32(x);
        self.put_i32(y);
        self.put_i32(w);
        self.put_i32(h);
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
    fn place_box_encodes_21_bytes_le() {
        // Catches: PlaceBox payload size/endianness drifting from apply.js Op.PlaceBox (#30).
        let mut buf = CommandBuffer::new();
        buf.place_box(7, 11, -2, 33, 44);
        let bytes = buf.take();
        assert_eq!(bytes.len(), 21);
        assert_eq!(bytes[0], opcode::PLACE_BOX);
        assert_eq!(u32::from_le_bytes(bytes[1..5].try_into().unwrap()), 7);
        assert_eq!(i32::from_le_bytes(bytes[5..9].try_into().unwrap()), 11);
        assert_eq!(i32::from_le_bytes(bytes[9..13].try_into().unwrap()), -2);
        assert_eq!(i32::from_le_bytes(bytes[13..17].try_into().unwrap()), 33);
        assert_eq!(i32::from_le_bytes(bytes[17..21].try_into().unwrap()), 44);
    }
}
