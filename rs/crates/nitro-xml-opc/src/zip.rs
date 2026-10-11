// SPDX-License-Identifier: MIT
//! Bounded zip reader. Upstream (ChristopherVR/ooxml@e66aaec) has no zip code of its own: it reads
//! packages through JSZip (`JSZip.loadAsync` in `src/core/xlsx/read/package.ts`,
//! `src/core/docx/parse.ts`), whose behaviour is the parity reference.
//!
//! TODO(#11): implement the bounded zip reader.
