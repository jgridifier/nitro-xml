# Rust hot-path perf workstream (branch `perf/rust-hotpaths`)

Goal: the Rust core is clearly faster than the TypeScript engine (`upstream/`, pinned e66aaec) on every
benchmark fixture (S/M/L for xlsx, docx, pptx) in both time and peak memory. This file is the cumulative
record; each slice appends a row to the summary and a section below.

## How to measure

| What | Command |
|---|---|
| TS vs WASM open time, 95% CI of the median ratio | `node --expose-gc perf/large/ts-vs-wasm.mjs --runs=21 --tier=S,M,L` |
| Isolated WASM open (fresh process, 2 warmups + 11 runs) | per-process median, median of 9 processes (see slice 1) |
| Peak memory, one fresh process per open (VmHWM growth over post-init baseline) | `node perf/large/peak-mem.mjs --runs=5 [--pkg=<dir>]` |
| Where the time goes | `cargo build --release --target wasm32-unknown-unknown` with `CARGO_PROFILE_RELEASE_DEBUG=1 CARGO_PROFILE_RELEASE_STRIP=none`, `wasm-bindgen --keep-debug`, then `node --cpu-prof --cpu-prof-interval 50` over many fresh processes (wasm frames keep their Rust names; ~1 min build, no wasm-opt) |

Notes that change how numbers read:

- **Cold open is Liftoff-bound.** V8 runs wasm in its baseline tier for roughly the first dozen opens; a 4 ms
  open warms to ~3 ms. The harness (2 warmups + N runs) therefore measures mostly baseline-tier code, so
  work removed from the open path pays off more than micro-tuning of loops.
- The TS number depends on run count (its JIT warms): `--runs=11` gave 6.0 ms on xlsx S, `--runs=21` gives 4.0 ms.
  Compare engines only at equal run counts.
- xlsx asymmetry (harness): TS = full `loadWorkbook`; WASM = `open_first_view` + `process_frame` (first paint).

## Summary (cumulative)

Time: `ts-vs-wasm.mjs --runs=21`, median ms (WASM/TS with 95% CI). Peak memory: `peak-mem.mjs --runs=5`, MiB of
process peak-RSS growth during one cold open in a fresh process (includes V8's compile memory for the wasm
module; see "Memory floor").

| Fixture | TS ms | WASM before | WASM after | ratio after (CI) | TS peak MiB | WASM peak before | WASM peak after |
|---|--:|--:|--:|--:|--:|--:|--:|
| xlsx S (1k cells) | 4.0 to 5.8 | 4.3 to 7.1 | 3.9 to 4.1 | 0.88 to 1.06 ([0.76, 1.12] best run) | 3.0 | 45.2 | 44.1 |
| xlsx M (100k) | 220.9 | 29.7 | 15.5 | 0.070 ([0.068, 0.078]) | 124.5 | 46.1 | 40.1 |
| xlsx L (1m) | 2484.4 | 5.9 | 4.5 | 0.002 ([0.002, 0.002]) | 1004.6 | 53.0 | 54.6 |
| docx S | 2.96 | 0.93 | 1.07 | 0.361 ([0.329, 0.376]) | 2.3 | 35.0 | 35.4 |
| docx M | 7.40 | 1.48 | 1.68 | 0.227 ([0.214, 0.238]) | 4.7 | 38.2 | 39.5 |
| docx L | 31.03 | 3.92 | 4.32 | 0.139 ([0.129, 0.153]) | 20.9 | 45.5 | 47.2 |
| pptx S | 14.84 | 0.49 | 0.48 | 0.032 ([0.030, 0.037]) | 4.4 | 32.2 | 31.7 |
| pptx M | 60.61 | 0.91 | 0.86 | 0.014 ([0.013, 0.015]) | 34.3 | 43.0 | 44.1 |
| pptx L | 223.03 | 2.69 | 2.54 | 0.011 ([0.011, 0.012]) | 48.9 | 47.0 | 46.0 |

docx and pptx rows are unchanged within noise (this slice does not touch those crates; the table is the full
baseline). Isolated WASM-only xlsx opens (fresh processes, median of 9): S 7.37 to 3.88 ms (-47%), M 27.98 to 16.97 ms (-39%).

### Where WASM still loses

- **xlsx S, time:** at tie to a small win depending on run count and machine noise (before: 1.2x to 1.8x slower).
  Remaining cost is about half first-paint (`process_frame`: `build_items` 26%, `StringTable::intern` 8%, text
  measure), which belongs to the grid-paint lineage and is out of scope for this workstream.
- **Peak memory, S/M docx/pptx and xlsx S:** a fixed ~32 to 45 MiB floor per fresh process (module compile) against
  2 to 5 MiB for TS. L and xlsx M already win (0.05x and 0.32x of TS). See "Memory floor" below.

## Slice 1: xlsx open path (this PR)

Profile first (flamegraph-style cpu-prof of the real wasm), then remove work from the open path. Changes, all
in `nitro-xml-xlsx`:

