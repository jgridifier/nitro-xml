// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/utils/image-first-pixel-png.ts; changes listed in PROVENANCE.md.
//! Synchronous, DOM-free PNG first-pixel (0,0) decoder for bar3D untargeted
//! extrusion faces. Inflates IDAT (zlib) then reads the first scanline filter +
//! pixel samples — matching upstream `zlibInflate` + filter-0 reconstruction.

use flate2::read::ZlibDecoder;
use std::io::Read;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

fn channels_for_color_type(color_type: u8) -> Option<usize> {
    match color_type {
        0 => Some(1), // grayscale
        2 => Some(3), // truecolor
        3 => Some(1), // indexed
        4 => Some(2), // grayscale + alpha
        6 => Some(4), // truecolor + alpha
        _ => None,
    }
}

/// Decode pixel (0,0) from a PNG byte buffer. Returns None for unsupported formats.
pub fn decode_png_first_pixel(bytes: &[u8]) -> Option<Rgba> {
    if bytes.len() < 24 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }

    let mut offset = 8usize;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut bit_depth = 0u8;
    let mut color_type = 0u8;
    let mut palette: Option<Vec<u8>> = None;
    let mut transparency: Option<Vec<u8>> = None;
    let mut idat: Vec<u8> = Vec::new();

    while offset + 8 <= bytes.len() {
        let chunk_len = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        if offset + 12 + chunk_len > bytes.len() {
            return None;
        }
        let chunk_type = &bytes[offset + 4..offset + 8];
        let data = &bytes[offset + 8..offset + 8 + chunk_len];
        match chunk_type {
            b"IHDR" if chunk_len >= 13 => {
                width = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                height = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                bit_depth = data[8];
                color_type = data[9];
            }
            b"PLTE" => palette = Some(data.to_vec()),
            b"tRNS" => transparency = Some(data.to_vec()),
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        offset += 12 + chunk_len;
    }

    if width == 0 || height == 0 || bit_depth != 8 {
        return None;
    }
    let channels = channels_for_color_type(color_type)?;
    if idat.is_empty() {
        return None;
    }

    let mut decoder = ZlibDecoder::new(idat.as_slice());
    // filter byte + first pixel samples are enough (row-0 left/up predictors are 0).
    let need = 1 + channels;
    let mut raw = vec![0u8; need];
    decoder.read_exact(&mut raw).ok()?;
    // Filter type is ignored: for (0,0) every PNG filter reconstructs to the raw samples.
    let samples = &raw[1..];

    match color_type {
        2 => Some(Rgba {
            r: samples[0],
            g: samples[1],
            b: samples[2],
            a: 255,
        }),
        6 => Some(Rgba {
            r: samples[0],
            g: samples[1],
            b: samples[2],
            a: samples[3],
        }),
        0 => Some(Rgba {
            r: samples[0],
            g: samples[0],
            b: samples[0],
            a: 255,
        }),
        4 => Some(Rgba {
            r: samples[0],
            g: samples[0],
            b: samples[0],
            a: samples[1],
        }),
        3 => {
            let pal = palette.as_ref()?;
            let idx = samples[0] as usize;
            if idx * 3 + 2 >= pal.len() {
                return None;
            }
            let a = transparency
                .as_ref()
                .and_then(|t| t.get(idx).copied())
                .unwrap_or(255);
            Some(Rgba {
                r: pal[idx * 3],
                g: pal[idx * 3 + 1],
                b: pal[idx * 3 + 2],
                a,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    fn crc32(data: &[u8]) -> u32 {
        // Minimal CRC for test PNG construction (IEEE).
        let mut crc: u32 = 0xffff_ffff;
        for &b in data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                let mask = if crc & 1 != 0 { 0xedb8_8320 } else { 0 };
                crc = (crc >> 1) ^ mask;
            }
        }
        !crc
    }

    fn chunk(ty: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(ty);
        out.extend_from_slice(data);
        let mut crc_data = Vec::new();
        crc_data.extend_from_slice(ty);
        crc_data.extend_from_slice(data);
        out.extend_from_slice(&crc32(&crc_data).to_be_bytes());
        out
    }

    fn tiny_rgba_png(r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
        // 1x1 RGBA, filter None
        let raw = vec![0, r, g, b, a];
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::fast());
        enc.write_all(&raw).unwrap();
        let idat = enc.finish().unwrap();

        let mut png = Vec::new();
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&1u32.to_be_bytes()); // w
        ihdr.extend_from_slice(&1u32.to_be_bytes()); // h
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // bit depth, RGBA, compression, filter, interlace
        png.extend(chunk(b"IHDR", &ihdr));
        png.extend(chunk(b"IDAT", &idat));
        png.extend(chunk(b"IEND", &[]));
        png
    }

    // Catches: PNG decoder rejects non-PNG data.
    #[test]
    fn decode_rejects_non_png() {
        assert!(decode_png_first_pixel(b"NOTPNG!!").is_none());
    }

    // Catches: zlib-compressed IDAT not inflated → bar3D face falls back to accent blue
    // instead of sampled green (f8f44b2075eb 2x2 picture fill).
    #[test]
    fn decode_zlib_idat_first_pixel_green() {
        let png = tiny_rgba_png(0, 255, 0, 255);
        let px = decode_png_first_pixel(&png).expect("decode");
        assert_eq!(
            px,
            Rgba {
                r: 0,
                g: 255,
                b: 0,
                a: 255
            }
        );
    }
}
