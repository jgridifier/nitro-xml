// SPDX-License-Identifier: MIT
//! Errors from the lossless OPC package writer (#65).

use crate::opc::OpcError;
use crate::xml::XmlError;
use crate::zip::ZipError;

/// Failure while building or finishing a package write.
#[derive(Debug)]
pub enum WriteError {
    Zip(ZipError),
    Opc(OpcError),
    Xml(XmlError),
    Io(std::io::Error),
    /// A dirty part was requested but the source package could not supply carry metadata.
    MissingPart { name: String },
    /// Zip crate / encoding failure while emitting an entry.
    Encode(String),
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Zip(e) => write!(f, "{e}"),
            Self::Opc(e) => write!(f, "{e}"),
            Self::Xml(e) => write!(f, "{e}"),
            Self::Io(e) => write!(f, "{e}"),
            Self::MissingPart { name } => write!(f, "missing part {name}"),
            Self::Encode(msg) => write!(f, "zip encode: {msg}"),
        }
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Zip(e) => Some(e),
            Self::Opc(e) => Some(e),
            Self::Xml(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ZipError> for WriteError {
    fn from(value: ZipError) -> Self {
        Self::Zip(value)
    }
}

impl From<OpcError> for WriteError {
    fn from(value: OpcError) -> Self {
        Self::Opc(value)
    }
}

impl From<XmlError> for WriteError {
    fn from(value: XmlError) -> Self {
        Self::Xml(value)
    }
}

impl From<std::io::Error> for WriteError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<::zip::result::ZipError> for WriteError {
    fn from(value: ::zip::result::ZipError) -> Self {
        Self::Encode(value.to_string())
    }
}
