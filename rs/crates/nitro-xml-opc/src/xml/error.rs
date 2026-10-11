// SPDX-License-Identifier: MIT
//! Typed errors for the streaming XML layer.

use std::fmt;

/// Failure while parsing an XML part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlError {
    kind: &'static str,
    detail: String,
}

impl XmlError {
    pub(crate) fn new(kind: &'static str, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    /// Short machine-readable kind for gate / dump output.
    pub fn kind(&self) -> &'static str {
        self.kind
    }

    /// Human-readable detail.
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for XmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for XmlError {}
