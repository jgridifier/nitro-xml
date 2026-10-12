// SPDX-License-Identifier: MIT
//! Typed errors for the shared edit core (#63).

use std::fmt;

/// Why a command, transaction or edit script failed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditError {
    /// Command id is known to the vocabulary but has no implementation yet (#66+).
    Unsupported { op: String },
    /// `op` string is not in `edits/schema.json`.
    UnknownOp { op: String },
    /// Script JSON failed to parse or failed shape checks.
    InvalidScript { message: String },
    /// Transaction rolled back after a handler error.
    Rollback { label: String, cause: String },
    /// Undo/redo requested while a transaction is open (upstream throws).
    NestedUndo,
    /// Registry / host / internal failure.
    Other { message: String },
}

impl EditError {
    /// Stable machine-readable tag (gate / WASM `code`).
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Unsupported { .. } => "unsupported",
            Self::UnknownOp { .. } => "unknown_op",
            Self::InvalidScript { .. } => "invalid_edit_script",
            Self::Rollback { .. } => "rollback",
            Self::NestedUndo => "nested_undo",
            Self::Other { .. } => "edit_error",
        }
    }

    pub fn unsupported(op: impl Into<String>) -> Self {
        Self::Unsupported { op: op.into() }
    }

    pub fn invalid_script(message: impl Into<String>) -> Self {
        Self::InvalidScript {
            message: message.into(),
        }
    }
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { op } => write!(f, "unsupported edit op `{op}`"),
            Self::UnknownOp { op } => write!(f, "unknown edit op `{op}`"),
            Self::InvalidScript { message } => write!(f, "invalid edit script: {message}"),
            Self::Rollback { label, cause } => {
                write!(f, "rolled back `{label}`: {cause}")
            }
            Self::NestedUndo => write!(f, "undo/redo while a transaction is open"),
            Self::Other { message } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for EditError {}
