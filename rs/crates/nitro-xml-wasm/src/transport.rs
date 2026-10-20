// SPDX-License-Identifier: MIT
//! Bulk transfer and interned strings for the JS/WASM boundary (#14).
//!
//! Per-frame command buffer, measure batches and post-apply layout read-back
//! live in [`nitro_xml_render`] (shared with format painters). Decision (see
//! `docs/boundary.md`): struct-of-arrays typed-array views over wasm memory for
//! bulk numeric data; never per-cell JS↔WASM calls on the render path.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// FNV-1a over bytes — fixed, no entropy, safe for wasm (no `getrandom` /
/// `RandomState`). Short CSS prop/value keys dominate the paint intern path.
#[derive(Clone)]
struct FnvHasher(u64);

impl Default for FnvHasher {
    fn default() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl Hasher for FnvHasher {
    fn write(&mut self, bytes: &[u8]) {
        const FNV_PRIME: u64 = 0x100000001b3;
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

type HashToIds = HashMap<u64, Vec<u32>, BuildHasherDefault<FnvHasher>>;

/// Per-document interned string table. Id `0` is always the empty string.
///
/// `by_id` owns each string once; `by_hash` maps FNV-1a → candidate ids (verify
/// with `by_id[id]`) so intern is O(1) expected without a second `String` copy
/// or `RandomState`/`getrandom`. Paint frames intern the same CSS keys
/// thousands of times — a linear `Vec` scan was the xlsx S encode hotspot (#30).
#[derive(Debug, Default)]
pub struct StringTable {
    by_id: Vec<String>,
    by_hash: HashToIds,
    /// One-slot sticky cache: paint emits the same class/prop many times in a row.
    last: Option<(u64, u32)>,
}

impl StringTable {
    pub fn new() -> Self {
        let mut t = Self {
            by_id: Vec::new(),
            by_hash: HashToIds::default(),
            last: None,
        };
        t.intern("");
        t
    }

    #[inline]
    fn hash_str(s: &str) -> u64 {
        let mut h = FnvHasher::default();
        h.write(s.as_bytes());
        h.finish()
    }

    pub fn intern(&mut self, s: &str) -> u32 {
        let hash = Self::hash_str(s);
        if let Some((h, id)) = self.last {
            if h == hash && self.by_id[id as usize] == s {
                return id;
            }
        }
        if let Some(ids) = self.by_hash.get(&hash) {
            for &id in ids {
                if self.by_id[id as usize] == s {
                    self.last = Some((hash, id));
                    return id;
                }
            }
        }
        let id = self.by_id.len() as u32;
        self.by_id.push(s.to_owned());
        self.by_hash.entry(hash).or_default().push(id);
        self.last = Some((hash, id));
        id
    }

    pub fn resolve(&self, id: u32) -> Option<&str> {
        self.by_id.get(id as usize).map(String::as_str)
    }

    pub fn len(&self) -> u32 {
        self.by_id.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_reuses_ids_via_hash_index() {
        // Catches: HashMap-backed intern must still dedupe and keep id 0 empty.
        let mut t = StringTable::new();
        assert_eq!(t.intern(""), 0);
        let a = t.intern("backgroundColor");
        let b = t.intern("backgroundColor");
        let c = t.intern("left");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(t.resolve(a), Some("backgroundColor"));
        assert_eq!(t.resolve(0), Some(""));
    }

    #[test]
    fn intern_many_unique_stays_correct() {
        // Catches: by_hash/by_id drift when inserting hundreds of unique paint styles.
        let mut t = StringTable::new();
        let mut ids = Vec::new();
        for i in 0..400 {
            ids.push(t.intern(&format!("left:{i}px;top:0px;width:10px;height:12px")));
        }
        for (i, &expected) in ids.iter().enumerate() {
            assert_eq!(
                t.intern(&format!("left:{i}px;top:0px;width:10px;height:12px")),
                expected
            );
        }
        assert_eq!(t.len(), 401); // empty + 400
    }

    #[test]
    fn intern_allocates_string_once_per_unique() {
        // Catches: hash→id index must not store a second owned copy of each string.
        let mut t = StringTable::new();
        let _ = t.intern("only-once");
        assert_eq!(
            t.by_id.iter().filter(|s| s.as_str() == "only-once").count(),
            1
        );
    }
}
