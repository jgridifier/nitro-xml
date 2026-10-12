// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/package.ts (SourceIndex) and
// src/core/opc/package.ts (findOfficeDocumentPart); changes listed in PROVENANCE.md.
//! OPC `Package::open`: content types, cached relationships, part resolution, format allow-list.

use std::collections::HashMap;

use indexmap::IndexMap;

use crate::opc::content_types::{content_type_for_part, parse_content_types, ContentTypes};
use crate::opc::error::OpcError;
use crate::opc::format::{format_from_package, sniff_unsupported_container, AllowedFormat};
use crate::opc::relationship_types;
use crate::opc::relationships::{
    parse_relationships, relationships_part_for, resolve_part_path, RelMode, Relationship,
};
use crate::opc::signature::{detect_digital_signatures, is_signature_part};
use crate::zip::{ZipLimits, ZipPackage};

/// Property part paths discovered from package relationships (listed only; XML not parsed here).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyParts {
    pub core: Option<String>,
    pub app: Option<String>,
    pub custom: Option<String>,
}

/// Options for [`Package::open_with_options`]. Defaults match upstream absolute zip caps plus the
/// safer compression-ratio-with-floor policy (see [`ZipLimits::default`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct PackageOpenOptions {
    pub zip: ZipLimits,
}

impl PackageOpenOptions {
    /// Absolute zip caps only — same as upstream `readZipParts` defaults (no ratio guard).
    pub const fn upstream_parity() -> Self {
        Self {
            zip: ZipLimits::upstream_parity(),
        }
    }
}

/// An opened OPC package with cached content types and relationships.
pub struct Package<'a> {
    zip: ZipPackage<'a>,
    content_types: ContentTypes,
    rels_cache: HashMap<String, IndexMap<String, Relationship>>,
    text_cache: HashMap<String, String>,
    format: AllowedFormat,
}

impl<'a> Package<'a> {
    /// Open bytes as an allow-listed Office Open XML package with default limits.
    pub fn open(bytes: &'a [u8]) -> Result<Self, OpcError> {
        Self::open_with_options(bytes, PackageOpenOptions::default())
    }

    /// Open with explicit zip / security limits.
    pub fn open_with_options(
        bytes: &'a [u8],
        options: PackageOpenOptions,
    ) -> Result<Self, OpcError> {
        if let Some(detected) = sniff_unsupported_container(bytes) {
            return Err(OpcError::unsupported(detected));
        }
        let mut zip = ZipPackage::open_with_limits(bytes, options.zip)?;
        let ct_xml = Self::read_text_uncached(&mut zip, "[Content_Types].xml")?;
        let content_types = parse_content_types(ct_xml.as_deref())?;

        let mut pkg = Self {
            zip,
            content_types,
            rels_cache: HashMap::new(),
            text_cache: HashMap::new(),
            format: AllowedFormat::Xlsx, // replaced below
        };
        if let Some(xml) = ct_xml {
            pkg.text_cache.insert("[Content_Types].xml".to_owned(), xml);
        }

        let main = pkg
            .target_of_type("", relationship_types::OFFICE_DOCUMENT)?
            .or_else(|| {
                // Fallbacks matching upstream workbookPart / common layouts when rels are odd.
                for candidate in [
                    "xl/workbook.xml",
                    "word/document.xml",
                    "ppt/presentation.xml",
                ] {
                    if pkg.has(candidate) {
                        return Some(candidate.to_owned());
                    }
                }
                None
            });
        pkg.format = format_from_package(&pkg.content_types, main.as_deref())?;
        Ok(pkg)
    }

    /// Detected allow-listed format (main-part content type).
    pub fn format(&self) -> AllowedFormat {
        self.format
    }

    /// Parsed content types.
    pub fn content_types(&self) -> &ContentTypes {
        &self.content_types
    }

    /// Content type of a part (override or default extension).
    pub fn content_type(&self, part_name: &str) -> Option<&str> {
        content_type_for_part(&self.content_types, part_name)
    }

    /// Normalised part names in archive order.
    pub fn part_names(&self) -> impl Iterator<Item = &str> {
        self.zip.part_names()
    }

    /// Whether a part exists (case-insensitive OPC lookup).
    pub fn has(&self, part_name: &str) -> bool {
        self.zip.actual(part_name).is_some()
    }

    /// Stored name of a part (exact then case-insensitive).
    pub fn actual(&self, part_name: &str) -> Option<&str> {
        self.zip.actual(part_name)
    }

    /// Inflate a part's bytes.
    pub fn read(&mut self, part_name: &str) -> Result<Option<Vec<u8>>, OpcError> {
        Ok(self.zip.read(part_name)?)
    }

    fn read_text_uncached(
        zip: &mut ZipPackage<'_>,
        part_name: &str,
    ) -> Result<Option<String>, OpcError> {
        let Some(bytes) = zip.read(part_name)? else {
            return Ok(None);
        };
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        if text.starts_with('\u{feff}') {
            text = text['\u{feff}'.len_utf8()..].to_owned();
        }
        Ok(Some(text))
    }

    /// A part decoded as UTF-8 text (BOM stripped), or `None` when absent.
    pub fn text(&mut self, part_name: &str) -> Result<Option<&str>, OpcError> {
        if self.text_cache.contains_key(part_name) {
            return Ok(self.text_cache.get(part_name).map(String::as_str));
        }
        let Some(text) = Self::read_text_uncached(&mut self.zip, part_name)? else {
            return Ok(None);
        };
        self.text_cache.insert(part_name.to_owned(), text);
        Ok(self.text_cache.get(part_name).map(String::as_str))
    }

