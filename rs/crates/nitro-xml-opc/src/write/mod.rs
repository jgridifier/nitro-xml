// SPDX-License-Identifier: MIT
//! Lossless OPC package writer (#65 / ADR 0003).
//!
//! Strategy (stricter than upstream TS):
//! - No dirty/removed parts → return the **original package bytes** (docx-style identity).
//! - Otherwise rebuild in central-directory order: `raw_copy` untouched entries (same
//!   compression method and compressed payload), rewrite dirty parts, skip removed ones.
//! - VBA / unknown parts / signatures stay carried unless explicitly removed.
//!
//! Format crates (#68+) feed dirty maps; this module owns only the OPC zip layer.

mod content_types;
mod error;

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Write};

use indexmap::IndexMap;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub use content_types::build_content_types_xml;
pub use error::WriteError;

use crate::opc::{content_type_for_part, parse_content_types, ContentTypes};

const CONTENT_TYPES_NAME: &str = "[Content_Types].xml";

/// How a dirty part is compressed when rewritten.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompressionChoice {
    /// Store uncompressed (small XML / already compressed media).
    Stored,
    /// Deflate (default for regenerated XML).
    #[default]
    Deflated,
}

/// A part payload to write (inflated bytes).
#[derive(Debug, Clone)]
pub struct PartWrite {
    pub data: Vec<u8>,
    pub content_type: Option<String>,
    pub method: CompressionChoice,
}

impl PartWrite {
    pub fn bytes(data: impl Into<Vec<u8>>) -> Self {
        Self {
            data: data.into(),
            content_type: None,
            method: CompressionChoice::Deflated,
        }
    }

    pub fn with_content_type(mut self, ct: impl Into<String>) -> Self {
        self.content_type = Some(ct.into());
        self
    }

    pub fn with_method(mut self, method: CompressionChoice) -> Self {
        self.method = method;
        self
    }
}

/// Collects dirty overrides then finishes into a package zip.
pub struct PackageWriter<'src> {
    original: &'src [u8],
    /// Output name → dirty payload.
    dirty: IndexMap<String, PartWrite>,
    /// Parts to drop from the rebuild.
    removed: HashSet<String>,
    /// Optional rename when carrying (src → as). Empty = identity names.
    rename: HashMap<String, String>,
    /// When true, rewrite `[Content_Types].xml` from merged type maps.
    rewrite_content_types: bool,
    /// Merged content types (starts from source; dirty overrides applied on finish).
    content_types: ContentTypes,
}

impl<'src> PackageWriter<'src> {
    /// Open a writer over the original package bytes.
    pub fn open(original: &'src [u8]) -> Result<Self, WriteError> {
        let mut archive = ZipArchive::new(Cursor::new(original))?;
        // Soft-parse content types so a no-op save can still return original bytes
        // when [Content_Types].xml is missing or malformed.
        let ct_xml = read_part_text(&mut archive, CONTENT_TYPES_NAME)
            .ok()
            .flatten();
        let content_types = parse_content_types(ct_xml.as_deref()).unwrap_or_default();
        Ok(Self {
            original,
            dirty: IndexMap::new(),
            removed: HashSet::new(),
            rename: HashMap::new(),
            rewrite_content_types: false,
            content_types,
        })
    }

    /// Whether any dirty or removed mutations are pending.
    pub fn is_noop(&self) -> bool {
        self.dirty.is_empty() && self.removed.is_empty() && self.rename.is_empty()
    }

    /// Add or replace a part (marks it dirty; clears carry for that name).
    pub fn add(&mut self, name: impl Into<String>, part: PartWrite) {
        let name = normalize_name(name.into());
        if let Some(ct) = part.content_type.as_deref() {
            let key = override_key(&name);
            let changed = self.content_types.overrides.get(&key).map(String::as_str) != Some(ct);
            self.content_types.overrides.insert(key, ct.to_owned());
            // Only regenerate [Content_Types].xml when the type map actually changes.
            if changed {
                self.rewrite_content_types = true;
            }
        }
        self.removed.remove(&name);
        self.dirty.insert(name, part);
    }

    /// Convenience: add UTF-8 XML/text as deflated bytes.
    pub fn add_text(
        &mut self,
        name: impl Into<String>,
        text: impl AsRef<str>,
        content_type: Option<&str>,
    ) {
        let mut part = PartWrite::bytes(text.as_ref().as_bytes().to_vec());
        if let Some(ct) = content_type {
            part = part.with_content_type(ct);
        }
        self.add(name, part);
    }

