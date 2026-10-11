// SPDX-License-Identifier: MIT
//! OPC packaging: content types, relationships, part resolution, safe hrefs, format allow-list.
//!
//! Ports ChristopherVR/ooxml@e66aaec `src/core/opc/` (content-types, relationships,
//! relationship-types, safe-href, package helpers) and the `SourceIndex` reader from
//! `src/core/xlsx/read/package.ts`.

mod content_types;
mod error;
mod format;
mod package;
pub mod rel_types;
mod relationships;
mod safe_href;
mod signature;

pub use content_types::{content_type_for_part, parse_content_types, ContentTypes};
pub use error::OpcError;
pub use format::AllowedFormat;
pub use package::{Package, PropertyParts};
pub use rel_types as relationship_types;
pub use relationships::{
    parse_relationships, relationships_part_for, resolve_part_path, RelMode, Relationship,
};
pub use safe_href::{
    hyperlink_policy, is_openable_hyperlink_href, is_safe_hyperlink_href,
    is_storable_hyperlink_href, HyperlinkPolicy, HyperlinkRejection,
};
pub use signature::{detect_digital_signatures, is_signature_part, SIGNATURE_PART_PREFIX};
