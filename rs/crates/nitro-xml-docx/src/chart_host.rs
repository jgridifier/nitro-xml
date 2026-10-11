// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/chart.ts (parseChartGraphic host side);
// deep chartSpace deferred to #28.

use indexmap::IndexMap;
use serde_json::{json, Value};

use nitro_xml_opc::opc::relationship_types;
use nitro_xml_opc::opc::{resolve_part_path, RelMode, Relationship};
use nitro_xml_opc::xml::Element;

use crate::xml_util::{get_r, ElementExt};

pub const CHART_NOTICE: &str = "The chart is drawn from the values cached in its part (bar, column, line, area, pie, doughnut, scatter and radar charts; other kinds show a labelled frame). It is not editable, and the chart part and its workbook are preserved unchanged on save.";

pub const CHART_GRAPHIC_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";

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
