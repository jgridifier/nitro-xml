// SPDX-License-Identifier: MIT
//! String table ids referenced by command-buffer opcodes (e.g. `SetInnerMarkup`).

#[derive(Debug, Default)]
pub struct StringTable {
    strings: Vec<String>,
}

impl StringTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, value: &str) -> u32 {
        if let Some((i, _)) = self
            .strings
            .iter()
            .enumerate()
            .find(|(_, s)| s.as_str() == value)
        {
            return i as u32;
        }
        let id = self.strings.len();
        self.strings.push(value.to_string());
        id as u32
    }

    pub fn get(&self, id: u32) -> Option<&str> {
        self.strings.get(id as usize).map(|s| s.as_str())
    }

    pub fn as_slice(&self) -> &[String] {
        &self.strings
    }
}
