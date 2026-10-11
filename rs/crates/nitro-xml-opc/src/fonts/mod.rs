// SPDX-License-Identifier: MIT
//! Configurable webfont source for the renderer (PRODUCT_SPEC.md R0 / #121).
//!
//! Hosts choose the remote CSS2 base (default: upstream Google Fonts), point it
//! at an internal mirror, or turn remote fetching off. When fetching is off,
//! painters use locally available fonts and record each substitution. No I/O
//! happens here — callers only receive a stylesheet href (or nothing).

pub mod google_webfonts;
pub mod source;

pub use google_webfonts::{
    build_google_fonts_fragment, build_google_fonts_href, GOOGLE_FONTS_CSS2_BASE,
    GOOGLE_WEBFONT_AXIS,
};
pub use source::{
    resolve_webfont_plan, FontSource, FontSubstitution, FONT_SUBSTITUTION_CODE,
};
