// SPDX-License-Identifier: MIT
//! Format allow-list for Nitro XML (PRODUCT_SPEC.md): Word/Excel/PowerPoint OOXML packages
//! including macro-enabled and template variants. Everything else is [`UnsupportedFormat`](crate::opc::OpcError::UnsupportedFormat).

use crate::opc::content_types::{content_type_for_part, ContentTypes};
use crate::opc::error::OpcError;

const CFB_MAGIC: [u8; 8] = [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];
const ZIP_LOCAL: [u8; 4] = [0x50, 0x4b, 0x03, 0x04];
const ZIP_EMPTY: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];

/// Allowed package kinds recognised from the main-part content type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowedFormat {
    Docx,
    Docm,
    Dotx,
    Dotm,
    Xlsx,
    Xlsm,
    Xltx,
    Xltm,
    Pptx,
    Pptm,
    Potx,
    Potm,
}

impl AllowedFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Docx => "docx",
            Self::Docm => "docm",
            Self::Dotx => "dotx",
            Self::Dotm => "dotm",
            Self::Xlsx => "xlsx",
            Self::Xlsm => "xlsm",
            Self::Xltx => "xltx",
            Self::Xltm => "xltm",
            Self::Pptx => "pptx",
            Self::Pptm => "pptm",
            Self::Potx => "potx",
            Self::Potm => "potm",
        }
    }

    /// Map a main-part content type to an allowed format.
    pub fn from_content_type(ct: &str) -> Option<Self> {
        Some(match ct {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml" => {
                Self::Docx
            }
            "application/vnd.ms-word.document.macroEnabled.main+xml" => Self::Docm,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml" => {
                Self::Dotx
            }
            "application/vnd.ms-word.template.macroEnabledTemplate.main+xml" => Self::Dotm,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml" => {
                Self::Xlsx
            }
            "application/vnd.ms-excel.sheet.macroEnabled.main+xml" => Self::Xlsm,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml" => {
                Self::Xltx
            }
            "application/vnd.ms-excel.template.macroEnabled.main+xml" => Self::Xltm,
            "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml" => {
                Self::Pptx
            }
            "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml" => Self::Pptm,
            "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml" => {
                Self::Potx
            }
            "application/vnd.ms-powerpoint.template.macroEnabled.main+xml" => Self::Potm,
            _ => return None,
        })
    }
}

fn starts_with(bytes: &[u8], magic: &[u8]) -> bool {
    bytes.len() >= magic.len() && bytes[..magic.len()] == *magic
}

fn utf16_le_contains(haystack: &[u8], needle: &str) -> bool {
    let encoded: Vec<u8> = needle
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    if encoded.is_empty() || haystack.len() < encoded.len() {
        return false;
    }
    haystack
        .windows(encoded.len())
        .any(|window| window == encoded.as_slice())
}

fn looks_like_text(bytes: &[u8]) -> bool {
    if bytes.len() >= 2
        && ((bytes[0] == 0xff && bytes[1] == 0xfe) || (bytes[0] == 0xfe && bytes[1] == 0xff))
    {
        return true;
    }
    let sample = &bytes[..bytes.len().min(4096)];
    !sample.contains(&0)
}

/// Sniff non-zip containers before attempting OPC open.
pub fn sniff_unsupported_container(bytes: &[u8]) -> Option<&'static str> {
    if starts_with(bytes, &CFB_MAGIC) {
        if utf16_le_contains(bytes, "EncryptionInfo")
            && utf16_le_contains(bytes, "EncryptedPackage")
        {
            return Some("encrypted");
        }
        if utf16_le_contains(bytes, "WordDocument") {
            return Some("doc");
        }
        if utf16_le_contains(bytes, "Workbook") || utf16_le_contains(bytes, "Book") {
            return Some("xls");
        }
        if utf16_le_contains(bytes, "PowerPoint Document") {
            return Some("ppt");
        }
        return Some("ole");
    }
    if starts_with(bytes, &ZIP_LOCAL) || starts_with(bytes, &ZIP_EMPTY) {
        return None;
    }
    if !bytes.is_empty() && looks_like_text(bytes) {
        return Some("csv");
    }
    if bytes.is_empty() {
        return Some("empty");
    }
    Some("unknown")
}

/// Resolve the allow-listed format from content types and the officeDocument target part.
pub fn format_from_package(
    types: &ContentTypes,
    main_part: Option<&str>,
) -> Result<AllowedFormat, OpcError> {
    let Some(main) = main_part else {
        return Err(OpcError::unsupported("missing_office_document"));
    };
    let Some(ct) = content_type_for_part(types, main) else {
        return Err(OpcError::unsupported(format!("unknown_main_part:{main}")));
    };
    // Binary workbook (xlsb) is a zip but out of scope.
    if ct.contains("sheet.binary") || main.ends_with("workbook.bin") {
        return Err(OpcError::unsupported("xlsb"));
    }
    AllowedFormat::from_content_type(ct).ok_or_else(|| OpcError::unsupported(ct))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: CFB magic accepted as a zip / allowed package instead of UnsupportedFormat.
    #[test]
    fn cfb_magic_is_unsupported_ole_or_legacy() {
        let mut bytes = vec![0u8; 64];
        bytes[..8].copy_from_slice(&CFB_MAGIC);
        // Embed UTF-16LE "Workbook"
        let name: Vec<u8> = "Workbook"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        bytes.extend_from_slice(&name);
        assert_eq!(sniff_unsupported_container(&bytes), Some("xls"));
    }
}
