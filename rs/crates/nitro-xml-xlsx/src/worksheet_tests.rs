// SPDX-License-Identifier: MIT
// Stage-16 worksheet tests (included from worksheet.rs).

use crate::formula_translate::translate_formula;
use crate::sheet_props::{default_sheet_view, read_sheet_view};
use nitro_xml_opc::xml::parse_xml_str;
use nitro_xml_opc::xml::Element;

#[test]
fn serde_json_large_integer_double() {
    // Catches: numeric `<v>` with scientific text must stringify like JSON.stringify(Number(v)).
    let n = "1.9999999991653392E+19".parse::<f64>().unwrap();
    let s = crate::number_format::js_number_string(n);
    assert_eq!(s, "19999999991653392000");
}

#[test]
fn shared_formula_slave_shifts_relative_refs() {
    // Catches: shared-formula slaves must shift relative A1 references like translateFormula.
    let master = "A1+$B$2+B$3+$C4";
    let slave = translate_formula(master, 1, 1);
    assert_eq!(slave, "B2+$B$2+C$3+$C5");
}

#[test]
fn missing_shared_string_warns() {
    // Catches: a shared-string index with no table entry must warn and skip the cell value.
    use crate::cells::CellStore;
    use crate::sheet_data::{read_sheet_data_xml, CellContext};
    use std::collections::{HashMap, HashSet};
    let xml = r#"<row r="1"><c r="A1" t="s"><v>9</v></c></row>"#;
    let mut warnings = Vec::new();
    let mut ctx = CellContext {
        shared_strings: &[],
        xf_map: &[],
        date1904: false,
        palette: None,
        dynamic_cells: &HashSet::new(),
        warnings: &mut warnings,
    };
    let mut store = CellStore::new();
    let mut row_info = HashMap::new();
    read_sheet_data_xml(&mut ctx, xml, &mut store, &mut row_info);
    assert!(warnings.iter().any(|w| w.contains("missing shared string")));
    assert_eq!(store.len(), 0);
}

#[test]
fn hidden_column_and_freeze_in_view_dump() {
    // Catches: column hidden flags and frozen panes must round-trip through sheet view/column parse.
    let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
      <sheetViews><sheetView><pane state="frozen" xSplit="1" ySplit="2" topLeftCell="C3"/></sheetView></sheetViews>
      <cols><col min="2" max="2" width="9" hidden="1" customWidth="1"/></cols>
    </worksheet>"#;
    let doc = parse_xml_str(xml).expect("xml");
    let by = |name: &str| -> Option<&Element> {
        doc.root
            .children
            .iter()
            .filter_map(|n| match n {
                nitro_xml_opc::xml::Node::Element(e) if e.local == name => Some(e),
                _ => None,
            })
            .next()
    };
    let view = read_sheet_view(by("sheetViews"));
    assert_eq!(view.freeze.as_ref().map(|f| f.cols), Some(1));
    assert_eq!(view.freeze.as_ref().map(|f| f.rows), Some(2));
    let cols = crate::sheet_props::read_columns(by("cols"), &[]);
    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0].hidden, Some(true));
    assert_eq!(view.show_grid_lines, default_sheet_view().show_grid_lines);
}

#[test]
fn catches_max_depth_worksheet_shell_open_and_paint() {
    // Catches: a worksheet sibling tree at MAX_XML_DEPTH blowing Drop/Clone/paint on open.
    use crate::model::DumpCellValue;
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../fixtures/fuzz/xlsx/deep-nest-4096.xlsx"
    );
    let bytes = std::fs::read(path).expect("fixture");
    let wb = crate::load_workbook(&bytes).expect("workbook opens");
    let sheets = crate::parse_all_worksheets(&bytes, &wb).expect("sheets");
    let sheet = sheets[0].as_ref().expect("sheet0");
    let a1 = sheet
        .cells
        .iter()
        .find(|c| c.row == 0 && c.col == 0)
        .expect("A1");
    match &a1.value {
        DumpCellValue::String(s) => assert_eq!(s, "deep-nest-ok"),
        other => panic!("A1 should be deep-nest-ok, got {other:?}"),
    }
    let mut next_id = 1u32;
    let mut intern = |_s: &str| -> u32 {
        let id = next_id;
        next_id += 1;
        id
    };
    let viewport = crate::GridPaintViewport {
        scroll_x: 0,
        scroll_y: 0,
        width: 800,
        height: 600,
        frozen_rows: 0,
        frozen_cols: 0,
        zoom_pct: None,
    };
    let cmds = crate::render::paint_grid_frame_vec(&mut intern, &wb, sheet, 0, &sheets, &viewport);
    assert!(
        !cmds.is_empty(),
        "paint must emit commands for the deep-nest workbook"
    );
}

#[test]
fn catches_over_max_depth_worksheet_opens_or_rejects_without_panic() {
    // Catches: depth > MAX_XML_DEPTH panicking instead of soft-empty / Err.
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../fixtures/fuzz/xlsx/deep-nest-over-max.xlsx"
    );
    let bytes = std::fs::read(path).expect("fixture");
    let result = std::panic::catch_unwind(|| crate::load_workbook(&bytes));
    assert!(result.is_ok(), "must not panic");
    let wb = result.unwrap();
    // Soft-empty sheet (shell parse failed) or full Err are both acceptable.
    if let Ok(wb) = wb {
        let _ = crate::parse_all_worksheets(&bytes, &wb);
    }
}

#[test]
fn catches_poi_46535_zoom_scale_and_hidden_gridlines() {
    // Catches: poi-46535 zoomScale=85 / showGridLines=0 dropped so live grid paints at 100%.
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../fixtures/real/xlsx/poi-46535.xlsx"
    );
    let bytes = std::fs::read(path).expect("fixture");
    let wb = crate::load_workbook(&bytes).expect("load");
    let ws = crate::parse_worksheet_at(&bytes, &wb, 0).expect("ws");
    assert!((ws.view.zoom - 85.0).abs() < 0.01);
    assert!(!ws.view.show_grid_lines);
}
