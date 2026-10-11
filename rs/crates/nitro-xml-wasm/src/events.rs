// SPDX-License-Identifier: MIT
//! Batched input-event protocol for the minimal JS loader (#14).
//!
//! The loader coalesces browser events and posts **at most one** batch per
//! animation frame into wasm (`Handle::push_event_batch`). Host wiring lives in
//! #37; the per-painter worker vs main-thread split is #36.
//!
//! ## Binary batch layout (little-endian)
//!
//! ```text
//! u16 version = 1
//! u16 count
//! repeated:
//!   u8  kind
//!   u8  flags
//!   u16 _pad
//!   f64 time_ms
//!   kind-specific payload (see kind table)
//! ```
//!
//! | Kind | Tag | Payload |
//! |---|---|---|
//! | Input | 1 | `u32 text_id` (interned composition/input string) |
//! | Pointer | 2 | `f32 x, y` `u8 type` `u8 button` `u16 buttons` — header `flags` is mods |
//! | Wheel | 3 | `f32 x, y, dx, dy` |
//! | Keyboard | 4 | `u32 key_id` `u32 code_id` `u8 type` `u8 mods` `u16 _` — header `flags` OR'd into mods |
//!
//! Header `flags` (pointer and keyboard): bit0 Shift, bit1 Ctrl, bit2 Alt, bit3 Meta.
//! Older batches leave `flags` at 0; keyboard mods still come from the payload byte.
//! | Composition | 5 | `u8 type` `u8 _` `u16 _` `u32 text_id` |
//! | Resize | 6 | `f32 width, height, dpr` |

use crate::handle::OoxmlError;

pub const VERSION: u16 = 1;

/// Modifier bits packed in the event header `flags` byte.
pub const MOD_SHIFT: u8 = 1;
pub const MOD_CTRL: u8 = 2;
pub const MOD_ALT: u8 = 4;
pub const MOD_META: u8 = 8;

/// Fixed key vocabulary interned by [`crate::handle::Handle`] at ids 1.. (id 0 is `""`).
/// The loader `KeyId` map must stay in this order.
pub const INPUT_KEYS: &[&str] = &[
    "ArrowLeft",
    "ArrowRight",
    "ArrowUp",
    "ArrowDown",
    "a",
    "A",
    "PageUp",
    "PageDown",
];

/// Intern [`INPUT_KEYS`] so their ids are `1..INPUT_KEYS.len()`.
pub fn seed_input_keys(table: &mut crate::transport::StringTable) {
    for (i, key) in INPUT_KEYS.iter().enumerate() {
        let id = table.intern(key);
        debug_assert_eq!(id, (i + 1) as u32);
    }
}

pub mod kind {
    pub const INPUT: u8 = 1;
    pub const POINTER: u8 = 2;
    pub const WHEEL: u8 = 3;
    pub const KEYBOARD: u8 = 4;
    pub const COMPOSITION: u8 = 5;
    pub const RESIZE: u8 = 6;
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Input {
        time_ms: f64,
        text_id: u32,
    },
    Pointer {
        time_ms: f64,
        x: f32,
        y: f32,
        type_: u8,
        button: u8,
        buttons: u16,
        /// Header flags: bit0 Shift, bit1 Ctrl, bit2 Alt, bit3 Meta.
        mods: u8,
    },
    Wheel {
        time_ms: f64,
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
    },
    Keyboard {
        time_ms: f64,
        key_id: u32,
        code_id: u32,
        type_: u8,
        mods: u8,
    },
    Composition {
        time_ms: f64,
        type_: u8,
        text_id: u32,
    },
    Resize {
        time_ms: f64,
        width: f32,
        height: f32,
        dpr: f32,
    },
}

#[derive(Debug, Default)]
pub struct EventQueue {
    pending: Vec<Event>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn extend(&mut self, batch: Vec<Event>) {
        self.pending.extend(batch);
    }

    pub fn drain(&mut self) -> usize {
        let n = self.pending.len();
        self.pending.clear();
        n
    }

    pub fn pending_snapshot(&self) -> Vec<Event> {
        self.pending.clone()
    }
}

