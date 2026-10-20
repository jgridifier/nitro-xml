// SPDX-License-Identifier: MIT
//! Compact per-frame command buffer opcodes (little-endian payloads).
//!
//! This is the **seed set** for Stage 2. Upstream painters also emit SVG NS
//! creates, text nodes, style property writes, `classList`, insert/remove,
//! markup parsing and canvas state — listed as gaps in `docs/boundary.md`.
//!
//! | Op | Tag | Payload |
//! |---|---|---|
//! | FillRect | 1 | x:i32 y:i32 w:i32 h:i32 color:u32 |
//! | StrokeRect | 2 | x:i32 y:i32 w:i32 h:i32 color:u32 line:f32 |
//! | DrawImage | 3 | img:u32 x:i32 y:i32 w:i32 h:i32 |
//! | DrawText | 4 | x:i32 y:i32 font:u32 size:f32 text:u32 color:u32 |
//! | CreateElement | 5 | id:u32 tag:u32 parent:u32 |
//! | SetAttr | 6 | id:u32 key:u32 val:u32 |
//! | AppendChild | 7 | parent:u32 child:u32 |
//! | SetText | 8 | id:u32 text_id:u32 |
//! | SetStyle | 9 | id:u32 prop_id:u32 val_id:u32 |
//! | SetInnerMarkup | 10 | parent:u32 markup:u32 |
//! | PlaceBox | 11 | id:u32 x:i32 y:i32 w:i32 h:i32 — CSSOM left/top/width/height (#30) |
//!
//! Seed protocol for painters / the loader (#37). Not called from the shipped
//! path yet; Stage 3+ ports will encode through these tags.
#[allow(dead_code)]
pub mod opcode {
    pub const FILL_RECT: u8 = 1;
    pub const STROKE_RECT: u8 = 2;
    pub const DRAW_IMAGE: u8 = 3;
    pub const DRAW_TEXT: u8 = 4;
    pub const CREATE_ELEMENT: u8 = 5;
    pub const SET_ATTR: u8 = 6;
    pub const APPEND_CHILD: u8 = 7;
    pub const SET_TEXT: u8 = 8;
    pub const SET_STYLE: u8 = 9;
    pub const SET_INNER_MARKUP: u8 = 10;
    pub const PLACE_BOX: u8 = 11;
}
