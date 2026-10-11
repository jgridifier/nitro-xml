// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xml/xml.ts, lite.ts and namespaces.ts; changes
// listed in PROVENANCE.md.
//! Strict streaming XML layer with namespaces.
//!
//! Pull-parses with `quick-xml` [`NsReader`](quick_xml::reader::NsReader), rejects DTD / ENTITY the
//! way upstream `parseXml` does, and builds a small owned tree for parts that need random access.

mod decode;
mod error;
mod namespaces;
mod parse;
mod tree;

pub use error::XmlError;
pub use namespaces::ns;
pub use parse::{parse_xml_bytes, parse_xml_str, parse_xml_str_with_options, ParseOptions};
pub use tree::{Attr, Document, Element, Node};
