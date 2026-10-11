// SPDX-License-Identifier: MIT
//! Column block JSON → [`LayoutBlockBox`].

use crate::layout::LayoutBlockBox;

pub fn parse_block(value: &serde_json::Value) -> Option<LayoutBlockBox> {
    serde_json::from_value(value.clone()).ok()
}
