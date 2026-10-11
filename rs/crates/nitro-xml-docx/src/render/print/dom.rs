// SPDX-License-Identifier: MIT
//! DOM opcode helpers for one Print Layout frame.

use nitro_xml_render::CommandBuffer;

pub trait StringIntern {
    fn intern(&mut self, s: &str) -> u32;
}

pub struct Dom<'a, I: StringIntern> {
    pub buf: &'a mut CommandBuffer,
    pub intern: &'a mut I,
    next: u32,
}

impl<'a, I: StringIntern> Dom<'a, I> {
    pub fn new(buf: &'a mut CommandBuffer, intern: &'a mut I) -> Self {
        Self {
            buf,
            intern,
            next: 1,
        }
    }

    pub fn el(&mut self, parent: u32, tag: &str) -> u32 {
        let id = self.next;
        self.next += 1;
        let tag_id = self.intern.intern(tag);
        self.buf.create_element(id, tag_id, parent);
        id
    }

    pub fn attr(&mut self, id: u32, key: &str, val: &str) {
        let k = self.intern.intern(key);
        let v = self.intern.intern(val);
        self.buf.set_attr(id, k, v);
    }

    pub fn class(&mut self, id: u32, class: &str) {
        self.attr(id, "class", class);
    }

    pub fn style(&mut self, id: u32, prop: &str, val: &str) {
        let p = self.intern.intern(prop);
        let v = self.intern.intern(val);
        self.buf.set_style(id, p, v);
    }

    pub fn text(&mut self, id: u32, text: &str) {
        let tid = self.intern.intern(text);
        self.buf.set_text(id, tid);
    }

    pub fn markup(&mut self, id: u32, markup: &str) {
        let mid = self.intern.intern(markup);
        self.buf.set_inner_markup(id, mid);
    }

    pub fn px(&mut self, id: u32, prop: &str, n: f64) {
        let val = format!("{}px", js_num(n));
        self.style(id, prop, &val);
    }
}

/// Shortest-enough decimal for CSS lengths (avoids trailing zeros).
pub fn js_num(n: f64) -> String {
    if !n.is_finite() {
        return "0".into();
    }
    let neg = n.is_sign_negative() && n != 0.0;
    let s = format!("{:.6}", n.abs());
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    let body = if trimmed.is_empty() { "0" } else { trimmed };
    if neg {
        format!("-{body}")
    } else {
        body.to_string()
    }
}
