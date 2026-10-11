// SPDX-License-Identifier: MIT
//! Typed errors for OPC package open and part reads.

use std::fmt;

use crate::xml::XmlError;
use crate::zip::ZipError;

/// Why an OPC package could not be opened or a part looked up.
#[derive(Debug)]
#[non_exhaustive]
pub enum OpcError {
    /// Zip container / size-limit failure.
    Zip(ZipError),
    /// Malformed content types or relationships XML.
    Xml(XmlError),
    /// Bytes are not an allowed Office Open XML package (PRODUCT_SPEC.md scope).
    UnsupportedFormat {
        /// Coarse detected kind (`ole`, `encrypted`, `csv`, `xlsb`, a main-part content type, …).
        detected: String,
    },
    /// A required OPC part was missing or unreadable.
    MissingPart { name: String },
}

impl OpcError {
    /// Stable machine-readable tag.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Zip(e) => e.kind(),
            Self::Xml(e) => e.kind(),
            Self::UnsupportedFormat { .. } => "unsupported_format",
            Self::MissingPart { .. } => "missing_part",
        }
    }

    pub(crate) fn unsupported(detected: impl Into<String>) -> Self {
        Self::UnsupportedFormat {
            detected: detected.into(),
        }
    }
}

impl fmt::Display for OpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zip(e) => write!(f, "{e}"),
            Self::Xml(e) => write!(f, "{e}"),
            Self::UnsupportedFormat { detected } => {
                write!(f, "unsupported format: {detected}")
            }
            Self::MissingPart { name } => write!(f, "missing package part: {name}"),
        }
    }
}

impl std::error::Error for OpcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Zip(e) => Some(e),
            Self::Xml(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ZipError> for OpcError {
    fn from(value: ZipError) -> Self {
        Self::Zip(value)
    }
}

impl From<XmlError> for OpcError {
    fn from(value: XmlError) -> Self {
        Self::Xml(value)
    }
}