struct Cursor<'a> {
    data: &'a [u8],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, i: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.i)
    }

    fn need(&self, n: usize) -> Result<(), OoxmlError> {
        if self.remaining() < n {
            Err(OoxmlError::with_code(
                "event_batch_truncated",
                format!("need {n} bytes, have {}", self.remaining()),
            ))
        } else {
            Ok(())
        }
    }

    fn u8(&mut self) -> Result<u8, OoxmlError> {
        self.need(1)?;
        let v = self.data[self.i];
        self.i += 1;
        Ok(v)
    }

    fn u16(&mut self) -> Result<u16, OoxmlError> {
        self.need(2)?;
        let v = u16::from_le_bytes([self.data[self.i], self.data[self.i + 1]]);
        self.i += 2;
        Ok(v)
    }

    fn u32(&mut self) -> Result<u32, OoxmlError> {
        self.need(4)?;
        let v = u32::from_le_bytes([
            self.data[self.i],
            self.data[self.i + 1],
            self.data[self.i + 2],
            self.data[self.i + 3],
        ]);
        self.i += 4;
        Ok(v)
    }

    fn f32(&mut self) -> Result<f32, OoxmlError> {
        Ok(f32::from_bits(self.u32()?))
    }

    fn f64(&mut self) -> Result<f64, OoxmlError> {
        self.need(8)?;
        let mut b = [0u8; 8];
        b.copy_from_slice(&self.data[self.i..self.i + 8]);
        self.i += 8;
        Ok(f64::from_le_bytes(b))
    }
}

/// Decode a version-1 event batch. Malformed input returns [`OoxmlError`], never panics.
pub fn decode_batch(bytes: &[u8]) -> Result<Vec<Event>, OoxmlError> {
    let mut c = Cursor::new(bytes);
    let version = c.u16()?;
    if version != VERSION {
        return Err(OoxmlError::with_code(
            "event_batch_version",
            format!("unsupported event batch version {version}"),
        ));
    }
    let count = c.u16()? as usize;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let k = c.u8()?;
        let flags = c.u8()?;
        let _pad = c.u16()?;
        let time_ms = c.f64()?;
        let ev = match k {
            kind::INPUT => Event::Input {
                time_ms,
                text_id: c.u32()?,
            },
            kind::POINTER => {
                let x = c.f32()?;
                let y = c.f32()?;
                let type_ = c.u8()?;
                let button = c.u8()?;
                let buttons = c.u16()?;
                Event::Pointer {
                    time_ms,
                    x,
                    y,
                    type_,
                    button,
                    buttons,
                    mods: flags,
                }
            }
            kind::WHEEL => Event::Wheel {
                time_ms,
                x: c.f32()?,
                y: c.f32()?,
                dx: c.f32()?,
                dy: c.f32()?,
            },
            kind::KEYBOARD => {
                let key_id = c.u32()?;
                let code_id = c.u32()?;
                let type_ = c.u8()?;
                let mods = c.u8()?;
                let _ = c.u16()?;
                Event::Keyboard {
                    time_ms,
                    key_id,
                    code_id,
                    type_,
                    mods: mods | flags,
                }
            }
            kind::COMPOSITION => {
                let type_ = c.u8()?;
                let _ = c.u8()?;
                let _ = c.u16()?;
                let text_id = c.u32()?;
                Event::Composition {
                    time_ms,
                    type_,
                    text_id,
                }
            }
            kind::RESIZE => Event::Resize {
                time_ms,
                width: c.f32()?,
                height: c.f32()?,
                dpr: c.f32()?,
            },
            other => {
                return Err(OoxmlError::with_code(
                    "event_batch_kind",
                    format!("unknown event kind {other}"),
                ));
            }
        };
        out.push(ev);
    }
    if c.remaining() != 0 {
        return Err(OoxmlError::with_code(
            "event_batch_trailing",
            format!("{} trailing bytes", c.remaining()),
        ));
    }
    Ok(out)
}

