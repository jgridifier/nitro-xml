// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/password-hash.ts; changes listed in
// PROVENANCE.md.
//! ECMA-376 agile password hash attributes on workbook/sheet protection.

use nitro_xml_opc::xml::Element;

use crate::model::ModernPasswordHash;
use crate::xml_util::att;

fn modern_hash_attr_names(prefix: &str) -> (String, String, String, String) {
    let name = |base: &str| -> String {
        if prefix.is_empty() {
            base.to_owned()
        } else {
            let mut chars = base.chars();
            let first = chars.next().map(|c| c.to_ascii_uppercase()).unwrap_or('A');
            format!("{prefix}{first}{}", chars.as_str())
        }
    };
    (
        name("algorithmName"),
        name("hashValue"),
        name("saltValue"),
        name("spinCount"),
    )
}

/// Reads an ECMA-376 agile password hash from `node`.
pub fn read_modern_hash(node: Option<&Element>, prefix: &str) -> Option<ModernPasswordHash> {
    let (algorithm_name_k, hash_value_k, salt_value_k, spin_count_k) =
        modern_hash_attr_names(prefix);
    let algorithm_name = att(node, &algorithm_name_k)?.to_owned();
    let hash_value = att(node, &hash_value_k)?.to_owned();
    let salt_value = att(node, &salt_value_k)?.to_owned();
    let mut hash = ModernPasswordHash {
        algorithm_name,
        hash_value,
        salt_value,
        spin_count: None,
    };
    if let Some(spin) = att(node, &spin_count_k) {
        if !spin.trim().is_empty() {
            if let Ok(n) = spin.parse::<f64>() {
                if n.is_finite() {
                    hash.spin_count = Some(n);
                }
            }
        }
    }
    Some(hash)
}
