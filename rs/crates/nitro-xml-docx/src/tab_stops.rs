// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/tab-stops.ts; changes listed in PROVENANCE.md.

use serde_json::{json, Value};

use nitro_xml_opc::xml::Element;

use crate::simple_types::parse_signed_twips;
use crate::xml_util::{children_w, get_w};

fn tab_align(value: Option<&str>) -> Option<&'static str> {
    match value {
        Some("clear") => Some("clear"),
        Some("start") => Some("start"),
        Some("center") => Some("center"),
        Some("end") => Some("end"),
        Some("decimal") => Some("decimal"),
        Some("bar") => Some("bar"),
        Some("num") => Some("num"),
        Some("left") => Some("left"),
        Some("right") => Some("right"),
        _ => None,
    }
}

fn tab_leader(value: Option<&str>) -> Option<&'static str> {
    match value {
        Some("dot") => Some("dot"),
        Some("hyphen") => Some("hyphen"),
        Some("underscore") => Some("underscore"),
        Some("heavy") => Some("heavy"),
        Some("middleDot") => Some("middleDot"),
        _ => None,
    }
}

pub fn parse_tab_stops(tabs: Option<&Element>) -> Vec<Value> {
    let mut stops = Vec::new();
    let Some(tabs) = tabs else {
        return stops;
    };
    for tab in children_w(tabs, "tab") {
        let align = tab_align(get_w(Some(tab), "val").as_deref());
        let pos = parse_signed_twips(get_w(Some(tab), "pos").as_deref());
        if align.is_none() || pos.is_none() {
            continue;
        }
        let mut stop = serde_json::Map::new();
        stop.insert("posTwips".into(), json!(pos.unwrap()));
        stop.insert("align".into(), json!(align.unwrap()));
        if let Some(leader) = tab_leader(get_w(Some(tab), "leader").as_deref()) {
            stop.insert("leader".into(), json!(leader));
        }
        stops.push(Value::Object(stop));
    }
    stops
}