    /// Replace an existing part's bytes, keeping any known content type.
    pub fn replace(&mut self, name: &str, data: Vec<u8>) {
        let name = normalize_name(name);
        let ct = content_type_for_part(&self.content_types, &name).map(str::to_owned);
        let mut part = PartWrite::bytes(data);
        if let Some(ct) = ct {
            part = part.with_content_type(ct);
        }
        self.add(name, part);
    }

    /// Drop a part from the output package.
    pub fn remove(&mut self, name: &str) {
        let name = normalize_name(name);
        self.dirty.shift_remove(&name);
        self.removed.insert(name);
    }

    /// Carry `src` under output name `as_name` (default: same name).
    /// With no-op finish this is a no-op; on rebuild, the entry is raw-copied (or renamed).
    pub fn carry_as(&mut self, src: &str, as_name: &str) {
        let src = normalize_name(src);
        let as_name = normalize_name(as_name);
        self.removed.remove(&as_name);
        if src != as_name {
            self.rename.insert(src, as_name);
        }
    }

    pub fn carry(&mut self, name: &str) {
        self.carry_as(name, name);
    }

    /// Force regenerating `[Content_Types].xml` from the merged type map.
    pub fn touch_content_types(&mut self) {
        self.rewrite_content_types = true;
    }

    /// Borrow the merged content-types map (for format writers).
    pub fn content_types(&self) -> &ContentTypes {
        &self.content_types
    }

    pub fn content_types_mut(&mut self) -> &mut ContentTypes {
        self.rewrite_content_types = true;
        &mut self.content_types
    }

    /// Finish: original bytes when unchanged; otherwise CD-order rebuild with raw carry.
    pub fn finish(self) -> Result<Vec<u8>, WriteError> {
        if self.is_noop() && !self.rewrite_content_types {
            return Ok(self.original.to_vec());
        }
        self.rebuild()
    }

    fn rebuild(self) -> Result<Vec<u8>, WriteError> {
        let mut source = ZipArchive::new(Cursor::new(self.original))?;
        let cursor = Cursor::new(Vec::with_capacity(self.original.len()));
        let mut writer = ZipWriter::new(cursor);

        let mut emitted: HashSet<String> = HashSet::new();
        let rewrite_ct = self.rewrite_content_types
            || self.dirty.contains_key(CONTENT_TYPES_NAME)
            || self.removed.contains(CONTENT_TYPES_NAME);

        // Emit source entries in central-directory / archive order.
        let count = source.len();
        for index in 0..count {
            let file = source.by_index_raw(index)?;
            if file.is_dir() {
                continue;
            }
            let raw_name = file.name().map_err(|e| WriteError::Encode(e.to_string()))?;
            let src_name = normalize_name(raw_name);
            if self.removed.contains(&src_name) {
                continue;
            }
            let out_name = self
                .rename
                .get(&src_name)
                .cloned()
                .unwrap_or_else(|| src_name.clone());

            if out_name == CONTENT_TYPES_NAME && rewrite_ct {
                // Keep CT at its original index so zip entry order stays stable.
                let ct_part = content_types_part(&self.dirty, &self.content_types);
                write_part(&mut writer, CONTENT_TYPES_NAME, &ct_part)?;
                emitted.insert(out_name);
                continue;
            }

            if let Some(part) = self.dirty.get(&out_name) {
                write_part(&mut writer, &out_name, part)?;
                emitted.insert(out_name);
                continue;
            }

            // Raw-copy preserves compression method and compressed payload.
            if out_name != src_name {
                writer.raw_copy_file_rename(file, out_name.clone())?;
            } else {
                writer.raw_copy_file(file)?;
            }
            emitted.insert(out_name);
        }

        // New dirty parts not present in the source (append after carried entries).
        for (name, part) in &self.dirty {
            if emitted.contains(name) {
                continue;
            }
            if name == CONTENT_TYPES_NAME {
                continue;
            }
            write_part(&mut writer, name, part)?;
            emitted.insert(name.clone());
        }

        if rewrite_ct && !emitted.contains(CONTENT_TYPES_NAME) {
            let ct_part = content_types_part(&self.dirty, &self.content_types);
            write_part(&mut writer, CONTENT_TYPES_NAME, &ct_part)?;
        }

        let cursor = writer.finish()?;
        Ok(cursor.into_inner())
    }
}

fn content_types_part(dirty: &IndexMap<String, PartWrite>, types: &ContentTypes) -> PartWrite {
    let xml = if let Some(part) = dirty.get(CONTENT_TYPES_NAME) {
        part.data.clone()
    } else {
        build_content_types_xml(types).into_bytes()
    };
    PartWrite::bytes(xml)
        .with_content_type("application/vnd.openxmlformats-package.contenttypes+xml")
        .with_method(CompressionChoice::Deflated)
}

