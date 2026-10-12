# SPDX-License-Identifier: MIT

# Fuzz / security-limit fixtures

Corpus inputs that exercise parser security limits and any minimised crash
reproducers found while fuzzing (#30 parser-hardening slice).

| Path | Purpose | Licence |
|------|---------|---------|
| `xml/depth-bomb-257.xml` | Nesting depth > `MAX_XML_DEPTH` (256); must reject, not panic | MIT (original) |
| `xml/reject-*.xml`, `xml/fuzz-doctype-reject.xml` | Copied from `fixtures/xml/` (DTD/ENTITY off) | see `fixtures/xml/SOURCES.md` |
| `zip/ratio-bomb-201x.zip` | Declared uncompressed/compressed = 201 > `MAX_COMPRESSION_RATIO` (200) | MIT (original) |

Crash reproducers (if any) are added here as minimised files with a one-line
note of the panic they caught; prefer a corpus file over a unit test
(AGENTS.md §0).
