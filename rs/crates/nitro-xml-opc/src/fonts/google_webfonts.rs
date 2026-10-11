// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/google-webfonts.ts; changes listed in PROVENANCE.md.

//! Google Fonts CSS2 href builders (upstream `google-webfonts.ts`).
//!
//! Pure URL construction only — catalogue lookup and DOM `<link>` injection stay
//! with the pptx painter port (#27). The configurable base URL / off switch is
//! [#121](https://github.com/jgridifier/nitro-xml/issues/121).

/// Base URL of the Google Fonts CSS2 API (upstream default).
pub const GOOGLE_FONTS_CSS2_BASE: &str = "https://fonts.googleapis.com/css2";

/// Axis spec requested for every catalogue family. The API is lenient: it serves
/// only the weights/styles the family actually has.
pub const GOOGLE_WEBFONT_AXIS: &str = "ital,wght@0,400;0,700;1,400;1,700";

/// `display=swap` so text paints with the fallback and swaps when ready.
const DISPLAY_PARAM: &str = "display=swap";

/// The css2 query fragment for one family, with the universal axis spec.
///
/// Word separators MUST be literal spaces, never `+`: the fragment is passed
/// through percent-encoding, which would turn an intended `+` separator into
/// `%2B`, and after URL decoding Google then sees a literal plus inside the
/// family name and rejects it with 400.
pub fn build_google_fonts_fragment(family: &str) -> String {
    format!("{family}:{GOOGLE_WEBFONT_AXIS}")
}

/// Build the Google Fonts CSS2 `<link>` href for the supplied (already verified)
/// fragments, or `None` when there is nothing to load.
///
/// `base_url` overrides [`GOOGLE_FONTS_CSS2_BASE`] for a deployment that mirrors
/// the CSS2 API on its own origin.
pub fn build_google_fonts_href(fragments: &[impl AsRef<str>], base_url: &str) -> Option<String> {
    if fragments.is_empty() {
        return None;
    }
    let mut query = String::new();
    for (i, fragment) in fragments.iter().enumerate() {
        if i > 0 {
            query.push('&');
        }
        query.push_str("family=");
        // encodeURIComponent-compatible: encode everything except A–Z a–z 0–9 -_.!~*'()
        // and leave spaces as %20 (not +).
        query.push_str(&encode_uri_component(fragment.as_ref()));
    }
    Some(format!("{base_url}?{query}&{DISPLAY_PARAM}"))
}

/// Percent-encode like JavaScript `encodeURIComponent`.
fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~'
            | b'*' | b'\'' | b'(' | b')' => out.push(b as char),
            _ => {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0xf) as usize] as char);
            }
        }
    }
    out
}

