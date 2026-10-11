// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/chart.ts (parseChartGraphic host side);
// deep chartSpace deferred to #28. Layout labels need a light title/kind peek until then.

use indexmap::IndexMap;
use serde_json::{json, Value};

use nitro_xml_opc::opc::relationship_types;
use nitro_xml_opc::opc::{resolve_part_path, RelMode, Relationship};
use nitro_xml_opc::xml::{parse_xml_str, Element};

use crate::xml_util::{get_r, ElementExt};

pub const CHART_NOTICE: &str = "The chart is drawn from the values cached in its part (bar, column, line, area, pie, doughnut, scatter and radar charts; other kinds show a labelled frame). It is not editable, and the chart part and its workbook are preserved unchanged on save.";

pub const CHART_GRAPHIC_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";

/// Chart families the shared painter draws (upstream `DRAWN_CHART_TYPES`).
const DRAWN_CHART_TYPES: &[&str] = &[
    "bar", "column", "line", "area", "pie", "doughnut", "scatter", "radar",
];

const CHART_GROUP_LOCALS: &[(&str, &str)] = &[
    ("barChart", "bar"),
    ("bar3DChart", "bar"),
    ("lineChart", "line"),
    ("line3DChart", "line"),
    ("areaChart", "area"),
    ("area3DChart", "area"),
    ("pieChart", "pie"),
    ("pie3DChart", "pie"),
    ("ofPieChart", "ofPie"),
    ("doughnutChart", "doughnut"),
    ("scatterChart", "scatter"),
    ("radarChart", "radar"),
    ("bubbleChart", "bubble"),
    ("stockChart", "stock"),
    ("surfaceChart", "surface"),
    ("surface3DChart", "surface"),
];

pub fn parse_chart_graphic(
    graphic_data: &Element,
    doc_pr: Option<&Element>,
    rels: &IndexMap<String, Relationship>,
    is_anchor: bool,
) -> Value {
    let chart_el = graphic_data
        .element_children()
        .into_iter()
        .find(|c| c.local == "chart");
    let rel_id = get_r(chart_el, "id");
    let mut issues = Vec::new();
    let mut part_name: Option<String> = None;
    if rel_id.is_none() {
        issues.push(json!({
            "code": "CHART_RELATIONSHIP_MISSING",
            "message": "The graphic has no c:chart/@r:id.",
        }));
    } else if let Some(rid) = &rel_id {
        let rel = rels.get(rid);
        if rel.is_none() || rel.map(|r| r.mode) == Some(RelMode::External) {
            issues.push(json!({
                "code": "CHART_RELATIONSHIP_UNRESOLVED",
                "message": format!("Chart relationship {} does not point at a part in this package.", rid),
            }));
        } else if let Some(rel) = rel {
            if rel.type_ != relationship_types::CHART {
                issues.push(json!({
                    "code": "CHART_RELATIONSHIP_TYPE",
                    "message": format!("Chart relationship {} has type {}, expected {}.", rid, rel.type_, relationship_types::CHART),
                }));
            }
            part_name = Some(resolve_part_path("word/document.xml", &rel.target));
        }
    }
    let mut obj = serde_json::Map::new();
    obj.insert(
        "placement".into(),
        json!(if is_anchor { "anchor" } else { "inline" }),
    );
    if let Some(dp) = doc_pr {
        if let Some(name) = dp
            .attrs
            .iter()
            .find(|a| a.local == "name")
            .map(|a| a.value.clone())
        {
            if !name.is_empty() {
                obj.insert("name".into(), json!(name));
            }
        }
        if let Some(id) = dp
            .attrs
            .iter()
            .find(|a| a.local == "id")
            .map(|a| a.value.clone())
        {
            if !id.is_empty() {
                obj.insert("id".into(), json!(id));
            }
        }
    }
    if let Some(rid) = rel_id {
        obj.insert("relId".into(), json!(rid));
    }
    if let Some(pn) = part_name {
        obj.insert("partName".into(), json!(pn));
    }
    obj.insert("notice".into(), json!(CHART_NOTICE));
    obj.insert("issues".into(), json!(issues));
    Value::Object(obj)
}

/// Lightweight chart-part peek for Print Layout labels (full `chartSpace` parse is #28).
/// Sets `title` and a stub `chartSpace: { type?, drawn }` used by [`crate::chart_paint`].
pub fn resolve_document_charts(
    blocks: &mut [Value],
    read_text: &mut dyn FnMut(&str) -> Option<String>,
) {
    for block in blocks {
        resolve_block_charts(block, read_text);
    }
}

fn resolve_block_charts(block: &mut Value, read_text: &mut dyn FnMut(&str) -> Option<String>) {
    if block.get("runs").is_some() {
        if let Some(runs) = block.get_mut("runs").and_then(|v| v.as_array_mut()) {
            for run in runs {
                if let Some(chart) = run.get_mut("image").and_then(|img| img.get_mut("chart")) {
                    resolve_chart_part_light(chart, read_text);
                }
            }
        }
        return;
    }
    if let Some(rows) = block.get_mut("rows").and_then(|v| v.as_array_mut()) {
        for row in rows {
            let cells: Option<&mut Vec<Value>> = if row.is_array() {
                row.as_array_mut()
            } else {
                row.get_mut("cells").and_then(|v| v.as_array_mut())
            };
            let Some(cells) = cells else { continue };
            for cell in cells.iter_mut() {
                if let Some(paras) = cell.get_mut("paragraphs").and_then(|v| v.as_array_mut()) {
                    resolve_document_charts(paras, read_text);
                }
            }
        }
    }
}

