// SPDX-License-Identifier: MIT
// Ported from ChristopherVR/ooxml@e66aaec src/core/text/css-font-family.ts; changes listed in PROVENANCE.md.

pub fn css_font_family(name: &str) -> String {
    let lower = name.to_lowercase();
    let known = match lower.as_str() {
        "calibri" => Some("\"Calibri\", \"Carlito\", \"Segoe UI\", Arial, sans-serif"),
        "calibri light" => {
            Some("\"Calibri Light\", \"Calibri\", \"Carlito\", \"Segoe UI\", Arial, sans-serif")
        }
        "cambria" => Some("\"Cambria\", \"Caladea\", Georgia, serif"),
        "arial" => Some("Arial, \"Liberation Sans\", Helvetica, sans-serif"),
        "times new roman" => Some("\"Times New Roman\", \"Liberation Serif\", Times, serif"),
        "courier new" => Some("\"Courier New\", \"Liberation Mono\", monospace"),
        "aptos" => Some("\"Aptos\", \"Calibri\", \"Carlito\", \"Segoe UI\", sans-serif"),
        _ => None,
    };
    if let Some(k) = known {
        return k.to_string();
    }
    let escaped = name.replace(['"', '\\'], "");
    format!(
        "\"{}\", \"Calibri\", \"Carlito\", \"Segoe UI\", sans-serif",
        escaped
    )
}
