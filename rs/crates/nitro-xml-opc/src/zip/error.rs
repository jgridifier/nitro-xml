// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/package.ts (readZipParts, SourceIndex limits); changes listed in PROVENANCE.md.
//! Typed errors for the bounded zip reader.

use std::fmt;

/// Why a package could not be read. `kind()` gives a stable machine-readable tag.
#[derive(Debug)]
#[non_exhaustive]
pub enum ZipError {
    /// The compressed input is larger than the input limit (checked before parsing).
    InputTooLarge,
    /// The zip container could not be opened, parsed or inflated.
    InvalidArchive(String),
    /// More non-directory entries than the part limit.
    TooManyParts,
    /// An entry declares an uncompressed size above the per-part limit.
    PartTooLarge { name: String },
    /// The declared or inflated total is above the package limit.
    TotalTooLarge,
    /// An entry's declared or inflated size exceeds the compression-ratio cap (zip-bomb).
    CompressionRatio { name: String },
    /// An entry inflated to a size other than its declared size.
    SizeMismatch { name: String },
}

impl ZipError {
    /// A stable tag for the error kind, e.g. `"too_many_parts"`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::InputTooLarge => "input_too_large",
            Self::InvalidArchive(_) => "invalid_archive",
            Self::TooManyParts => "too_many_parts",
            Self::PartTooLarge { .. } => "part_too_large",
            Self::TotalTooLarge => "total_too_large",
            Self::CompressionRatio { .. } => "compression_ratio",
            Self::SizeMismatch { .. } => "size_mismatch",
        }
    }
}

impl fmt::Display for ZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooLarge => write!(f, "package exceeds the compressed input limit"),
            Self::InvalidArchive(msg) => write!(f, "not a valid zip package: {msg}"),
            Self::TooManyParts => write!(f, "package exceeds the part limit"),
            Self::PartTooLarge { name } => {
                write!(f, "part {name} exceeds the uncompressed part size limit")
            }
            Self::TotalTooLarge => write!(f, "package exceeds the uncompressed content limit"),
            Self::CompressionRatio { name } => {
                write!(
                    f,
                    "part {name} exceeds the compression-ratio limit (zip-bomb guard)"
                )
            }
            Self::SizeMismatch { name } => {
                write!(f, "part {name} does not inflate to its declared size")
            }
        }
    }
}

impl std::error::Error for ZipError {}
