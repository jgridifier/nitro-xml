// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/fill-style.ts
// (`getPatternTile`); changes listed in PROVENANCE.md.

//! OOXML `a:pattFill/@prst` tile markup for SVG `<pattern>` paint servers.

/// One preset pattern tile (`w`×`h` user units + inner SVG primitives).
#[derive(Debug, Clone, PartialEq)]
pub struct PatternTile {
    pub w: u32,
    pub h: u32,
    pub inner: String,
}

/// Build a tile for `preset` with foreground/background hex colours.
///
/// Returns `None` for presets we do not draw yet (callers keep a solid fallback).
pub fn get_pattern_tile(preset: &str, fg: &str, bg: &str) -> Option<PatternTile> {
    let s = 8u32;
    let rect = |x: u32, y: u32, w: u32, h: u32, fill: &str| {
        format!(r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"/>"#)
    };
    let bg_rect = |w: u32, h: u32| rect(0, 0, w, h, bg);
    let line = |x1: i32, y1: i32, x2: i32, y2: i32, stroke: &str, sw: u32| {
        format!(
            r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{stroke}" stroke-width="{sw}"/>"#
        )
    };
    let svg = |w: u32, h: u32, inner: String| PatternTile { w, h, inner };

    Some(match preset {
        "dkDnDiag" => svg(
            s,
            s,
            format!(
                "{}{}{}{}",
                bg_rect(s, s),
                line(0, 0, s as i32, s as i32, fg, 3),
                line(-(s as i32), 0, 0, s as i32, fg, 3),
                line(s as i32, 0, (s * 2) as i32, s as i32, fg, 3),
            ),
        ),
        "dnDiag" => svg(
            s,
            s,
            format!(
                "{}{}{}{}",
                bg_rect(s, s),
                line(0, 0, s as i32, s as i32, fg, 2),
                line(-(s as i32), 0, 0, s as i32, fg, 2),
                line(s as i32, 0, (s * 2) as i32, s as i32, fg, 2),
            ),
        ),
        "smGrid" => svg(
            s,
            s,
            format!(
                "{}{}{}",
                bg_rect(s, s),
                rect(0, 0, s, 1, fg),
                rect(0, 0, 1, s, fg)
            ),
        ),
        "lgGrid" => svg(
            16,
            16,
            format!(
                "{}{}{}",
                bg_rect(16, 16),
                rect(0, 0, 16, 1, fg),
                rect(0, 0, 1, 16, fg)
            ),
        ),
        "cross" => svg(
            s,
            s,
            format!(
                "{}{}{}",
                bg_rect(s, s),
                rect(3, 0, 2, s, fg),
                rect(0, 3, s, 2, fg)
            ),
        ),
        "horz" => svg(s, s, format!("{}{}", bg_rect(s, s), rect(0, 3, s, 2, fg))),
        "vert" => svg(s, s, format!("{}{}", bg_rect(s, s), rect(3, 0, 2, s, fg))),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::get_pattern_tile;

    #[test]
    fn catches_pattern_tile_not_flattened_to_foreground_only() {
        // Catches: patterned outline painted as a flat strokeColor (fg) so hatching disappears.
        let tile = get_pattern_tile("dkDnDiag", "#1F4E79", "#FFF2CC").expect("tile");
        assert_eq!(tile.w, 8);
        assert!(
            tile.inner.contains("#FFF2CC"),
            "bg must appear: {}",
            tile.inner
        );
        assert!(
            tile.inner.contains("#1F4E79"),
            "fg must appear: {}",
            tile.inner
        );
        assert!(tile.inner.contains("<line"), "dkDnDiag uses diagonal lines");
    }

    #[test]
    fn catches_sm_grid_tile_has_crosshair_rects() {
        // Catches: smGrid outline falling back to solid when the preset table omits grids.
        let tile = get_pattern_tile("smGrid", "#1F4E79", "#FFF2CC").expect("tile");
        assert!(
            tile.inner.contains("width=\"8\" height=\"1\""),
            "{}",
            tile.inner
        );
        assert!(
            tile.inner.contains("width=\"1\" height=\"8\""),
            "{}",
            tile.inner
        );
    }
}
