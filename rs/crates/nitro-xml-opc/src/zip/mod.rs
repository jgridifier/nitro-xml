// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/package.ts (readZipParts, SourceIndex limits); changes listed in PROVENANCE.md.
//! Bounded zip reader. Upstream reads packages through JSZip (`readZipParts` in
//! `src/core/xlsx/read/package.ts`); this module keeps its limits and case-insensitive part
//! lookup over the `zip` crate.
//!
//! - Limits: compressed input size, non-directory entry count, declared size per part and declared
//!   total. The declared sizes are checked at open; the real guard counts inflated bytes while
//!   reading, so a header that under-declares its size is rejected before it inflates further.
//! - Lazy inflation: opening reads only the central directory. [`ZipPackage::read`] inflates one
//!   part on demand, counting its bytes against the package total.
//! - Lookup: [`ZipPackage::actual`] matches a name exactly, then case-insensitively, like upstream
//!   `SourceIndex.actual`. Names are stored without one leading `/`.

mod error;

use std::collections::HashMap;
use std::io::{Cursor, Read};

use ::zip::ZipArchive;

pub use error::ZipError;

/// Largest accepted compressed package.
pub const MAX_INPUT_BYTES: u64 = 50 * 1024 * 1024;
/// Largest number of non-directory entries.
pub const MAX_PARTS: usize = 10_000;
/// Largest total of inflated bytes across all parts.
pub const MAX_UNCOMPRESSED_BYTES: u64 = 300 * 1024 * 1024;
/// Largest inflated size of one part.
pub const MAX_PART_BYTES: u64 = 200 * 1024 * 1024;

/// Read chunk size; also the granularity at which limits are enforced while inflating.
const CHUNK_BYTES: usize = 64 * 1024;
/// Upper bound on the up-front allocation for one part, whatever its declared size.
const MAX_PREALLOCATE_BYTES: u64 = 64 * 1024 * 1024;

/// Size limits for [`ZipPackage::open_with_limits`]. `Default` is the exported constants.
#[derive(Debug, Clone, Copy)]
pub struct ZipLimits {
    pub max_input_bytes: u64,
    pub max_parts: usize,
    pub max_part_bytes: u64,
    pub max_total_bytes: u64,
}

impl Default for ZipLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: MAX_INPUT_BYTES,
            max_parts: MAX_PARTS,
            max_part_bytes: MAX_PART_BYTES,
            max_total_bytes: MAX_UNCOMPRESSED_BYTES,
        }
    }
}

/// One normalised, non-directory part.
struct Entry {
    /// Name with one leading `/` removed.
    name: String,
    /// Index in the zip central directory.
    index: usize,
    /// Declared uncompressed size.
    size: u64,
}

/// An opened zip package. Parts are inflated on demand by [`ZipPackage::read`].
pub struct ZipPackage<'a> {
    archive: ZipArchive<Cursor<&'a [u8]>>,
    /// One entry per normalised name, in first-occurrence archive order.
    entries: Vec<Entry>,
    /// Exact name to position in `entries`.
    by_name: HashMap<String, usize>,
    /// Lower-cased name to position in `entries`.
    lower: HashMap<String, usize>,
    limits: ZipLimits,
    /// Bytes inflated by successful `read` calls.
    inflated_total: u64,
}

fn invalid(error: impl std::fmt::Display) -> ZipError {
    ZipError::InvalidArchive(error.to_string())
}

/// The zip crate's "larger than its declared uncompressed size" error (zip 9.0.1, `Crc32Reader`).
fn is_over_declared(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::InvalidData
        && error.to_string().contains("declared uncompressed size")
}

impl<'a> ZipPackage<'a> {
    /// Opens a package with the default limits.
    pub fn open(bytes: &'a [u8]) -> Result<Self, ZipError> {
        Self::open_with_limits(bytes, ZipLimits::default())
    }

