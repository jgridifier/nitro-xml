// SPDX-License-Identifier: MIT
//! Shared cmdbuf encode helpers for xlsx grid paint (#30).
//! Stack geometry intern + once-per-frame prop ids (same pattern as cells.rs).

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::intern::StringIntern;

/// CSS / attr keys used by grid chrome (headers, gridlines, selection, frame).
pub(crate) struct ChromeKeys {
    pub class: u32,
    pub style: u32,
    pub width: u32,
    pub height: u32,
    pub transform: u32,
    pub font_size: u32,
    pub line_height: u32,
    pub role: u32,
    pub data_index: u32,
    pub position: u32,
    pub overflow: u32,
    pub outline: u32,
    pub contain: u32,
    pub user_select: u32,
    pub webkit_user_select: u32,
    pub font_family: u32,
    pub part: u32,
    pub aria_rowcount: u32,
    pub aria_colcount: u32,
    pub aria_multiselectable: u32,
    pub aria_label: u32,
    pub aria_live: u32,
}

impl ChromeKeys {
    pub(crate) fn new<I: StringIntern>(intern: &mut I) -> Self {
        Self {
            class: intern.intern("class"),
            style: intern.intern("style"),
            width: intern.intern("width"),
            height: intern.intern("height"),
            transform: intern.intern("transform"),
            font_size: intern.intern("fontSize"),
            line_height: intern.intern("lineHeight"),
            role: intern.intern("role"),
            data_index: intern.intern("data-index"),
            position: intern.intern("position"),
            overflow: intern.intern("overflow"),
            outline: intern.intern("outline"),
            contain: intern.intern("contain"),
            user_select: intern.intern("userSelect"),
            webkit_user_select: intern.intern("WebkitUserSelect"),
            font_family: intern.intern("fontFamily"),
            part: intern.intern("part"),
            aria_rowcount: intern.intern("aria-rowcount"),
            aria_colcount: intern.intern("aria-colcount"),
            aria_multiselectable: intern.intern("aria-multiselectable"),
            aria_label: intern.intern("aria-label"),
            aria_live: intern.intern("aria-live"),
        }
    }
}

#[inline]
pub(crate) fn set_style_v(
    buf: &mut CommandBuffer,
    intern: &mut impl StringIntern,
    id: u32,
    prop: u32,
    val: &str,
) {
    buf.set_style(id, prop, intern.intern(val));
}

#[inline]
pub(crate) fn set_attr_v(
    buf: &mut CommandBuffer,
    intern: &mut impl StringIntern,
    id: u32,
    key: u32,
    val: &str,
) {
    buf.set_attr(id, key, intern.intern(val));
}

fn push_bytes(buf: &mut [u8], o: &mut usize, s: &[u8]) {
    buf[*o..*o + s.len()].copy_from_slice(s);
    *o += s.len();
}

fn push_i32(buf: &mut [u8], o: &mut usize, n: i32) {
    let mut tmp = [0u8; 16];
    let mut x = n.unsigned_abs();
    let neg = n < 0;
    let mut i = tmp.len();
    if x == 0 {
        i -= 1;
        tmp[i] = b'0';
    } else {
        while x > 0 {
            i -= 1;
            tmp[i] = b'0' + (x % 10) as u8;
            x /= 10;
        }
    }
    if neg {
        i -= 1;
        tmp[i] = b'-';
    }
    push_bytes(buf, o, &tmp[i..]);
}

/// `left:Xpx;top:Ypx;width:Wpx;height:Hpx` — no `format!`.
pub(crate) fn intern_box_geometry(
    intern: &mut impl StringIntern,
    left: i32,
    top: i32,
    w: i32,
    h: i32,
) -> u32 {
    let mut buf = [0u8; 96];
    let mut o = 0usize;
    push_bytes(&mut buf, &mut o, b"left:");
    push_i32(&mut buf, &mut o, left);
    push_bytes(&mut buf, &mut o, b"px;top:");
    push_i32(&mut buf, &mut o, top);
    push_bytes(&mut buf, &mut o, b"px;width:");
    push_i32(&mut buf, &mut o, w);
    push_bytes(&mut buf, &mut o, b"px;height:");
    push_i32(&mut buf, &mut o, h);
    push_bytes(&mut buf, &mut o, b"px");
    // SAFETY: only ASCII digits and fixed prefixes.
    intern.intern(unsafe { std::str::from_utf8_unchecked(&buf[..o]) })
}

/// `{n}px` — no `format!`.
pub(crate) fn intern_px(intern: &mut impl StringIntern, n: i32) -> u32 {
    let mut buf = [0u8; 24];
    let mut o = 0usize;
    push_i32(&mut buf, &mut o, n);
    push_bytes(&mut buf, &mut o, b"px");
    intern.intern(unsafe { std::str::from_utf8_unchecked(&buf[..o]) })
}

/// `translate3d(Xpx,Ypx,0)` — no `format!`.
pub(crate) fn intern_translate3d(intern: &mut impl StringIntern, x: i32, y: i32) -> u32 {
    let mut buf = [0u8; 64];
    let mut o = 0usize;
    push_bytes(&mut buf, &mut o, b"translate3d(");
    push_i32(&mut buf, &mut o, x);
    push_bytes(&mut buf, &mut o, b"px,");
    push_i32(&mut buf, &mut o, y);
    push_bytes(&mut buf, &mut o, b"px,0)");
    intern.intern(unsafe { std::str::from_utf8_unchecked(&buf[..o]) })
}

/// Place via one `style` attr (four SetStyle → one SetAttr).
#[allow(clippy::too_many_arguments)] // mirrors upstream place(node, x, y, w, h) + buf/intern/keys
pub(crate) fn place_style_attr(
    buf: &mut CommandBuffer,
    intern: &mut impl StringIntern,
    keys: &ChromeKeys,
    id: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    buf.set_attr(id, keys.style, intern_box_geometry(intern, x, y, w, h));
}
