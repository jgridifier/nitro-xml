// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/visual-3d-materials.ts; changes listed in PROVENANCE.md.

/// CSS overrides that approximate an OOXML `a:sp3d/@prstMaterial` preset.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MaterialCssOverrides {
    pub box_shadow: Option<&'static str>,
    pub background_image: Option<&'static str>,
}

/// Returns CSS overrides for a theme-matrix material token (empty for unknown values).
pub fn get_material_css_overrides(material: &str) -> MaterialCssOverrides {
    match material {
        "matte" => MaterialCssOverrides {
            box_shadow: None,
            background_image: Some(
                "linear-gradient(180deg, rgba(255,255,255,0.04) 0%, transparent 40%, rgba(0,0,0,0.03) 100%)",
            ),
        },
        "metal" => MaterialCssOverrides {
            box_shadow: Some(
                "inset -3px -3px 8px rgba(255,255,255,0.45), inset 2px 2px 4px rgba(255,255,255,0.2), inset 0 0 2px rgba(0,0,0,0.15)",
            ),
            background_image: Some(
                "linear-gradient(135deg, rgba(255,255,255,0.25) 0%, rgba(255,255,255,0.08) 20%, transparent 45%, rgba(0,0,0,0.06) 75%, rgba(255,255,255,0.1) 100%)",
            ),
        },
        "softmetal" => MaterialCssOverrides {
            box_shadow: Some(
                "inset -2px -2px 6px rgba(255,255,255,0.3), inset 1px 1px 3px rgba(255,255,255,0.12)",
            ),
            background_image: Some(
                "linear-gradient(135deg, rgba(255,255,255,0.15) 0%, rgba(255,255,255,0.04) 25%, transparent 50%, rgba(0,0,0,0.04) 85%, rgba(255,255,255,0.06) 100%)",
            ),
        },
        _ => MaterialCssOverrides::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: metal preset constants drift from upstream visual-3d-materials.ts.
    #[test]
    fn metal_matches_upstream_inset_and_gradient() {
        let m = get_material_css_overrides("metal");
        let shadow = m.box_shadow.expect("shadow");
        assert!(shadow.contains("inset -3px -3px 8px"));
        let bg = m.background_image.expect("gradient");
        assert!(bg.starts_with("linear-gradient(135deg"));
    }
}