    /// Relationships declared by `part_name` (`""` for the package root).
    pub fn rels(&mut self, part_name: &str) -> Result<&IndexMap<String, Relationship>, OpcError> {
        let rels_part = if part_name.is_empty() {
            "_rels/.rels".to_owned()
        } else {
            relationships_part_for(part_name)
        };
        if !self.rels_cache.contains_key(&rels_part) {
            let xml = self.text(&rels_part)?.map(str::to_owned);
            let map = parse_relationships(xml.as_deref())?;
            self.rels_cache.insert(rels_part.clone(), map);
        }
        Ok(self.rels_cache.get(&rels_part).expect("just inserted"))
    }

    /// The part a relationship points at, or `None` for external targets.
    pub fn target(&self, part_name: &str, rel: &Relationship) -> Option<String> {
        if rel.mode == RelMode::External {
            return None;
        }
        let base = if part_name.is_empty() { "/" } else { part_name };
        let resolved = resolve_part_path(base, &rel.target);
        Some(
            self.zip
                .actual(&resolved)
                .unwrap_or(resolved.as_str())
                .to_owned(),
        )
    }

    /// The first internal target of a relationship type declared by `part_name`.
    pub fn target_of_type(
        &mut self,
        part_name: &str,
        type_: &str,
    ) -> Result<Option<String>, OpcError> {
        let rels: Vec<Relationship> = self.rels(part_name)?.values().cloned().collect();
        for rel in rels {
            if rel.type_ == type_ {
                return Ok(self.target(part_name, &rel));
            }
        }
        Ok(None)
    }

    /// Signature XML part paths (`_xmlsignatures/*.xml`), listed only.
    pub fn signature_parts(&self) -> Vec<&str> {
        detect_digital_signatures(self.zip.part_names())
    }

    /// All signature-related entry paths (including `.sigs` / `.rels`).
    pub fn signature_related_parts(&self) -> Vec<&str> {
        self.zip
            .part_names()
            .filter(|p| is_signature_part(p))
            .collect()
    }

    /// Core / app / custom property part paths from package relationships.
    pub fn property_parts(&mut self) -> Result<PropertyParts, OpcError> {
        Ok(PropertyParts {
            core: self.target_of_type("", relationship_types::CORE_PROPERTIES)?,
            app: self.target_of_type("", relationship_types::EXTENDED_PROPERTIES)?,
            custom: self.target_of_type("", relationship_types::CUSTOM_PROPERTIES)?,
        })
    }

    /// Diagnostic JSON for the part graph (`inspectPackage` / opc_dump).
    pub fn inspect_json(&mut self) -> Result<String, OpcError> {
        use serde_json::{json, Map, Value};

        let mut parts: Vec<String> = self.part_names().map(str::to_owned).collect();
        parts.sort();
        let defaults: Map<String, Value> = self
            .content_types
            .defaults
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect();
        let overrides: Map<String, Value> = self
            .content_types
            .overrides
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect();

        // Collect every part that has a sibling `.rels` (plus package root).
        let mut rel_owners: Vec<String> = vec![String::new()];
        for name in &parts {
            if name.ends_with(".rels") {
                continue;
            }
            let rels_name = relationships_part_for(name);
            if self.has(&rels_name) {
                rel_owners.push(name.clone());
            }
        }
        rel_owners.sort();
        rel_owners.dedup();

        let mut rels_out = Map::new();
        let mut targets_out = Map::new();
        for owner in &rel_owners {
            let key = if owner.is_empty() {
                "".to_owned()
            } else {
                owner.clone()
            };
            let rels = self.rels(owner)?.clone();
            let mut rel_map = Map::new();
            let mut type_targets = Map::new();
            for (id, rel) in &rels {
                rel_map.insert(
                    id.clone(),
                    json!({
                        "id": rel.id,
                        "type": rel.type_,
                        "target": rel.target,
                        "mode": rel.mode.as_str(),
                    }),
                );
                if let Some(resolved) = self.target(owner, rel) {
                    // First target wins per type (target_of_type semantics).
                    type_targets
                        .entry(rel.type_.clone())
                        .or_insert(Value::String(resolved));
                }
            }
            rels_out.insert(key.clone(), Value::Object(rel_map));
            targets_out.insert(key, Value::Object(type_targets));
        }

        let mut signatures: Vec<String> = self
            .signature_parts()
            .into_iter()
            .map(str::to_owned)
            .collect();
        signatures.sort();
        let props = self.property_parts()?;

        Ok(json!({
            "format": self.format.as_str(),
            "parts": parts,
            "defaults": defaults,
            "overrides": overrides,
            "rels": rels_out,
            "targets": targets_out,
            "signatures": signatures,
            "properties": {
                "core": props.core,
                "app": props.app,
                "custom": props.custom,
            },
        })
        .to_string())
    }
}

#[cfg(test)]
mod tests {
    use crate::opc::relationships::resolve_part_path;

    // Catches: absolute package targets keeping the leading slash, or `..` failing to leave the
    // declaring folder (zip-slip / wrong part wiring).
    #[test]
    fn resolve_part_path_absolute_and_parent_segments() {
        assert_eq!(
            resolve_part_path("word/document.xml", "/docProps/core.xml"),
            "docProps/core.xml"
        );
        assert_eq!(
            resolve_part_path("word/document.xml", "../customXml/item1.xml"),
            "customXml/item1.xml"
        );
        assert_eq!(
            resolve_part_path("ppt/slides/slide1.xml", "../../evil.xml"),
            "evil.xml"
        );
        // More `..` than depth collapses to a root-relative name (upstream pops until empty).
        assert_eq!(
            resolve_part_path("a/b/c.xml", "../../../../outside.txt"),
            "outside.txt"
        );
    }
}
