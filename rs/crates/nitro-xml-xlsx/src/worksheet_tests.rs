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