| Change | Why (profile) |
|---|---|
| `sheet_data` stream parser used for every fragment size (was only >= 64 KiB), plain `Reader` + zero-copy `read_event`, one attribute pass per `<c>` with duplicate checks off | small sheets went through wrapper `format!` + DOM + per-element name compares (`children_ns`/`memcmp` 2 ms); 5 attribute scans per cell |
| Stream path normalises `\r\n` / `\r` to `\n` like the DOM parser | latent bug: fragments >= 64 KiB kept `\r\n` in formula/text (found by `poi-dataValidationTableRange.xlsx` once the stream path was enabled everywhere) |
| `FastHasher` (multiply-rotate) for `CellIndex`, `CellStore`, SST sets/maps, number-format cache | SipHash was ~4% of open (`hash_one<&usize>`) |
| `compile_format` returns `Rc<Compiled>`; `choose_section` borrows | every formatted cell cloned the whole compiled format (token vectors) |
| `format_general`: integer fast path; `to_digits` takes the shortest round-trip digits when they fit in 15 places (exact: expansion is within 2^-53 of them, far inside half a unit of digit 15) | 40-digit exact (Dragon) formatting per cell |
| `lazy/sheet.rs`, `lazy/sst.rs`: `memchr` jumps between `<`, `memmem` for `r=`, borrowed `TagKind`, `drain` instead of re-allocating the remaining text per row/`<si>` | byte-at-a-time scans and an O(n) copy per row |
| SST: plain `<si><t>text</t></si>` (no entities, `\r`, `_x`) skips wrapper + DOM + escape decoding | DOM parse per shared string |

Gates run locally: `scripts/ci/rust.sh` (fmt, clippy -D warnings, all tests incl. numfmt oracle) and
`bun parity/run --ci` (xlsx worksheet/view-IR/engine/shell 48/48, docx 34/34, pptx 120/120, chart).

### Release-profile and wasm-opt trade-offs (measured, not applied here)

`docs/PRODUCT_SPEC.md` section 5 pins `opt-level=3` + `wasm-opt -O3`, so changing them is a spec decision. Measured on
the post-slice code, xlsx S (isolated process, median of 7 processes), first-open peak RSS (`/proc` VmHWM, Node base 50.8 MiB):

| Build | wasm bytes | xlsx S ms | xlsx M ms | RSS after first open |
|---|--:|--:|--:|--:|
| opt-level 3, no wasm-opt | 7,905,327 | 4.03 | n/a | 97.2 MiB |
| opt-level 3, `-O3` (shipped) | 6,558,519 | 3.92 | 16.7 | 114.0 MiB |
| opt-level 3, `-O2` | 6,641,525 | 4.35 | n/a | 119.6 MiB |
| opt-level 3, `-Os` | 6,566,405 | 4.11 | n/a | 112.8 MiB |
| opt-level 3, `-Oz` | 6,433,472 | 4.03 | n/a | 115.6 MiB |
| opt-level 3, `-O1` | 6,732,640 | 4.10 to 4.43 | 15.7 to 16.6 | 93.9 MiB |
| opt-level 3, `-O3` without inlining | 6,611,794 | 5.16 | n/a | 97.9 MiB |
| opt-level 2, `-O1` | 6,579,962 | 4.36 | 15.5 | 94.9 MiB |
| opt-level s, `-O1` | 5,594,572 | **3.16** | 23.9 | **88.3 MiB** |
| opt-level s, `-O3` | 5,404,016 | 4.88 | n/a | 99.3 MiB |
| opt-level 3, packages pptx/docx/render/regex/serde_json at `s`, `-O1` | 6,168,026 | 3.98 | 15.5 | 94.0 MiB |

Reading: `wasm-opt -O3`'s inlining costs ~20 MiB of first-open memory and about 1 ms of cold time against `-O1`
(and `-O3` takes ~170 s vs ~25 s in CI), with no throughput benefit (M, L, docx, pptx are equal). Compressed size
is flat (brotli 1.64 MB either way). `opt-level=s` is faster cold on xlsx S but ~45% slower on 100k-cell parse
(still ~13x faster than TS), so it is a product call. Recommended follow-up (needs Jared's sign-off on the spec
line): `wasm-opt -O1`.

### Memory floor (next slices)

Fresh-process first open grows RSS by 32 to 45 MiB: ~13 MiB at `initSync` (6.5 MB module) plus 20 to 30 MiB of lazy
compile for the functions the first open touches. TS grows 2 to 5 MiB on S fixtures. Levers measured above (`-O1`
saves ~20 MiB) and still to try: drop `regex`/`serde_json` from the wasm graph (regex_automata + regex_syntax +
aho_corasick = 8% of code), move the 1.1 MB static data (preset shapes) behind lazy init, split docx/pptx-only
code out of the xlsx path.

### Candidates seen in the profile but not touched (other lineages)

- `nitro_xml_wasm::transport::StringTable::intern` (memcmp-heavy, 8% of xlsx S open, called per painted cell and per measure): command-buffer encode.
- `view::fake_measure::estimate_width` substring searches (~1 ms per process at S) and `MeasureBatch::push_unique` SipHash: grid paint.
- zip CRC32 verification (3% of open, byte-wise in baseline-tier wasm).
