// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/opc/signature/detection.ts
// (isSignaturePart / detectDigitalSignatures listing only); changes listed in PROVENANCE.md.
//! Digital signature part detection. Paths are listed only; no XML-DSig verification in WASM.

/// Path prefix for all digital signature parts in an OOXML package.
pub const SIGNATURE_PART_PREFIX: &str = "_xmlsignatures/";

/// Whether a package part is a digital signature part (origin, signature, or their relationships).
pub fn is_signature_part(path: &str) -> bool {
    path.trim_start_matches('/')
        .to_ascii_lowercase()
        .starts_with(SIGNATURE_PART_PREFIX)
}

/// Detect digital signatures by checking for `_xmlsignatures/*.xml` paths.
pub fn detect_digital_signatures<'a>(
    zip_entry_paths: impl IntoIterator<Item = &'a str>,
) -> Vec<&'a str> {
    zip_entry_paths
        .into_iter()
        .filter(|p| p.starts_with(SIGNATURE_PART_PREFIX) && p.ends_with(".xml"))
        .collect()
}