fn write_part<W: Write + std::io::Seek>(
    writer: &mut ZipWriter<W>,
    name: &str,
    part: &PartWrite,
) -> Result<(), WriteError> {
    let method = match part.method {
        CompressionChoice::Stored => CompressionMethod::Stored,
        CompressionChoice::Deflated => CompressionMethod::Deflated,
    };
    let options = SimpleFileOptions::default()
        .compression_method(method)
        .last_modified_time(
            zip::DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0).unwrap_or_default(),
        );
    writer.start_file(name, options)?;
    writer.write_all(&part.data)?;
    Ok(())
}

fn normalize_name(name: impl AsRef<str>) -> String {
    name.as_ref().trim_start_matches('/').to_owned()
}

fn override_key(part_name: &str) -> String {
    if part_name.starts_with('/') {
        part_name.to_owned()
    } else {
        format!("/{part_name}")
    }
}

fn read_part_text(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    name: &str,
) -> Result<Option<String>, WriteError> {
    let Ok(mut file) = archive.by_name(name) else {
        // Case-insensitive fallback
        let lower = name.to_ascii_lowercase();
        let mut found = None;
        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            if file.is_dir() {
                continue;
            }
            let raw = file.name().map_err(|e| WriteError::Encode(e.to_string()))?;
            let n = raw.trim_start_matches('/');
            if n.to_ascii_lowercase() == lower {
                found = Some(i);
                break;
            }
        }
        let Some(i) = found else {
            return Ok(None);
        };
        let mut file = archive.by_index(i)?;
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut buf)?;
        let mut text = String::from_utf8_lossy(&buf).into_owned();
        if text.starts_with('\u{feff}') {
            text = text['\u{feff}'.len_utf8()..].to_owned();
        }
        return Ok(Some(text));
    };
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut file, &mut buf)?;
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    if text.starts_with('\u{feff}') {
        text = text['\u{feff}'.len_utf8()..].to_owned();
    }
    Ok(Some(text))
}

/// No-op save: no edits → original package bytes (even if the zip is corrupt).
///
/// Prefer this over `PackageWriter::open` + `finish` when the caller knows the
/// model is untouched; opening is unnecessary and would reject non-zip inputs
/// that still deserve an identity round-trip.
pub fn noop_save(original: &[u8]) -> Result<Vec<u8>, WriteError> {
    Ok(original.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::ZipPackage;

    fn stored_zip(parts: &[(&str, &[u8])]) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut cursor);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            for (name, data) in parts {
                writer.start_file(*name, opts).unwrap();
                writer.write_all(data).unwrap();
            }
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }

    // Catches: no-op save rebuilding the zip and losing byte identity vs the original package.
    #[test]
    fn noop_save_returns_original_bytes() {
        let original = stored_zip(&[
            (
                "[Content_Types].xml",
                br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#,
            ),
            ("xl/workbook.xml", b"<workbook/>"),
        ]);
        let out = noop_save(&original).expect("noop");
        assert_eq!(out, original);
    }

    // Catches: dirty rewrite dropping an untouched sibling part (carry regression).
    #[test]
    fn dirty_replace_keeps_other_parts() {
        let original = stored_zip(&[
            (
                "[Content_Types].xml",
                br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>"#,
            ),
            ("xl/workbook.xml", b"<workbook/>"),
            ("xl/worksheets/sheet1.xml", b"<worksheet/>"),
        ]);
        let mut writer = PackageWriter::open(&original).unwrap();
        writer.replace("xl/workbook.xml", b"<workbook edited='1'/>".to_vec());
        let out = writer.finish().unwrap();
        let mut pkg = ZipPackage::open(&out).unwrap();
        assert_eq!(
            pkg.read("xl/worksheets/sheet1.xml").unwrap(),
            Some(b"<worksheet/>".to_vec())
        );
        assert_eq!(
            pkg.read("xl/workbook.xml").unwrap(),
            Some(b"<workbook edited='1'/>".to_vec())
        );
    }

    // Catches: dirty rebuild regenerating [Content_Types].xml when the type map is unchanged.
    #[test]
    fn replace_same_content_type_keeps_content_types_bytes() {
        let ct = br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>"#;
        let original = stored_zip(&[
            ("[Content_Types].xml", ct.as_slice()),
            ("xl/workbook.xml", b"<workbook/>"),
            ("xl/worksheets/sheet1.xml", b"<worksheet/>"),
        ]);
        let mut writer = PackageWriter::open(&original).unwrap();
        writer.replace("xl/workbook.xml", b"<workbook edited='1'/>".to_vec());
        let out = writer.finish().unwrap();
        let mut pkg = ZipPackage::open(&out).unwrap();
        assert_eq!(pkg.read("[Content_Types].xml").unwrap(), Some(ct.to_vec()));
    }
}
