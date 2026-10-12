// SPDX-License-Identifier: MIT
//! Lossless OPC package writer (#65 / ADR 0003).
//!
//! Strategy (stricter than upstream TS):
//! - **Identity shortcut:** when `finish` sees no dirty/removed/rename and content-types
//!   were not touched, it returns the **original package bytes** without rebuilding the
//!   zip (docx-style). [`noop_save`] is that shortcut. A no-edit round-trip that is
//!   byte-identical therefore does **not** by itself prove the rebuild/carry path.
//! - **Rebuild path:** any dirty/removed part (see [`touch_save`]) rebuilds in
//!   central-directory order: `raw_copy` untouched entries (same compression method and
//!   compressed payload), rewrite dirty parts, skip removed ones.
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

use crate::opc::{parse_content_types, ContentTypes};

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
    /// Test-only: inflate+recompress these names instead of `raw_copy` (breaks CD identity).
    /// Used by the #65 negative control; production callers leave this empty.
    corrupt_carry: HashSet<String>,
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
            corrupt_carry: HashSet::new(),
        })
    }

    /// Whether any dirty or removed mutations are pending.
    pub fn is_noop(&self) -> bool {
        self.dirty.is_empty()
            && self.removed.is_empty()
            && self.rename.is_empty()
            && self.corrupt_carry.is_empty()
    }

    /// Test-only: force a carried part through inflate+recompress (not `raw_copy`).
    ///
    /// The #65 negative control uses this to prove the harness detects broken carry
    /// (flipped compression / changed CRC). Do not call from production save paths.
    pub fn debug_corrupt_carry(&mut self, name: &str) {
        self.corrupt_carry.insert(normalize_name(name));
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

    /// Replace an existing part's bytes without touching the content-types map.
    ///
    /// Attaching a content type here would insert an Override and force a
    /// `[Content_Types].xml` rewrite; use [`Self::add`] with
    /// [`PartWrite::with_content_type`] when the type map must change.
    pub fn replace(&mut self, name: &str, data: Vec<u8>) {
        let name = normalize_name(name);
        self.add(name, PartWrite::bytes(data));
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

    /// Finish: **identity shortcut** when unchanged (returns `original` bytes without
    /// rebuilding); otherwise CD-order rebuild with raw carry.
    pub fn finish(self) -> Result<Vec<u8>, WriteError> {
        if self.is_noop() && !self.rewrite_content_types {
            // IDENTITY SHORTCUT — not a zip rebuild. See module docs and [`touch_save`].
            return Ok(self.original.to_vec());
        }
        self.rebuild()
    }

    /// Force the rebuild path even when nothing is dirty (still rewrite every entry
    /// via `raw_copy`). Prefer [`touch_save`] for a meaningful dirty part.
    pub fn finish_rebuild(self) -> Result<Vec<u8>, WriteError> {
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

            // Negative-control path: inflate + recompress with flipped method (not raw_copy).
            if self.corrupt_carry.contains(&out_name) || self.corrupt_carry.contains(&src_name) {
                let orig_method = file.compression();
                drop(file);
                let mut inflated = source.by_name(&src_name)?;
                let mut data = Vec::new();
                std::io::Read::read_to_end(&mut inflated, &mut data)?;
                drop(inflated);
                let flipped = match orig_method {
                    CompressionMethod::Stored => CompressionChoice::Deflated,
                    _ => CompressionChoice::Stored,
                };
                let part = PartWrite::bytes(data).with_method(flipped);
                write_part(&mut writer, &out_name, &part)?;
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

/// No-op save via the **identity shortcut**: returns `original` bytes unchanged.
///
/// This does **not** open a zip or call [`PackageWriter::rebuild`]. It exists so a
/// true no-edit save stays byte-identical even for corrupt/encrypted inputs.
/// Use [`touch_save`] (or any dirty [`PackageWriter::finish`]) to exercise the
/// real rebuild + `raw_copy` carry path.
pub fn noop_save(original: &[u8]) -> Result<Vec<u8>, WriteError> {
    Ok(original.to_vec())
}

/// Result of [`touch_save`]: rebuilt package bytes and the part that was dirtied.
#[derive(Debug, Clone)]
pub struct TouchSave {
    pub bytes: Vec<u8>,
    pub dirty_part: String,
}

/// Mark one existing part dirty (rewrite same inflated bytes through `write_part`)
/// and finish via the **rebuild** path — proves carry/`raw_copy` for every other part.
///
/// Part selection (first hit wins): `docProps/core.xml`, format main part
/// (`xl/workbook.xml` / `word/document.xml` / `ppt/presentation.xml`), else the
/// first non-`[Content_Types]` XML part.
pub fn touch_save(original: &[u8]) -> Result<TouchSave, WriteError> {
    touch_save_with_options(original, None, None)
}

/// Like [`touch_save`], with optional forced dirty part and optional corrupt-carry
/// target (negative control).
pub fn touch_save_with_options(
    original: &[u8],
    dirty_part: Option<&str>,
    corrupt_carry: Option<&str>,
) -> Result<TouchSave, WriteError> {
    let mut writer = PackageWriter::open(original)?;
    let part_name = match dirty_part {
        Some(name) => normalize_name(name),
        None => pick_touch_part(original)?,
    };
    let data = read_part_bytes(original, &part_name)?.ok_or_else(|| WriteError::MissingPart {
        name: part_name.clone(),
    })?;
    writer.replace(&part_name, data);
    if let Some(name) = corrupt_carry {
        writer.debug_corrupt_carry(name);
    }
    let bytes = writer.finish()?;
    Ok(TouchSave {
        bytes,
        dirty_part: part_name,
    })
}

fn pick_touch_part(original: &[u8]) -> Result<String, WriteError> {
    let mut archive = ZipArchive::new(Cursor::new(original))?;
    let preferred = [
        "docProps/core.xml",
        "xl/workbook.xml",
        "word/document.xml",
        "ppt/presentation.xml",
        "xl/worksheets/sheet1.xml",
        "ppt/slides/slide1.xml",
    ];
    for name in preferred {
        if archive.by_name(name).is_ok() {
            return Ok(name.to_owned());
        }
    }
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        if file.is_dir() {
            continue;
        }
        let raw = file.name().map_err(|e| WriteError::Encode(e.to_string()))?;
        let name = normalize_name(raw);
        if name.eq_ignore_ascii_case(CONTENT_TYPES_NAME) {
            continue;
        }
        if name.ends_with(".xml") || name.ends_with(".rels") {
            return Ok(name);
        }
    }
    Err(WriteError::MissingPart {
        name: "touch-candidate".into(),
    })
}

fn read_part_bytes(original: &[u8], name: &str) -> Result<Option<Vec<u8>>, WriteError> {
    let mut archive = ZipArchive::new(Cursor::new(original))?;
    let Ok(mut file) = archive.by_name(name) else {
        return Ok(None);
    };
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut file, &mut buf)?;
    Ok(Some(buf))
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

    // Catches: touch_save taking the identity shortcut (output == input) instead of rebuilding.
    #[test]
    fn touch_save_rebuilds_not_identity_shortcut() {
        let original = stored_zip(&[
            (
                "[Content_Types].xml",
                br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#,
            ),
            ("docProps/core.xml", b"<cp:coreProperties/>"),
            ("xl/workbook.xml", b"<workbook/>"),
            ("xl/worksheets/sheet1.xml", b"<worksheet/>"),
        ]);
        let touched = touch_save(&original).expect("touch");
        assert_ne!(
            touched.bytes, original,
            "touch must rebuild (zip metadata/date changes) not return original bytes"
        );
        assert_eq!(touched.dirty_part, "docProps/core.xml");
        let mut pkg = ZipPackage::open(&touched.bytes).unwrap();
        assert_eq!(
            pkg.read("xl/worksheets/sheet1.xml").unwrap(),
            Some(b"<worksheet/>".to_vec())
        );
        assert_eq!(
            pkg.read("docProps/core.xml").unwrap(),
            Some(b"<cp:coreProperties/>".to_vec())
        );
    }

    // Catches: corrupt_carry leaving compression method unchanged (negative control silent).
    #[test]
    fn corrupt_carry_flips_compression_method() {
        let original = stored_zip(&[
            (
                "[Content_Types].xml",
                br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#,
            ),
            ("docProps/core.xml", b"<cp:coreProperties/>"),
            ("xl/worksheets/sheet1.xml", b"<worksheet pad='xxxxxxxxxxxxxxxx'/>"),
        ]);
        let good = touch_save(&original).unwrap();
        let bad = touch_save_with_options(
            &original,
            Some("docProps/core.xml"),
            Some("xl/worksheets/sheet1.xml"),
        )
        .unwrap();
        assert_ne!(good.bytes, bad.bytes);
        let mut src = ZipArchive::new(Cursor::new(&original)).unwrap();
        let mut out = ZipArchive::new(Cursor::new(&bad.bytes)).unwrap();
        let m_in = src
            .by_name("xl/worksheets/sheet1.xml")
            .unwrap()
            .compression();
        let m_out = out
            .by_name("xl/worksheets/sheet1.xml")
            .unwrap()
            .compression();
        assert_ne!(m_in, m_out, "corrupt carry must flip compression");
    }
}