/// Encode helpers for tests (the real encoder is in the loader, #37).
#[cfg(test)]
pub fn encode_batch(events: &[Event]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(events.len() as u16).to_le_bytes());
    for ev in events {
        match ev {
            Event::Input { time_ms, text_id } => {
                out.push(kind::INPUT);
                out.push(0);
                out.extend_from_slice(&0u16.to_le_bytes());
                out.extend_from_slice(&time_ms.to_le_bytes());
                out.extend_from_slice(&text_id.to_le_bytes());
            }
            Event::Pointer {
                time_ms,
                x,
                y,
                type_,
                button,
                buttons,
                mods,
            } => {
                out.push(kind::POINTER);
                out.push(*mods);
                out.extend_from_slice(&0u16.to_le_bytes());
                out.extend_from_slice(&time_ms.to_le_bytes());
                out.extend_from_slice(&x.to_le_bytes());
                out.extend_from_slice(&y.to_le_bytes());
                out.push(*type_);
                out.push(*button);
                out.extend_from_slice(&buttons.to_le_bytes());
            }
            Event::Wheel {
                time_ms,
                x,
                y,
                dx,
                dy,
            } => {
                out.push(kind::WHEEL);
                out.push(0);
                out.extend_from_slice(&0u16.to_le_bytes());
                out.extend_from_slice(&time_ms.to_le_bytes());
                out.extend_from_slice(&x.to_le_bytes());
                out.extend_from_slice(&y.to_le_bytes());
                out.extend_from_slice(&dx.to_le_bytes());
                out.extend_from_slice(&dy.to_le_bytes());
            }
            Event::Keyboard {
                time_ms,
                key_id,
                code_id,
                type_,
                mods,
            } => {
                out.push(kind::KEYBOARD);
                out.push(*mods);
                out.extend_from_slice(&0u16.to_le_bytes());
                out.extend_from_slice(&time_ms.to_le_bytes());
                out.extend_from_slice(&key_id.to_le_bytes());
                out.extend_from_slice(&code_id.to_le_bytes());
                out.push(*type_);
                out.push(*mods);
                out.extend_from_slice(&0u16.to_le_bytes());
            }
            Event::Composition {
                time_ms,
                type_,
                text_id,
            } => {
                out.push(kind::COMPOSITION);
                out.push(0);
                out.extend_from_slice(&0u16.to_le_bytes());
                out.extend_from_slice(&time_ms.to_le_bytes());
                out.push(*type_);
                out.push(0);
                out.extend_from_slice(&0u16.to_le_bytes());
                out.extend_from_slice(&text_id.to_le_bytes());
            }
            Event::Resize {
                time_ms,
                width,
                height,
                dpr,
            } => {
                out.push(kind::RESIZE);
                out.push(0);
                out.extend_from_slice(&0u16.to_le_bytes());
                out.extend_from_slice(&time_ms.to_le_bytes());
                out.extend_from_slice(&width.to_le_bytes());
                out.extend_from_slice(&height.to_le_bytes());
                out.extend_from_slice(&dpr.to_le_bytes());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_batch_returns_typed_error_not_panic() {
        // Catches: truncated event batches must not panic across the wasm boundary.
        let err = decode_batch(&[1, 0, 1, 0]).unwrap_err();
        assert_eq!(err.code(), "event_batch_truncated");
        let err = decode_batch(&[]).unwrap_err();
        assert_eq!(err.code(), "event_batch_truncated");
        let mut bad = encode_batch(&[Event::Input {
            time_ms: 0.0,
            text_id: 1,
        }]);
        bad[4] = 99; // corrupt kind
        let err = decode_batch(&bad).unwrap_err();
        assert_eq!(err.code(), "event_batch_kind");
    }

    #[test]
    fn mixed_batch_round_trips() {
        // Catches: pointer/keyboard/resize packing must stay aligned for the loader encoder (#37).
        let events = vec![
            Event::Pointer {
                time_ms: 1.5,
                x: 10.0,
                y: 20.0,
                type_: 1,
                button: 0,
                buttons: 1,
                mods: MOD_SHIFT,
            },
            Event::Keyboard {
                time_ms: 2.0,
                key_id: 3,
                code_id: 4,
                type_: 1,
                mods: 2,
            },
            Event::Resize {
                time_ms: 3.0,
                width: 800.0,
                height: 600.0,
                dpr: 2.0,
            },
            Event::Composition {
                time_ms: 4.0,
                type_: 2,
                text_id: 9,
            },
        ];
        let bytes = encode_batch(&events);
        let decoded = decode_batch(&bytes).unwrap();
        assert_eq!(decoded, events);
    }
}
