// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec upstream/src/ui/src/pptx/render/text-warp-preset-definitions.ts; changes listed in PROVENANCE.md.
//! Spec-transcribed OOXML `prstTxWarp` guide formulas for the WordArt
//! "envelope" preset family (inflate/deflate/can), used by
//! `wordart_preset_sampler.rs` to sample the real top/bottom paths instead
//! of the reconstructed sine lobes in `wordart_envelope_curves.rs`.
//!
//! Source: `presetTextWarpDefinitions.xml`, the same `avLst`/`gdLst`/`pathLst`
//! table format ECMA-376 uses for preset *shapes* (`presetShapeDefinitions.xml`),
//! but for text warps. The file itself is not reproduced in the free ECMA-376
//! text; this transcription was sourced from LibreOffice's OOXML export table
//! (`oox/source/export/presetTextWarpDefinitions.xml` in LibreOffice/core,
//! https://github.com/LibreOffice/core), which LibreOffice uses to write
//! spec-conformant `prstTxWarp` XML when exporting its own Fontwork shapes to
//! `.pptx` - so its numbers must round-trip correctly through real PowerPoint,
//! unlike LibreOffice's IMPORT-side mapping (which discards the formulas in
//! favour of LibreOffice's own Fontwork geometry, and is what an earlier
//! investigation of this gap found and mistook for "no free source exists").
//! Guide names, formulas, and adjustment ranges below are copied verbatim.
//!
//! Each preset in `presetTextWarpDefinitions.xml` draws a `pathLst` of two or
//! more (top, bottom) path pairs: single-lobe presets (inflate/deflate/can)
//! define exactly one pair spanning the whole shape height; the two compound
//! "Deflate Inflate[ Deflate]" presets define one flat full-height pair (the
//! shape's own bounding-box edges, `y = t` and `y = b`) plus one curved pair
//! PER STACKED TEXT ROW (their own name literally lists the per-row
//! behaviour: "Deflate Inflate" = row 0 deflates, row 1 inflates). A single
//! line of WordArt text only ever occupies row 0, so this module's `top`/
//! `bottom` for a compound preset is path[0] (the flat top edge) and path[1]
//! (row 0's curve) - confirmed empirically (2026-09-06) against PowerPoint
//! COM screenshots: measuring the per-glyph ink-height ratio across a single
//! line of `textDeflateInflateDeflate` text against the row-0-pair
//! prediction gave a max ratio deviation of 0.036 (glyph height predicted to
//! within ~4% of measured, across 16 glyph positions); the alternative
//! "outermost internal pair" reading tried first (path[1] vs path[n-2],
//! skipping the flat edges but spanning every row) was off by up to 0.63 (63%),
//! i.e. clearly wrong. See `wordart_preset_sampler.rs` tests for the fixture.
//!
//! Coordinates are guide names to be resolved (via `evaluate_guides`)
//! against a normalised `w = h = 100000` box, matching the 0..100000 scale
//! `adj` values are already stored in (`textStyle.textWarpAdj` is the
//! raw, unscaled `val` from `a:avLst`), so the resolved guide values are
//! already fractions of `h` in `0..100000` and only need dividing by 100000.

use nitro_xml_opc::drawingml::GeometryGuide;

/// A single curve segment of a preset's top or bottom path, `x` always spanning `l..r`.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum WarpCurveSegment {
    Line {
        start_y: String,
        end_y: String,
    },
    Quad {
        start_y: String,
        ctrl_y: String,
        end_y: String,
    },
    Cubic {
        start_y: String,
        ctrl1_y: String,
        ctrl2_y: String,
        end_y: String,
    },
    Arc {
        pen_x: String,
        pen_y: String,
        w_r: String,
        h_r: String,
        st_ang: String,
        sw_ang: String,
    },
}

/// One preset's transcribed guide list plus its top/bottom curve segments.
#[allow(dead_code)]
pub struct WarpPresetDefinition {
    pub default_adj: i32,
    pub gd_list: Vec<GeometryGuide>,
    pub top: WarpCurveSegment,
    pub bottom: WarpCurveSegment,
}

