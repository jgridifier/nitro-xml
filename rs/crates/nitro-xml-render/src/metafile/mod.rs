// SPDX-License-Identifier: MIT
//! EMF/WMF metafile conversion for M1 pictures and OLE previews (inventory **S3**).
//!
//! Upstream loads [`emf-converter`](https://github.com/ChristopherVR/emf-converter)
//! (Apache-2.0) on demand via `src/core/pptx/core/utils/metafile-convert.ts`.
//! The one-to-one port lands with P5/P15; this module is the reserved path
//! (`nitro-xml-render::metafile`). See `docs/painters.md` and ADR 0002.

// Painter implementation TODO(P5/P15): convert EMF/WMF bytes to raster/SVG data
// URLs for `nitro-xml-pptx::render::{picture, ole}`. Ported files from
// emf-converter keep Apache-2.0 + PROVENANCE rows; switch this crate's
// `license` field to `MIT AND Apache-2.0` in that PR.
