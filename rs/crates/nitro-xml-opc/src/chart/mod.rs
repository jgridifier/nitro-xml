// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/; changes listed in PROVENANCE.md.
//! Format-neutral chart model and `c:chartSpace` parser.

mod data_cache;
mod dom;
mod manual_layout;
mod model;
mod parse_chrome;
mod parse_plot;
mod parse_series;
mod parse_space;
mod parse_text;
mod parse_util;
mod xml_serialize;

pub use data_cache::{chart_cache_values, chart_source_values};
pub use model::*;
pub use parse_space::{parse_chart_space_element, parse_chart_space_xml};
