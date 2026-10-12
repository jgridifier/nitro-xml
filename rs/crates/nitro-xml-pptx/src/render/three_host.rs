// SPDX-License-Identifier: MIT
//! P13 three.js / WebGL view host.
//!
//! M1 is complete as a no-op host: charts and SmartArt stay on the SVG painters.
//! There is no WebGL context in this slice.

/// `false` for the whole M1 view. Callers keep the 2D SVG fallback.
pub fn is_available() -> bool {
    false
}