fn line(start_y: &str, end_y: &str) -> WarpCurveSegment {
    WarpCurveSegment::Line {
        start_y: start_y.to_string(),
        end_y: end_y.to_string(),
    }
}

fn quad(start_y: &str, ctrl_y: &str, end_y: &str) -> WarpCurveSegment {
    WarpCurveSegment::Quad {
        start_y: start_y.to_string(),
        ctrl_y: ctrl_y.to_string(),
        end_y: end_y.to_string(),
    }
}

fn cubic(start_y: &str, ctrl1_y: &str, ctrl2_y: &str, end_y: &str) -> WarpCurveSegment {
    WarpCurveSegment::Cubic {
        start_y: start_y.to_string(),
        ctrl1_y: ctrl1_y.to_string(),
        ctrl2_y: ctrl2_y.to_string(),
        end_y: end_y.to_string(),
    }
}

fn arc(pen_y: &str, w_r: &str, h_r: &str, st_ang: &str, sw_ang: &str) -> WarpCurveSegment {
    WarpCurveSegment::Arc {
        pen_x: "l".to_string(),
        pen_y: pen_y.to_string(),
        w_r: w_r.to_string(),
        h_r: h_r.to_string(),
        st_ang: st_ang.to_string(),
        sw_ang: sw_ang.to_string(),
    }
}

fn guide(name: &str, formula: &str) -> GeometryGuide {
    GeometryGuide {
        name: name.to_string(),
        formula: formula.to_string(),
    }
}

