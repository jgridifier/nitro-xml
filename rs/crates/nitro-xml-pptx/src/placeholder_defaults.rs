// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/core/runtime/PptxHandlerRuntimePlaceholderLookup.ts
// and PptxHandlerRuntimePlaceholderDefaults.ts; changes in PROVENANCE.md.

use std::collections::BTreeMap;

use crate::inheritance::{placeholder_style_family, PlaceholderInfo};
use crate::level_style::{
    master_text_levels_for_placeholder, merge_level_styles, LevelStylesMap, MasterTxStyles,
};

pub fn placeholder_defaults_key(ph: &PlaceholderInfo) -> String {
    let family = placeholder_style_family(ph.type_.as_deref());
    if let Some(idx) = &ph.idx {
        format!("{}_{}", family, idx)
    } else {
        family
    }
}

pub fn lookup_effective_level_styles(
    layout_levels: Option<&LevelStylesMap>,
    master_shape_levels: Option<&LevelStylesMap>,
    master_tx: Option<&MasterTxStyles>,
    ph_type: Option<&str>,
    presentation_levels: Option<&LevelStylesMap>,
) -> LevelStylesMap {
    let master_text_levels = master_tx.map(|tx| master_text_levels_for_placeholder(tx, ph_type));
    let mut merged = LevelStylesMap::new();
    if let Some(m) = master_text_levels {
        merged = merge_level_styles(&merged, m);
    }
    if let Some(m) = master_shape_levels {
        merged = merge_level_styles(&merged, m);
    }
    if let Some(l) = layout_levels {
        merged = merge_level_styles(&merged, l);
    }
    if merged.is_empty() {
        if let Some(p) = presentation_levels {
            return p.clone();
        }
    }
    merged
}

pub type PlaceholderDefaultsCache = BTreeMap<String, LevelStylesMap>;
