# SPDX-License-Identifier: MIT

# Fuzz / security-limit fixtures

Corpus inputs that exercise parser security limits and any minimised crash
reproducers found while fuzzing (#30 parser-hardening slice).

## Limit / reject fixtures

| Path | Purpose | Licence |
|------|---------|---------|
| `xml/depth-bomb-over-max.xml` | Nesting depth > `MAX_XML_DEPTH` (4096); must reject, not panic | MIT (original) |
| `xlsx/deep-nest-4096.xlsx` | Worksheet shell with 4095 nested `<n>` under `<worksheet>` (stack depth 4096); must open+paint in WASM browser without trap | MIT (original) |
| `xlsx/deep-nest-over-max.xlsx` | Same pattern at depth 4097; shell parse hits depth limit — open soft-empty or reject, no trap | MIT (original) |
| `xml/reject-*.xml`, `xml/fuzz-doctype-reject.xml` | Copied from `fixtures/xml/` (DTD/ENTITY off) | see `fixtures/xml/SOURCES.md` |
| `zip/ratio-bomb-11mib.zip` | Declared 11 MiB / 1 compressed byte (above `MIN_RATIO_CHECK_BYTES`, ratio ≫ 1000) | MIT (original) |

## Legitimate high-compression cases (must open)

Generated locally for #178 ratio-policy validation. Not upstream samples.

| Path | Worst part ratio | Uncompressed | Notes |
|------|------------------|--------------|-------|
| `xlsx/repeated-empty-rows-100k.xlsx` | **15.36×** (`sheet1.xml`) | 3.49 MiB | 100k styled empty rows |
| `xlsx/repeated-sst-cells-100k.xlsx` | **13.90×** (`sheet1.xml`) | 7.18 MiB | 100k cells → one shared string |
| `docx/repeated-paragraphs-50k.docx` | **255.33×** (`document.xml`) | 7.00 MiB | identical bold paragraphs |
| `docx/repeated-paragraphs-150k.docx` | **256.98×** (`document.xml`) | 21.0 MiB | same pattern, above 10 MiB floor |

## Corpus-wide measurement (all `fixtures/**` zip parts, Oct 2026)

Excluding the synthetic `ratio-bomb-*` fixture:

| Scope | Max per-part ratio | Part |
|-------|-------------------|------|
| All existing fixtures | **188.75×** | `upstream/pptx/.../ppt/media/media1.wav` (u=16 044, c=85) |
| Parts ≥ 1 MiB | **37.61×** | `real/docx/poi-bug65649.docx` `word/document.xml` (u=12.6 MiB) |
| Parts ≥ 10 MiB | **37.61×** | same |
| Generated legitimate (above) | **256.98×** | `repeated-paragraphs-150k.docx` |

Crash reproducers (if any) are added here as minimised files with a one-line note of
the panic they caught; prefer a corpus file over a unit test (AGENTS.md §0).
