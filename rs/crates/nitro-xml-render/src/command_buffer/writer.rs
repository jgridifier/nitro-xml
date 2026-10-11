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
}
