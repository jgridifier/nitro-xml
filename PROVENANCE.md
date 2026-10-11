# Provenance

Nitro XML is MIT-licensed. **Files ported or derived from ooxml remain under Apache-2.0** (see NOTICE and LICENSE-THIRD-PARTY). This file states, for every such file, where it came from and what changed, following upstream's own `PROVENANCE.md` convention. It is required by Apache-2.0 §4(b) ("You must cause any modified files to carry prominent notices stating that You changed the files").

## Upstream

| Item | Value |
|---|---|
| Repository | https://github.com/ChristopherVR/ooxml |
| Licence | Apache-2.0 |
| Pinned commit | `e66aaec24adc0b68472eae1fc42aa4598e09c352` (2026-10-11) |
| Role | Algorithm source for the Rust port; the TS engine runs **only in the test and bench harness** as the correctness reference and speed baseline. No TypeScript from upstream ships in Nitro XML. |

## Rules

- A Rust file that ports or derives logic from an upstream file starts with:
  ```rust
  // SPDX-License-Identifier: Apache-2.0
  // Ported from ChristopherVR/ooxml@e66aaec <upstream path(s)>; changes listed in PROVENANCE.md.
  ```
- New original files start with `// SPDX-License-Identifier: MIT`.
- Add one row below per ported file in the same PR. The licence check fails if an `Apache-2.0` file has no row.
- Test fixtures copied from upstream are listed in `fixtures/manifest.json` with their own licence; fixtures of unknown licence are never committed.

## Ported files

| Nitro XML file | Upstream source (path @ commit) | Changes |
|---|---|---|
| `rs/crates/nitro-xml-opc/src/zip/mod.rs` | `src/core/xlsx/read/package.ts @ e66aaec` | Rust rewrite of readZipParts limits and SourceIndex case-insensitive lookup over the `zip` crate instead of JSZip; lazy per-part inflation; typed errors. |
| `rs/crates/nitro-xml-opc/src/zip/error.rs` | `src/core/xlsx/read/package.ts @ e66aaec` | Typed `ZipError` kinds in place of upstream `Error` messages; same limit categories as readZipParts. |
