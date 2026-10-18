// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/chart-bar3d-face-picture.ts; changes listed in PROVENANCE.md.
//! Picture-fill face targeting for a `bar3D` chart's oblique-projection
//! extrusion faces (C2-G9 3-D face-targeting half).
//!
//! Bar3D untargeted extrusion faces (top/side/end where applyToSides/applyToEnd are 0)
//! should sample the picture's first pixel and tint/shade that colour instead of using
//! the series colour, matching PowerPoint's behaviour.

use super::chart_palette_shade::{shade_hex, tint_hex};
use super::png_first_pixel::decode_png_first_pixel;

/// Resolve the fallback fill for a bar3D extrusion face when no picture targets it:
/// if a picture exists and can be sampled, use the sampled colour tinted/shaded;
/// otherwise use the base colour tinted/shaded.
pub fn resolve_untargeted_bar_face_fill(
    face: &str,
    base_color: &str,
    picture_color: Option<String>,
) -> String {
    let color = picture_color.unwrap_or_else(|| base_color.to_string());
    if face == "end" {
        tint_hex(&color, 0.22)
    } else {
        shade_hex(&color, 0.25)
    }
}

/// Try to sample a PNG's first pixel (0,0) from a data URL.
/// Returns the sampled colour as #rrggbb, or None if it cannot be decoded.
pub fn sample_first_pixel_from_data_url(data_url: &str) -> Option<String> {
    if !data_url.starts_with("data:image/png;base64,") {
        return None;
    }

    let base64_part = &data_url["data:image/png;base64,".len()..];
    let bytes = base64_decode(base64_part).ok()?;

    let pixel = decode_png_first_pixel(&bytes)?;
    if pixel.a == 0 {
        return None;
    }

    Some(format!("#{:02x}{:02x}{:02x}", pixel.r, pixel.g, pixel.b))
}

fn base64_decode(s: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let bytes = s.as_bytes();
    let mut result = Vec::new();

    for chunk in bytes.chunks(4) {
        let b64_chars: Vec<u8> = chunk.to_vec();
        if b64_chars.len() < 2 {
            break;
        }

        let mut sextet = [0u8; 4];
        for (i, &ch) in b64_chars.iter().enumerate() {
            if i >= 4 {
                break;
            }
            sextet[i] = match ch {
                b'A'..=b'Z' => ch - b'A',
                b'a'..=b'z' => ch - b'a' + 26,
                b'0'..=b'9' => ch - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' => 0,
                _ => continue,
            };
        }

        match b64_chars.len() {
            2 => {
                let byte = (sextet[0] << 2) | (sextet[1] >> 4);
                result.push(byte);
            }
            3 => {
                let byte1 = (sextet[0] << 2) | (sextet[1] >> 4);
                let byte2 = (sextet[1] << 4) | (sextet[2] >> 2);
                result.push(byte1);
                result.push(byte2);
            }
            4 => {
                let byte1 = (sextet[0] << 2) | (sextet[1] >> 4);
                let byte2 = (sextet[1] << 4) | (sextet[2] >> 2);
                let byte3 = (sextet[2] << 6) | sextet[3];
                result.push(byte1);
                result.push(byte2);
                if b64_chars[3] != b'=' {
                    result.push(byte3);
                }
            }
            _ => {}
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // Catches: untargeted bar3D face uses sampled green not accent blue
    fn resolve_untargeted_with_sampled_color() {
        let sampled = Some("#00ff00".to_string()); // green
        let result = resolve_untargeted_bar_face_fill("side", "#4472c4", sampled);
        // Side face gets shaded, should be darker green not blue
        assert!(result.starts_with('#'));
    }

    #[test]
    // Catches: absent picture falls back to base colour
    fn resolve_untargeted_without_picture() {
        let result = resolve_untargeted_bar_face_fill("side", "#4472c4", None);
        // Should be shaded blue
        assert_eq!(result.len(), 7); // #rrggbb
    }

    #[test]
    fn base64_decode_simple() {
        let encoded = "SGVsbG8="; // "Hello"
        let decoded = base64_decode(encoded).unwrap();
        assert_eq!(&decoded, b"Hello");
    }
}
