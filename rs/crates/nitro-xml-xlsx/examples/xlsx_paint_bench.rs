// SPDX-License-Identifier: MIT
//! Native first-frame open/paint timing for M1 xlsx S (`cells_1k.xlsx`).
//! Not a gate — report-only aid for #30.
//!
//! ```text
//! cargo run -p nitro-xml-xlsx --release --example xlsx_paint_bench
//! ```

use std::time::Instant;

use nitro_xml_xlsx::lazy::{open_first_view, ViewportRect};
use nitro_xml_xlsx::render::{paint_grid_frame_vec, GridPaintViewport};

fn median(xs: &mut [f64]) -> f64 {
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs[xs.len() / 2]
}

fn main() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../fixtures/generated/xlsx/cells_1k.xlsx"
    );
    let bytes = std::fs::read(path).expect("cells_1k.xlsx");
    let view = ViewportRect {
        r0: 0,
        c0: 0,
        r1: 40,
        c1: 20,
    };
    let viewport = GridPaintViewport {
        scroll_x: 0,
        scroll_y: 0,
        width: 800,
        height: 600,
        frozen_rows: 0,
        frozen_cols: 0,
        zoom_pct: None,
    };

    for _ in 0..8 {
        let _ = open_first_view(&bytes, view).unwrap();
    }

    let mut t_open = Vec::with_capacity(31);
    let mut t_clone = Vec::with_capacity(31);
    let mut t_paint = Vec::with_capacity(31);
    let mut ncmds = 0usize;
    for _ in 0..31 {
        let t0 = Instant::now();
        let fv = open_first_view(&bytes, view).expect("open_first_view");
        t_open.push(t0.elapsed().as_secs_f64() * 1000.0);

        let t1 = Instant::now();
        let mut wb = fv.workbook().clone();
        let sheet_idx = wb.active_sheet as usize;
        let ws = fv.sheet(sheet_idx).cloned();
        if let Some(ref s) = ws {
            wb.active_worksheet = Some(s.clone());
        }
        let mut sheets = vec![None; wb.sheets.len()];
        sheets[sheet_idx] = ws;
        t_clone.push(t1.elapsed().as_secs_f64() * 1000.0);

        let sheet = sheets[sheet_idx].as_ref().unwrap();
        let mut next_id = 1u32;
        let mut intern = move |_s: &str| -> u32 {
            let id = next_id;
            next_id += 1;
            id
        };
        let t2 = Instant::now();
        let cmds = paint_grid_frame_vec(&mut intern, &wb, sheet, sheet_idx, &sheets, &viewport);
        t_paint.push(t2.elapsed().as_secs_f64() * 1000.0);
        ncmds = cmds.len();
    }
    println!(
        "cells_1k open_first_view={:.3}ms clone_wb+sheet={:.3}ms paint={:.3}ms cmd_bytes={ncmds}",
        median(&mut t_open),
        median(&mut t_clone),
        median(&mut t_paint)
    );
}