    /// Opens a package, checking the input size and the declared sizes. No part is inflated.
    pub fn open_with_limits(bytes: &'a [u8], limits: ZipLimits) -> Result<Self, ZipError> {
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limits.max_input_bytes {
            return Err(ZipError::InputTooLarge);
        }
        let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(invalid)?;

        let mut found: Vec<(String, usize, u64)> = Vec::new();
        for index in 0..archive.len() {
            let file = archive.by_index_raw(index).map_err(invalid)?;
            if file.is_dir() {
                continue;
            }
            let raw_name = file.name().map_err(invalid)?;
            let name = raw_name.strip_prefix('/').unwrap_or(&raw_name).to_owned();
            found.push((name, index, file.size()));
        }

        if found.len() > limits.max_parts {
            return Err(ZipError::TooManyParts);
        }
        let mut declared_total: u64 = 0;
        for (name, _, size) in &found {
            if *size > limits.max_part_bytes {
                return Err(ZipError::PartTooLarge { name: name.clone() });
            }
            declared_total = declared_total.saturating_add(*size);
        }
        if declared_total > limits.max_total_bytes {
            return Err(ZipError::TotalTooLarge);
        }

        let mut entries: Vec<Entry> = Vec::with_capacity(found.len());
        let mut by_name: HashMap<String, usize> = HashMap::with_capacity(found.len());
        let mut lower: HashMap<String, usize> = HashMap::with_capacity(found.len());
        for (name, index, size) in found {
            // A later entry with the same normalised name replaces the earlier one in place.
            let pos = match by_name.get(&name) {
                Some(&pos) => {
                    if let Some(slot) = entries.get_mut(pos) {
                        *slot = Entry {
                            name: name.clone(),
                            index,
                            size,
                        };
                    }
                    pos
                }
                None => {
                    entries.push(Entry {
                        name: name.clone(),
                        index,
                        size,
                    });
                    by_name.insert(name.clone(), entries.len() - 1);
                    entries.len() - 1
                }
            };
            lower.insert(name.to_lowercase(), pos);
        }

        Ok(Self {
            archive,
            entries,
            by_name,
            lower,
            limits,
            inflated_total: 0,
        })
    }

