// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec upstream/src/ui/src/pptx/render/connector-style.ts; changes listed in PROVENANCE.md.

//! Compound (double/triple) parallel-stroke geometry for OOXML line styles.
//! Port of upstream connector-style.ts helpers: offsets and widths for multi-strand rendering.

/// OOXML compound line token type (`a:ln/@cmpd`).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompoundLineType {
    Sng,
    Dbl,
    ThickThin,
    ThinThick,
    Tri,
}

impl CompoundLineType {
    /// Parse from OOXML string value.
    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "sng" => Some(CompoundLineType::Sng),
            "dbl" => Some(CompoundLineType::Dbl),
            "thickThin" => Some(CompoundLineType::ThickThin),
            "thinThick" => Some(CompoundLineType::ThinThick),
            "tri" => Some(CompoundLineType::Tri),
            _ => None,
        }
    }

    /// Canonical string representation.
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            CompoundLineType::Sng => "sng",
            CompoundLineType::Dbl => "dbl",
            CompoundLineType::ThickThin => "thickThin",
            CompoundLineType::ThinThick => "thinThick",
            CompoundLineType::Tri => "tri",
        }
    }
}

/// Compute perpendicular offsets for compound (double/triple) line styles.
/// Returns an array of offset distances from the centre line in px. A single
/// line style returns `[0]`.
///
/// # Arguments
/// * `compound_line` - OOXML `a:ln/@cmpd` value (e.g. `"dbl"`, `"tri"`).
/// * `stroke_width` - The resolved stroke width in pixels.
///
/// # Returns
/// Array of offset distances, length 1 (sng), 2 (dbl/thickThin/thinThick), or 3 (tri).
pub fn get_compound_line_offsets(compound_line: Option<&str>, stroke_width: f64) -> Vec<f64> {
    if compound_line.is_none() || compound_line == Some("sng") {
        return vec![0.0];
    }

    let gap = (stroke_width * 0.6).max(1.5);

    match compound_line {
        Some("dbl") => vec![-gap, gap],
        Some("thickThin") => vec![-gap * 0.6, gap],
        Some("thinThick") => vec![-gap, gap * 0.6],
        Some("tri") => vec![-gap, 0.0, gap],
        _ => vec![0.0],
    }
}

/// Compute individual stroke widths for each parallel path in a compound line.
/// The array length matches the one returned by [`get_compound_line_offsets`].
///
/// # Arguments
/// * `compound_line` - OOXML `a:ln/@cmpd` value.
/// * `stroke_width` - The resolved stroke width in pixels.
///
/// # Returns
/// Array of individual strand widths, each ≥ 1.0 px.
pub fn get_compound_line_widths(compound_line: Option<&str>, stroke_width: f64) -> Vec<f64> {
    let base = stroke_width.max(1.0);

    if compound_line.is_none() || compound_line == Some("sng") {
        return vec![base];
    }

    match compound_line {
        Some("dbl") => vec![base * 0.5, base * 0.5],
        Some("thickThin") => vec![base * 0.7, base * 0.3],
        Some("thinThick") => vec![base * 0.3, base * 0.7],
        Some("tri") => vec![base * 0.3, base * 0.4, base * 0.3],
        _ => vec![base],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: dbl compound line must yield two equal strands with a non-zero gap.
    #[test]
    fn test_dbl_offsets() {
        let offsets = get_compound_line_offsets(Some("dbl"), 10.0);
        assert_eq!(offsets.len(), 2);
        assert!(offsets[0] < 0.0);
        assert!(offsets[1] > 0.0);
        assert_eq!(offsets[0], -offsets[1]); // symmetric
    }

    // Catches: dbl compound line strands must each be half the total stroke width.
    #[test]
    fn test_dbl_widths() {
        let widths = get_compound_line_widths(Some("dbl"), 10.0);
        assert_eq!(widths.len(), 2);
        assert_eq!(widths[0], 5.0);
        assert_eq!(widths[1], 5.0);
    }

    // Catches: tri compound line must yield three strands with two outer and one center.
    #[test]
    fn test_tri_offsets() {
        let offsets = get_compound_line_offsets(Some("tri"), 10.0);
        assert_eq!(offsets.len(), 3);
        assert!(offsets[0] < 0.0);
        assert_eq!(offsets[1], 0.0);
        assert!(offsets[2] > 0.0);
    }

    // Catches: tri compound line strands must sum correctly with center line present.
    #[test]
    fn test_tri_widths() {
        let widths = get_compound_line_widths(Some("tri"), 10.0);
        assert_eq!(widths.len(), 3);
        assert_eq!(widths[0], 3.0);
        assert_eq!(widths[1], 4.0);
        assert_eq!(widths[2], 3.0);
    }

    // Catches: sng (or None) compound line must yield single strand at full width.
    #[test]
    fn test_sng_offsets() {
        assert_eq!(get_compound_line_offsets(None, 10.0), vec![0.0]);
        assert_eq!(get_compound_line_offsets(Some("sng"), 10.0), vec![0.0]);
    }

    #[test]
    fn test_sng_widths() {
        assert_eq!(get_compound_line_widths(None, 10.0), vec![10.0]);
        assert_eq!(get_compound_line_widths(Some("sng"), 10.0), vec![10.0]);
    }

    #[test]
    fn test_thick_thin_offsets() {
        let offsets = get_compound_line_offsets(Some("thickThin"), 10.0);
        assert_eq!(offsets.len(), 2);
        assert!(offsets[0].abs() < offsets[1]); // inner offset smaller
    }

    #[test]
    fn test_thick_thin_widths() {
        let widths = get_compound_line_widths(Some("thickThin"), 10.0);
        assert_eq!(widths.len(), 2);
        assert!(widths[0] > widths[1]); // first is thick
    }
}