fn resolve_chart_part_light(chart: &mut Value, read_text: &mut dyn FnMut(&str) -> Option<String>) {
    let Some(part_name) = chart
        .get("partName")
        .and_then(|v| v.as_str())
        .map(str::to_string)
    else {
        return;
    };
    let Some(xml) = read_text(&part_name) else {
        if let Some(issues) = chart.get_mut("issues").and_then(|v| v.as_array_mut()) {
            issues.push(json!({
                "code": "CHART_PART_MISSING",
                "message": format!("The chart part {part_name} is not in the package."),
            }));
        }
        return;
    };
    let Ok(doc) = parse_xml_str(&xml) else {
        if let Some(issues) = chart.get_mut("issues").and_then(|v| v.as_array_mut()) {
            issues.push(json!({
                "code": "CHART_PART_UNREADABLE",
                "message": format!("The chart part {part_name} could not be read."),
            }));
        }
        return;
    };
    let root = &doc.root;
    // Modern `cx:chartSpace` (chartex) has no classic plot groups — undrawn frame.
    if root.local == "chartSpace"
        && root
            .ns
            .as_deref()
            .is_some_and(|ns| ns.contains("chartex") || ns.contains("2014/chartex"))
    {
        if let Some(obj) = chart.as_object_mut() {
            obj.insert("chartSpace".into(), json!({ "drawn": false }));
        }
        return;
    }
    let (kind, drawn, title) = peek_chart_space(root);
    if let Some(obj) = chart.as_object_mut() {
        let mut space = serde_json::Map::new();
        if let Some(k) = kind {
            space.insert("type".into(), json!(k));
        }
        space.insert("drawn".into(), json!(drawn));
        obj.insert("chartSpace".into(), Value::Object(space));
        if let Some(t) = title {
            if !t.is_empty() {
                obj.insert("title".into(), json!(t));
            }
        }
    }
}

fn peek_chart_space(root: &Element) -> (Option<String>, bool, Option<String>) {
    let chart = root
        .element_children()
        .into_iter()
        .find(|e| e.local == "chart");
    let Some(chart) = chart else {
        return (None, false, None);
    };
    let auto_deleted = chart
        .element_children()
        .into_iter()
        .find(|e| e.local == "autoTitleDeleted")
        .map(|e| {
            let val = e
                .attrs
                .iter()
                .find(|a| a.local == "val")
                .map(|a| a.value.as_str());
            match val {
                None | Some("1") | Some("true") => true,
                Some("0") | Some("false") => false,
                _ => true,
            }
        })
        .unwrap_or(false);
    let title = if auto_deleted {
        None
    } else {
        chart
            .element_children()
            .into_iter()
            .find(|e| e.local == "title")
            .map(|title_el| collect_title_text(title_el))
            .filter(|s| !s.is_empty())
    };
    let plot = chart
        .element_children()
        .into_iter()
        .find(|e| e.local == "plotArea");
    let mut kind = None;
    if let Some(plot) = plot {
        for child in plot.element_children() {
            if let Some((_, mapped)) = CHART_GROUP_LOCALS
                .iter()
                .find(|(local, _)| child.local == *local)
            {
                let mut k = (*mapped).to_string();
                if k == "bar" {
                    let bar_dir = child
                        .element_children()
                        .into_iter()
                        .find(|e| e.local == "barDir")
                        .and_then(|e| {
                            e.attrs
                                .iter()
                                .find(|a| a.local == "val")
                                .map(|a| a.value.as_str())
                        });
                    // Upstream summary: bar + barDir != "bar" → column.
                    if bar_dir != Some("bar") {
                        k = "column".into();
                    }
                }
                kind = Some(k);
                break;
            }
        }
    }
    let drawn = kind
        .as_deref()
        .is_some_and(|k| DRAWN_CHART_TYPES.contains(&k));
    (kind, drawn, title)
}

fn collect_title_text(title: &Element) -> String {
    let mut out = String::new();
    collect_a_t(title, &mut out);
    out
}

fn collect_a_t(el: &Element, out: &mut String) {
    if el.local == "t" {
        out.push_str(&el.text_content());
        return;
    }
    // `c:strRef` / `c:strLit` title text uses `c:v` (prefer rich `a:t` when present).
    if el.local == "v" {
        if out.is_empty() {
            out.push_str(&el.text_content());
        }
        return;
    }
    for child in el.element_children() {
        collect_a_t(child, out);
    }
}

/// Resolve chart parts on every story that can hold drawings (body, notes, headers).
pub fn resolve_loaded_charts(
    blocks: &mut [Value],
    footnotes: Option<&mut Vec<Value>>,
    endnotes: Option<&mut Vec<Value>>,
    sections: Option<&mut Vec<Value>>,
    read_text: &mut dyn FnMut(&str) -> Option<String>,
) {
    resolve_document_charts(blocks, read_text);
    if let Some(notes) = footnotes {
        for note in notes {
            if let Some(b) = note.get_mut("blocks").and_then(|v| v.as_array_mut()) {
                resolve_document_charts(b, read_text);
            }
        }
    }
    if let Some(notes) = endnotes {
        for note in notes {
            if let Some(b) = note.get_mut("blocks").and_then(|v| v.as_array_mut()) {
                resolve_document_charts(b, read_text);
            }
        }
    }
    if let Some(sections) = sections {
        for section in sections {
            for key in ["headers", "footers"] {
                if let Some(parts) = section.get_mut(key).and_then(|v| v.as_array_mut()) {
                    for part in parts {
                        if let Some(b) = part.get_mut("blocks").and_then(|v| v.as_array_mut()) {
                            resolve_document_charts(b, read_text);
                        }
                    }
                }
            }
        }
    }
}
