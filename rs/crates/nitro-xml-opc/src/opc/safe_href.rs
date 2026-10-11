// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/opc/safe-href.ts; changes listed in PROVENANCE.md.
//! One hyperlink safety policy for every format.
//!
//! - [`is_safe_hyperlink_href`] is the original strict check (http, https, mailto only).
//! - [`hyperlink_policy`] separates store vs open; script-capable schemes are never stored or
//!   opened, including when hidden behind case changes, whitespace, controls or zero-width chars.

use std::collections::HashSet;
use std::sync::LazyLock;

static SAFE_SCHEMES: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| HashSet::from(["http:", "https:", "mailto:"]));

static OPEN_SCHEMES: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| HashSet::from(["http", "https", "mailto"]));

static STORE_SCHEMES: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| HashSet::from(["http", "https", "mailto", "ftp", "file"]));

static SCRIPT_SCHEMES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "javascript",
        "vbscript",
        "livescript",
        "jscript",
        "mocha",
        "data",
        "mhtml",
        "ms-its",
        "mk",
        "x-javascript",
        "blob",
    ])
});

/// Why a hyperlink may not be stored or opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HyperlinkRejection {
    Empty,
    ScriptScheme,
    ObfuscatedScheme,
    ControlCharacters,
    UnsupportedScheme,
    NotOpenable,
}

impl HyperlinkRejection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::ScriptScheme => "script-scheme",
            Self::ObfuscatedScheme => "obfuscated-scheme",
            Self::ControlCharacters => "control-characters",
            Self::UnsupportedScheme => "unsupported-scheme",
            Self::NotOpenable => "not-openable",
        }
    }
}

/// Verdict of [`hyperlink_policy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HyperlinkPolicy {
    pub store: bool,
    pub open: bool,
    pub reason: Option<HyperlinkRejection>,
}

const STORE_ONLY: HyperlinkPolicy = HyperlinkPolicy {
    store: true,
    open: false,
    reason: Some(HyperlinkRejection::NotOpenable),
};

fn reject(reason: HyperlinkRejection) -> HyperlinkPolicy {
    HyperlinkPolicy {
        store: false,
        open: false,
        reason: Some(reason),
    }
}

/// Only http(s) and mailto targets are accepted; anything else (notably `javascript:`) is rejected.
pub fn is_safe_hyperlink_href(href: &str) -> bool {
    let trimmed = href.trim();
    let scheme = scheme_with_colon(trimmed);
    scheme
        .map(|s| SAFE_SCHEMES.contains(s.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn scheme_with_colon(href: &str) -> Option<&str> {
    // /^([a-z][a-z0-9+.-]*:)/i
    let bytes = href.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let first = bytes[0];
    if !first.is_ascii_alphabetic() {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b':' {
            return std::str::from_utf8(&bytes[..=i]).ok();
        }
        if !(b.is_ascii_alphanumeric() || b == b'+' || b == b'.' || b == b'-') {
            return None;
        }
        i += 1;
    }
    None
}

fn scheme_without_colon(normalized: &str) -> Option<&str> {
    // /^([a-z][a-z0-9+.-]*):/
    let bytes = normalized.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_lowercase() {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b':' {
            return std::str::from_utf8(&bytes[..i]).ok();
        }
        if !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'+' || b == b'.' || b == b'-') {
            return None;
        }
        i += 1;
    }
    None
}

fn is_ignorable(ch: char) -> bool {
    matches!(
        ch,
        '\u{0000}'..='\u{0020}'
            | '\u{007f}'
            | '\u{00ad}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{feff}'
    )
}

fn has_control(s: &str) -> bool {
    s.chars()
        .any(|c| matches!(c, '\u{0000}'..='\u{001f}' | '\u{007f}'))
}

/// Classify a hyperlink target. Leading and trailing whitespace is ignored; anything else that
/// could disguise the scheme makes the href unsafe.
pub fn hyperlink_policy(href: Option<&str>) -> HyperlinkPolicy {
    let Some(href) = href else {
        return reject(HyperlinkRejection::Empty);
    };
    let trimmed = href.trim();
    let normalized: String = trimmed
        .chars()
        .filter(|c| !is_ignorable(*c))
        .flat_map(|c| c.to_lowercase())
        .collect();
    if normalized.is_empty() {
        return reject(HyperlinkRejection::Empty);
    }
    let scheme = scheme_without_colon(&normalized);
    if let Some(scheme) = scheme {
        if SCRIPT_SCHEMES.contains(scheme) {
            return reject(HyperlinkRejection::ScriptScheme);
        }
        let expected = format!("{scheme}:");
        if !trimmed.to_ascii_lowercase().starts_with(&expected) {
            return reject(HyperlinkRejection::ObfuscatedScheme);
        }
    }
    if has_control(trimmed) {
        return reject(HyperlinkRejection::ControlCharacters);
    }
    // A one-letter "scheme" is a Windows drive path (C:\Reports\q1.xlsx).
    match scheme {
        None => STORE_ONLY,
        Some(s) if s.len() == 1 => STORE_ONLY,
        Some(s) if OPEN_SCHEMES.contains(s) => HyperlinkPolicy {
            store: true,
            open: true,
            reason: None,
        },
        Some(s) if STORE_SCHEMES.contains(s) => STORE_ONLY,
        Some(_) => reject(HyperlinkRejection::UnsupportedScheme),
    }
}

/// True when the href may be written to the document.
pub fn is_storable_hyperlink_href(href: Option<&str>) -> bool {
    hyperlink_policy(href).store
}

/// True when a viewer may open the href on click.
pub fn is_openable_hyperlink_href(href: Option<&str>) -> bool {
    hyperlink_policy(href).open
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: zero-width characters splicing a javascript: scheme past a naive starts_with check.
    #[test]
    fn rejects_zero_width_obfuscated_javascript() {
        let href = format!("java{}script:alert(1)", '\u{200b}');
        let policy = hyperlink_policy(Some(&href));
        assert!(!policy.store);
        assert!(!policy.open);
        assert_eq!(policy.reason, Some(HyperlinkRejection::ScriptScheme));
    }
}
