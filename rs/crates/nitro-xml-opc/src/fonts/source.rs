// SPDX-License-Identifier: MIT
//! Font-source policy: default Google Fonts, host mirror, or remote fetch off (#121).

use crate::fonts::google_webfonts::{build_google_fonts_href, GOOGLE_FONTS_CSS2_BASE};

/// Stable warning code when a requested face is served from a local/generic
/// fallback because remote webfont fetch is disabled.
pub const FONT_SUBSTITUTION_CODE: &str = "font_substitution";

/// Where the engine may load webfont stylesheets from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontSource {
    /// Remote CSS2 API. Default base is upstream's Google Fonts URL; hosts may
    /// supply an internal mirror with the same query shape.
    Remote { base_url: String },
    /// No remote fetch. Use embedded and locally installed fonts only; record
    /// substitutions in diagnostics for faces that would have been fetched.
    Off,
}

impl Default for FontSource {
    fn default() -> Self {
        Self::Remote {
            base_url: GOOGLE_FONTS_CSS2_BASE.to_owned(),
        }
    }
}

impl FontSource {
    /// Upstream default (`https://fonts.googleapis.com/css2`).
    pub fn google_fonts() -> Self {
        Self::default()
    }

    /// Internal mirror (or any CSS2-compatible base URL).
    pub fn mirror(base_url: impl Into<String>) -> Self {
        Self::Remote {
            base_url: base_url.into(),
        }
    }

    /// Disable remote webfont fetching entirely.
    pub fn off() -> Self {
        Self::Off
    }

    /// Whether any remote stylesheet / font-file fetch is allowed.
    pub fn allows_remote_fetch(&self) -> bool {
        matches!(self, Self::Remote { .. })
    }

    /// CSS2 base URL when remote fetch is enabled.
    pub fn base_url(&self) -> Option<&str> {
        match self {
            Self::Remote { base_url } => Some(base_url.as_str()),
            Self::Off => None,
        }
    }
}

/// One requested family that will paint with a local/generic face instead of a
/// remote webfont (fetch off, or resolver returned no href).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontSubstitution {
    pub requested: String,
    /// Face the engine will rely on (local install or CSS generic).
    pub used: String,
}

impl FontSubstitution {
    pub fn local(requested: impl Into<String>) -> Self {
        Self {
            requested: requested.into(),
            used: "local/generic".to_owned(),
        }
    }

    /// Human-readable diagnostic message for [`FONT_SUBSTITUTION_CODE`].
    pub fn message(&self) -> String {
        format!(
            "font substitution: '{}' → {} (remote webfont fetch disabled)",
            self.requested, self.used
        )
    }
}

/// Plan a webfont stylesheet fetch without performing any network I/O.
///
/// `catalogue_fragments` are already-matched CSS2 `family=…` fragments (from
/// the Google Fonts catalogue / metric-clone map — ported with #27). When the
/// source is [`FontSource::Off`], returns no href and one substitution per
/// `requested_families` entry that would otherwise have been fetched.
///
/// Returns `(stylesheet_href, substitutions)`. A `None` href means the host
/// must not issue any webfont network request.
pub fn resolve_webfont_plan(
    source: &FontSource,
    catalogue_fragments: &[impl AsRef<str>],
    requested_families: &[impl AsRef<str>],
) -> (Option<String>, Vec<FontSubstitution>) {
    match source {
        FontSource::Off => {
            let subs = requested_families
                .iter()
                .map(|f| FontSubstitution::local(f.as_ref()))
                .collect();
            (None, subs)
        }
        FontSource::Remote { base_url } => {
            let href = build_google_fonts_href(catalogue_fragments, base_url);
            (href, Vec::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::google_webfonts::build_google_fonts_fragment;

    #[test]
    fn fetch_off_never_produces_a_network_href() {
        // Catches: Off mode still emitting fonts.googleapis.com (or any) href,
        // which would make the loader/browser issue a remote webfont request.
        let frag = build_google_fonts_fragment("Carlito");
        let (href, subs) = resolve_webfont_plan(
            &FontSource::off(),
            &[frag.as_str()],
            &["Calibri", "Carlito"],
        );
        assert!(href.is_none(), "Off must not return a stylesheet href: {href:?}");
        assert_eq!(subs.len(), 2);
        assert!(subs.iter().all(|s| s.used == "local/generic"));
        assert!(subs.iter().any(|s| s.requested == "Calibri"));
        for s in &subs {
            assert!(!s.message().contains("fonts.googleapis.com"));
        }
    }

    #[test]
    fn substitution_shows_up_in_diagnostics() {
        // Catches: fetch-off path dropping the requested family from diagnostics,
        // so hosts cannot see which face was substituted.
        let (_href, subs) =
            resolve_webfont_plan(&FontSource::off(), &[] as &[&str], &["Calibri"]);
        assert_eq!(subs.len(), 1);
        assert_eq!(FONT_SUBSTITUTION_CODE, "font_substitution");
        let msg = subs[0].message();
        assert!(msg.contains("Calibri"), "{msg}");
        assert!(msg.contains("local/generic"), "{msg}");
    }
}