    /// Normalised part names, in archive order.
    pub fn part_names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|entry| entry.name.as_str())
    }

    /// Number of parts.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the package has no parts.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn position(&self, name: &str) -> Option<usize> {
        if let Some(&pos) = self.by_name.get(name) {
            return Some(pos);
        }
        self.lower.get(&name.to_lowercase()).copied()
    }

    /// The stored name of a part: exact match first, then case-insensitive.
    pub fn actual(&self, name: &str) -> Option<&str> {
        let entry = self.entries.get(self.position(name)?)?;
        Some(entry.name.as_str())
    }

    /// The declared uncompressed size of a part, if present.
    pub fn declared_size(&self, name: &str) -> Option<u64> {
        let entry = self.entries.get(self.position(name)?)?;
        Some(entry.size)
    }

    /// Inflates one part. `Ok(None)` if no such part exists. Each successful read counts its bytes
    /// against the package total again, so re-reading a part is not free.
    pub fn read(&mut self, name: &str) -> Result<Option<Vec<u8>>, ZipError> {
        let Some(entry) = self.position(name).and_then(|pos| self.entries.get(pos)) else {
            return Ok(None);
        };
        let (index, declared) = (entry.index, entry.size);
        let max_total = self.limits.max_total_bytes;
        let inflated_total = self.inflated_total;
        let stored_name = entry.name.as_str();

        let mut file = self.archive.by_index(index).map_err(invalid)?;
        let prealloc = usize::try_from(declared.min(MAX_PREALLOCATE_BYTES)).unwrap_or(0);
        let mut out: Vec<u8> = Vec::with_capacity(prealloc);
        let mut buf = vec![0u8; CHUNK_BYTES];
        // One byte past the declared size, so an over-long stream is seen and stopped.
        let mut limited = (&mut file).take(declared.saturating_add(1));
        loop {
            let n = match limited.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                // The zip crate enforces the header's declared size itself (same condition as
                // `take` below) and reports it as an io error; it is a size mismatch, not corruption.
                Err(e) if is_over_declared(&e) => {
                    return Err(ZipError::SizeMismatch {
                        name: stored_name.to_owned(),
                    });
                }
                Err(e) => return Err(invalid(e)),
            };
            let produced = u64::try_from(out.len())
                .unwrap_or(u64::MAX)
                .saturating_add(n as u64);
            if produced > declared {
                return Err(ZipError::SizeMismatch {
                    name: stored_name.to_owned(),
                });
            }
            if inflated_total.saturating_add(produced) > max_total {
                return Err(ZipError::TotalTooLarge);
            }
            if let Some(chunk) = buf.get(..n) {
                out.extend_from_slice(chunk);
            }
        }
        let produced = u64::try_from(out.len()).unwrap_or(u64::MAX);
        if produced != declared {
            return Err(ZipError::SizeMismatch {
                name: stored_name.to_owned(),
            });
        }
        self.inflated_total = inflated_total.saturating_add(produced);
        Ok(Some(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a zip of stored (method 0) entries. Each entry is `(name, data, declared size)`; the
    /// headers record `data.len()` as the compressed size, so `declared` may lie about the content.
    fn stored_zip(parts: &[(&str, &[u8], u32)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, data, declared) in parts {
            let offset = u32::try_from(out.len()).unwrap_or(u32::MAX);
            let crc = crc32fast::hash(data);
            let comp = u32::try_from(data.len()).unwrap_or(u32::MAX);
            let name_len = u16::try_from(name.len()).unwrap_or(u16::MAX);
            out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&[0u8; 2]); // flags
            out.extend_from_slice(&[0u8; 2]); // method: stored
            out.extend_from_slice(&[0u8; 4]); // time, date
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&comp.to_le_bytes());
            out.extend_from_slice(&declared.to_le_bytes());
            out.extend_from_slice(&name_len.to_le_bytes());
            out.extend_from_slice(&[0u8; 2]); // extra length
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(data);

            central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&[0u8; 2]); // flags
            central.extend_from_slice(&[0u8; 2]); // method
            central.extend_from_slice(&[0u8; 4]); // time, date
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&comp.to_le_bytes());
            central.extend_from_slice(&declared.to_le_bytes());
            central.extend_from_slice(&name_len.to_le_bytes());
            central.extend_from_slice(&[0u8; 8]); // extra, comment, disk, internal attr
            central.extend_from_slice(&[0u8; 4]); // external attr
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let cd_offset = u32::try_from(out.len()).unwrap_or(u32::MAX);
        let count = u16::try_from(parts.len()).unwrap_or(u16::MAX);
        let cd_size = u32::try_from(central.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&central);
        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&[0u8; 4]); // disk numbers
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&[0u8; 2]); // comment length
        out
    }

    // Catches: an entry whose headers under-declare its size (zip-bomb style) being inflated past the declared size instead of rejected.
    #[test]
    fn under_declared_part_is_rejected_while_reading() {
        let data = vec![b'x'; 4096];
        let bytes = stored_zip(&[("xl/big.bin", &data, 16)]);
        let mut pkg =
            ZipPackage::open(&bytes).expect("the limit is enforced while reading, not at open");
        assert!(matches!(
            pkg.read("xl/big.bin"),
            Err(ZipError::SizeMismatch { .. })
        ));
    }

    // Catches: part lookup being case-sensitive or keeping the leading '/', unlike SourceIndex.actual.
    #[test]
    fn lookup_strips_leading_slash_and_ignores_case() {
        let bytes = stored_zip(&[
            ("/[Content_Types].xml", b"ct", 2),
            ("XL/", b"", 0),
            ("XL/Workbook.xml", b"wb", 2),
        ]);
        let mut pkg = ZipPackage::open(&bytes).expect("opens");
        let names: Vec<&str> = pkg.part_names().collect();
        assert_eq!(names, ["[Content_Types].xml", "XL/Workbook.xml"]);
        assert_eq!(
            pkg.read("xl/workbook.xml").expect("reads"),
            Some(b"wb".to_vec())
        );
        assert_eq!(pkg.actual("xl/WORKBOOK.xml"), Some("XL/Workbook.xml"));
    }
}
