// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/conditional.ts,
// layout/cf-evaluator.ts, cf-helpers.ts, cf-values.ts, ui/grid/icon-sets.ts;
// changes listed in PROVENANCE.md.
//! Conditional formatting read + evaluate + icon SVG (#150 soft / #18 leftover).

mod eval;
mod icon_sets;
mod read;

pub use eval::{cf_icon_at, CfIcon};
pub use icon_sets::icon_svg;
pub use read::read_conditional_formats;
