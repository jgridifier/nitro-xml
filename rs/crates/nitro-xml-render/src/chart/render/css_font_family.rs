// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/text/css-font-family.ts

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
    if let Some(stack) = known {
        return stack.to_string();
    }
    let quoted = format!("\"{}\"", name.replace(['"', '\\'], ""));
    format!(
        "{}, \"Calibri\", \"Carlito\", \"Segoe UI\", sans-serif",
        quoted
    )
}