/// Get a preset definition by name, or None if not available.
#[allow(dead_code)]
pub fn get_warp_preset_definition(preset: &str) -> Option<WarpPresetDefinition> {
    match preset {
        "textCanDown" => Some(WarpPresetDefinition {
            default_adj: 14286,
            gd_list: vec![
                guide("a", "pin 0 adj 33333"),
                guide("dy", "*/ a h 100000"),
                guide("y0", "+- t dy 0"),
                guide("y1", "+- b 0 dy"),
                guide("ncd2", "*/ cd2 -1 1"),
            ],
            top: arc("t", "wd2", "dy", "cd2", "ncd2"),
            bottom: arc("y1", "wd2", "dy", "cd2", "ncd2"),
        }),
        "textCanUp" => Some(WarpPresetDefinition {
            default_adj: 85714,
            gd_list: vec![
                guide("a", "pin 66667 adj 100000"),
                guide("dy1", "*/ a h 100000"),
                guide("dy", "+- h 0 dy1"),
                guide("y0", "+- t dy1 0"),
                guide("y1", "+- t dy 0"),
            ],
            top: arc("y1", "wd2", "dy", "cd2", "cd2"),
            bottom: arc("b", "wd2", "dy", "cd2", "cd2"),
        }),
        "textDeflate" => Some(WarpPresetDefinition {
            default_adj: 18750,
            gd_list: vec![
                guide("a", "pin 0 adj 37500"),
                guide("dy", "*/ a ss 100000"),
                guide("gd0", "*/ dy 4 3"),
                guide("gd1", "+- h 0 gd0"),
                guide("adjY", "+- t dy 0"),
                guide("y0", "+- t gd0 0"),
                guide("y1", "+- t gd1 0"),
                guide("x0", "+- l wd3 0"),
                guide("x1", "+- r 0 wd3"),
            ],
            top: cubic("t", "y0", "y0", "t"),
            bottom: cubic("b", "y1", "y1", "b"),
        }),
        "textDeflateBottom" => Some(WarpPresetDefinition {
            default_adj: 50000,
            gd_list: vec![
                guide("a", "pin 6250 adj 100000"),
                guide("dy", "*/ a ss 100000"),
                guide("dy2", "+- h 0 dy"),
                guide("y1", "+- t dy 0"),
                guide("cp", "+- y1 0 dy2"),
            ],
            top: line("t", "t"),
            bottom: quad("b", "cp", "b"),
        }),
        "textDeflateInflate" => Some(WarpPresetDefinition {
            default_adj: 35000,
            gd_list: vec![
                guide("a", "pin 5000 adj 95000"),
                guide("dy", "*/ a h 100000"),
                guide("del", "*/ h 5 100"),
                guide("dh1", "*/ h 45 100"),
                guide("dh2", "*/ h 55 100"),
                guide("yh", "+- dy 0 del"),
                guide("yl", "+- dy del 0"),
                guide("y3", "+- yh yh dh1"),
                guide("y4", "+- yl yl dh2"),
            ],
            // Row 0's pair (flat box-top edge, row 0's deflate curve)
            top: line("t", "t"),
            bottom: quad("dh1", "y3", "dh1"),
        }),
        "textDeflateInflateDeflate" => Some(WarpPresetDefinition {
            default_adj: 25000,
            gd_list: vec![
                guide("a", "pin 3000 adj 47000"),
                guide("dy", "*/ a h 100000"),
                guide("del", "*/ h 3 100"),
                guide("ey1", "*/ h 30 100"),
                guide("ey2", "*/ h 36 100"),
                guide("ey3", "*/ h 63 100"),
                guide("ey4", "*/ h 70 100"),
                guide("by", "+- b 0 dy"),
                guide("yh1", "+- dy 0 del"),
                guide("yl1", "+- dy del 0"),
                guide("yh2", "+- by 0 del"),
                guide("yl2", "+- by del 0"),
                guide("y1", "+- yh1 yh1 ey1"),
                guide("y2", "+- yl1 yl1 ey2"),
                guide("y3", "+- yh2 yh2 ey3"),
                guide("y4", "+- yl2 yl2 ey4"),
            ],
            // Row 0's pair (flat box-top edge, row 0's deflate curve)
            top: line("t", "t"),
            bottom: quad("ey1", "y1", "ey1"),
        }),
        "textDeflateTop" => Some(WarpPresetDefinition {
            default_adj: 50000,
            gd_list: vec![
                guide("a", "pin 0 adj 93750"),
                guide("dy", "*/ a h 100000"),
                guide("y1", "+- t dy 0"),
                guide("cp", "+- y1 dy 0"),
            ],
            top: quad("t", "cp", "t"),
            bottom: line("b", "b"),
        }),
        "textInflate" => Some(WarpPresetDefinition {
            default_adj: 18750,
            gd_list: vec![
                guide("a", "pin 0 adj 20000"),
                guide("dy", "*/ a h 100000"),
                guide("gd", "*/ dy 1 3"),
                guide("gd0", "+- 0 0 gd"),
                guide("gd1", "+- h 0 gd0"),
                guide("ty", "+- t dy 0"),
                guide("by", "+- b 0 dy"),
                guide("y0", "+- t gd0 0"),
                guide("y1", "+- t gd1 0"),
                guide("x0", "+- l wd3 0"),
                guide("x1", "+- r 0 wd3"),
            ],
            top: cubic("ty", "y0", "y0", "ty"),
            bottom: cubic("by", "y1", "y1", "by"),
        }),
        "textInflateBottom" => Some(WarpPresetDefinition {
            default_adj: 60000,
            gd_list: vec![
                guide("a", "pin 60000 adj 100000"),
                guide("dy", "*/ a h 100000"),
                guide("ty", "+- t dy 0"),
            ],
            top: line("t", "t"),
            bottom: quad("ty", "b", "ty"),
        }),
        "textInflateTop" => Some(WarpPresetDefinition {
            default_adj: 40000,
            gd_list: vec![
                guide("a", "pin 0 adj 50000"),
                guide("dy", "*/ a h 100000"),
                guide("ty", "+- t dy 0"),
            ],
            top: quad("ty", "t", "ty"),
            bottom: line("b", "b"),
        }),
        _ => None,
    }
}

/// Check if a preset name has a transcribed definition.
#[allow(dead_code)]
pub fn has_warp_preset_definition(preset: &str) -> bool {
    matches!(
        preset,
        "textInflate"
            | "textDeflate"
            | "textInflateBottom"
            | "textInflateTop"
            | "textDeflateBottom"
            | "textDeflateTop"
            | "textDeflateInflate"
            | "textDeflateInflateDeflate"
            | "textCanUp"
            | "textCanDown"
    )
}
