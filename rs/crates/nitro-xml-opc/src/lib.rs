// SPDX-License-Identifier: MIT
//! Shared foundation for every Nitro XML format crate: the zip reader, the
//! streaming XML layer, OPC packaging and DrawingML.
//!
//! Ports (from ChristopherVR/ooxml@e66aaec):
//! - `src/core/xml/` → [`xml`]
//! - `src/core/opc/` → [`opc`]
//! - `src/core/drawingml/` → [`drawingml`]
//! - zip: upstream uses JSZip (third-party, not ported); [`zip`] replaces it.

pub mod chart;
pub mod drawingml;
pub mod fonts;
pub mod opc;
pub mod xml;
pub mod zip;
