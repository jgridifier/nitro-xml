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

#### xlsx S gate peak — post-#352 attribution (2026-10-21, tip `68c7ba6`)

`peak-mem.mjs` scores **post-`initSync` VmHWM growth**. After [#352](https://github.com/jgridifier/nitro-xml/pull/352)'s
one-caller inline cap, xlsx S gate peak is ~18 MiB WASM vs ~5.7 MiB TS (LOSS ~3.1×). Stage split:

| | MiB |
|--|--:|
| Cold first open (post-init baseline → open) | **18.0** |
| Second open in the same process (compile already done) | **2.3** |
| TS `loadWorkbook` peak (same harness) | **5.6** |

So ~87% of the LOSS is first-touch compile, not retained xlsx heap. Application-level cuts on the
xlsx open path cannot flip the cell; the remaining lever is `wasm-opt -O1` (draft #332). Docx
peak / `handle.rs` format split stays with draft #359 — do not retouch from the xlsx stream.

### Candidates seen in the profile but not touched (other lineages)

- `nitro_xml_wasm::transport::StringTable::intern` (memcmp-heavy, 8% of xlsx S open, called per painted cell and per measure): command-buffer encode.
- `view::fake_measure::estimate_width` substring searches (~1 ms per process at S) and `MeasureBatch::push_unique` SipHash: grid paint.
- zip CRC32 verification (3% of open, byte-wise in baseline-tier wasm).

## Slice 2: XML parser zero-copy (`nitro-xml-opc` DOM parse path)

Same-machine A/B of origin/main (0648541) against this branch, both built with `scripts/build-wasm.sh`;
`ts-vs-wasm.mjs --runs=21`, `peak-mem.mjs --runs=5`. The TS column is that run's own TS median.

| Fixture | TS ms | WASM before | WASM after | ratio after (CI) | TS peak MiB | WASM peak before | WASM peak after |
|---|--:|--:|--:|--:|--:|--:|--:|
| xlsx S | 4.30 | 4.61 | 3.53 | 0.820 ([0.660, 0.967]) | 2.9 | 45.0 | 44.5 |
| xlsx M | 262.89 | 17.83 | 17.42 | 0.066 ([0.063, 0.071]) | 125.8 | 40.1 | 41.7 |
| xlsx L | 3034.27 | 5.04 | 7.46 (noise, first view only) | 0.002 ([0.002, 0.003]) | 1004.9 | 49.4 | 51.1 |
| docx S | 5.29 | 1.14 | 0.91 | 0.172 ([0.154, 0.233]) | 2.9 | 35.3 | 35.8 |
| docx M | 9.35 | 1.49 | 1.53 | 0.164 ([0.143, 0.176]) | 5.0 | 41.1 | 37.3 |
| docx L | 38.26 | 4.25 | 3.83 | 0.100 ([0.097, 0.108]) | 21.1 | 48.4 | 46.1 |
| pptx S | 16.32 | 0.45 | 0.43 | 0.026 ([0.023, 0.030]) | 5.0 | 31.2 | 33.0 |
| pptx M | 88.83 | 0.94 | 0.83 | 0.009 ([0.009, 0.010]) | 34.0 | 44.8 | 45.4 |
| pptx L | 268.93 | 3.03 | 2.63 | 0.010 ([0.010, 0.011]) | 50.9 | 49.5 | 50.1 |

Changes (all `nitro-xml-opc`): `normalize_line_endings`, `decode_entities`, `normalize_attribute_value` return
`Cow` and borrow when nothing needs rewriting; `memchr`/`memmem` for `&`, `;`, `<`, `\r` and the DTD/ENTITY scan
(was a byte loop); namespace URIs interned as `Arc<str>` per parse (one allocation per URI instead of one per
element and attribute); duplicate-attribute check compares in place (no per-attribute `(Option<String>, String)`
clone); end-tag names are no longer allocated; entity references no longer go through a `Vec`/`char.to_string()`.

Reading: docx/pptx DOM-bound opens gain 5 to 20% (docx S 1.14 to 0.91 ms, L 4.25 to 3.83 ms; pptx M/L about -10%);
xlsx S moves from tie/loss to a win in this run (CI upper bound 0.967) but the CI is wide, so treat it as a tie
until a longer run. Peak memory is unchanged within noise: it is dominated by the wasm compile floor (see
"Memory floor"), not by XML allocation. Still losing: peak memory on xlsx S, docx S/M/L, pptx S (and docx M time
in `peak-mem`'s cold single-open measure).

## Slice 3: zip/inflate (`perf/rust-zip-inflate`)

`ZipPackage::read` inflated into a 64 KiB chunk buffer, then copied every chunk into the output `Vec`
(zeroed chunk buffer allocated per call, one extra memcpy of every byte). Now it inflates straight into the
output buffer: parts up to 256 KiB use `read_to_end` into exact capacity (no zeroing pass); larger parts read
into one zeroed exact-size buffer (calloc, no growth/realloc). Parts over 64 MiB keep the incremental path so
a lying header cannot commit memory up front. All size/ratio/total checks are unchanged; the zip crate still
verifies CRC32. `stream_part` is unchanged.

Native `read` of every part (best of 30, same box, A/B alternated): pptx L 2.05 to 1.62 ms (-20%); docx L,
xlsx M, xlsx L within noise (+/-3%).

Inflate crate evaluation: `deflate` (miniz_oxide) vs `deflate-flate2-zlib-rs` measured natively on docx L,
pptx L, xlsx M: equal within 5%. zlib-rs stays: `nitro-xml-render` also depends on flate2 with zlib-rs, so
swapping only the zip crate would not remove it from the wasm; no size or time win.

End to end (WASM open, `--runs=21`) and peak memory are unchanged within noise: the copy removed is a
small share of an open, and the memory floor is the wasm compile (see "Memory floor"). wasm size +0.03%.

| Fixture | TS ms | WASM before | WASM after | ratio after (CI) | TS peak MiB | WASM peak before | WASM peak after |
|---|--:|--:|--:|--:|--:|--:|--:|
| xlsx S | 4.26 | 4.09 | 4.53 | 0.966 ([0.854, 1.051]) | 2.9 | 44.8 | 41.3 |
| xlsx M | 237.96 | 17.49 | 14.94 | 0.070 ([0.067, 0.072]) | 124.6 | 39.7 | 43.6 |
| xlsx L | 2534.50 | 4.44 | 4.43 | 0.002 ([0.002, 0.002]) | 1004.9 | 55.0 | 52.6 |
| docx S | 3.14 | 1.00 | 1.19 | 0.375 ([0.321, 0.473]) | 2.7 | 36.1 | 33.4 |
| docx M | 7.93 | 1.49 | 1.53 | 0.183 ([0.171, 0.204]) | 4.7 | 35.5 | 34.9 |
| docx L | 30.76 | 3.82 | 4.03 | 0.127 ([0.121, 0.134]) | 21.2 | 44.4 | 48.0 |
| pptx S | 14.73 | 0.47 | 0.49 | 0.033 ([0.029, 0.037]) | 4.1 | 31.5 | 30.7 |
| pptx M | 67.45 | 0.87 | 0.94 | 0.014 ([0.013, 0.015]) | 32.7 | 42.9 | 45.0 |
| pptx L | 246.65 | 2.73 | 2.82 | 0.012 ([0.010, 0.012]) | 51.8 | 47.1 | 48.8 |

Before/after were taken minutes apart on a shared box; differences of this size are noise. Still losing:
xlsx S time (tie, CI spans 1.0) and peak memory on xlsx S, docx S/M/L, pptx S/M (wasm compile floor).

## Slice 4: peak memory (`perf/rust-tables-alloc`)

Finding: the 30 to 45 MiB "compile floor" is mostly **V8 TurboFan tier-up of a few giant wasm functions**, not Rust
allocation (linear memory is 2.3 to 3.4 MiB after an open). `node --liftoff-only` cuts first-open growth on xlsx S
from 41 to 17 MiB; `--trace-wasm-compilation-times` lists the functions (xlsx S: 54 tiered, 132 to 205 MiB of
cumulative compiler zone). With `wasm-opt -O3` every single-caller function is inlined into its caller, so
`process_frame` became one 139 KB body (52 MiB of compiler zone alone) and `load_sheet` 31 KB.

| Change | Effect |
|---|---|
| `wasm-opt --one-caller-inline-max-function-size=1500` (still `-O3`) | main lever: 9 to 22 MiB less peak on every fixture; brotli 1,660,308 to 1,642,845 B |
| xlsx wasm state no longer fills the write-only `workbook.active_worksheet` (2 full-sheet clones per open/sheet switch/tail load removed) | less copy work and one fewer sheet copy resident |
| `cf_at_cached`: rule engine out of line (`cf_apply_rules`), bbox build out of line | per-cell hot function is a few instructions |
| `compile_format`: cache-miss compile out of line | per-cell hot function no longer drags a 9 KB body |

Tried and dropped: zip/flate2 on miniz_oxide instead of zlib-rs (inflate body 12 KB vs 14 KB, no memory change,
slower compress); `-ocimfs` 300/600/3000 (same as 1500 within noise).

Method: `peak-mem.mjs --runs=9`, same box, base build vs this branch (`/tmp` copies of `scripts/build-wasm.sh` output).
Time: `ts-vs-wasm.mjs --runs=21`.

| Fixture | TS ms | WASM ms before | WASM ms after | ratio after (CI) | TS peak MiB | WASM peak before | WASM peak after |
|---|--:|--:|--:|--:|--:|--:|--:|
| xlsx S | 3.86 | 3.86 | 3.57 | 0.924 ([0.753, 1.061]) | 2.9 | 44.2 | 29.8 |
| xlsx M | 219.75 | 16.97 | 15.51 | 0.071 ([0.067, 0.075]) | 124.6 | 42.1 | 26.5 |
| xlsx L | 2708.75 | 4.81 | 3.99 | 0.001 ([0.001, 0.002]) | 1004.4 | 52.7 | 39.1 |
| docx S | 4.69 | 1.23 | 0.78 | 0.166 ([0.141, 0.216]) | 2.7 | 33.3 | 21.7 |
| docx M | 8.08 | 2.76 | 1.20 | 0.149 ([0.139, 0.164]) | 4.9 | 37.6 | 24.9 |
| docx L | 34.03 | 4.65 | 3.94 | 0.116 ([0.103, 0.131]) | 20.8 | 47.7 | 26.0 |
| pptx S | 16.04 | 0.60 | 0.42 | 0.026 ([0.021, 0.032]) | 4.8 | 32.0 | 21.2 |
| pptx M | 77.32 | 1.04 | 0.85 | 0.011 ([0.010, 0.012]) | 33.9 | 42.4 | 30.0 |
| pptx L | 268.92 | 2.41 | 2.65 | 0.010 ([0.009, 0.010]) | 49.6 | 47.8 | 32.6 |

Before-time runs were taken minutes apart on a shared box; TS medians differ between the two runs, so compare
ratios. Still losing: **peak memory on xlsx S, docx S/M/L, pptx S** (TS 2 to 5 MiB vs a 13 MiB `initSync` floor plus
compile of touched code; docx L 1.25x, pptx M/L now win), and xlsx S time is a statistical tie (CI spans 1.0).
Next levers: `process_frame`/`load_sheet` body size (grid-paint lineage), `parse_normalized` (16 KB) and inflate
(14 KB) are tiered on every format (about 21 MiB of compiler zone), and the 13 MiB module-instantiate floor.

## Slice 5: format-split open/paint (`perf-docx-open-split`)

Finding: `Handle::open_document` and `process_frame` were single wasm functions containing all three
format paths. On a docx open, V8 TurboFan still tiered the xlsx/pptx bodies reachable from that
shared function (same compile-floor mechanism as slice 4).

| Change | Effect |
|---|---|
| `#[inline(never)]` `open_{xlsx,pptx,docx}_document` + thin `openDocument` dispatcher | docx open no longer compiles xlsx/pptx open paths |
| `#[inline(never)]` `process_frame_{xlsx,pptx,docx}` + thin `process_frame` | same for first paint / prepareFrame |

wasm-opt flags unchanged (`-O3` + `--one-caller-inline-max-function-size=1500`). Method:
`peak-mem.mjs --runs=9 --format=docx`, same box, main (`b48e0e2` / #352) vs this branch.

| Fixture | TS ms | WASM ms before | WASM ms after | TS peak MiB | WASM peak before | WASM peak after |
|---|--:|--:|--:|--:|--:|--:|
| docx S | 16.6 / 16.8 | 22.91 | **21.00** | 2.6 | 14.8 | **13.1** |
| docx M | 28.1 / 27.7 | 23.67 | **23.12** | 6.6 / 6.7 | 15.4 | **14.5** |
| docx L | 77.7 / 78.8 | 33.12 | **30.61** | 21.9 / 22.8 | 20.5 | **19.7** |

Peak drops ~0.8 to 1.7 MiB per tier; cold open time also down slightly. Still LOSS vs TS on S/M
(module instantiate + remaining shared compile: parse_normalized / inflate). L stays WIN.

## Slice 6: live-edit stale scan (xlsx editor, Rust side)

Every `setCellInput` value edit re-ran `recompute_stale`, which cloned every formula `String` and re-ran the
regex reference parser over every formula in the workbook, then did a linear scan of all dirty precedents per
reference. Now: formulas are borrowed (no clones), parsed references are cached per distinct formula text
(`RefCache`, trimmed when it grows past 2x live formulas), and the precedent lookup is a `BTreeSet` range query
on (sheet, row) instead of a full scan. Results are identical (stale sets unchanged; xlsx crate tests pass).

`cargo run --release -p nitro-xml-xlsx --example xlsx_edit_bench <fixture> 2000 200` (native, per-edit
`apply_raw("setCellInput")`, 2000 `=SUM(Bn:En)` formulas, 200 edits):

| Metric | Before | After |
|---|--:|--:|
| median per edit | 8.89 ms | 1.71 ms (-81%) |
| p95 per edit | 10.30 ms | 2.00 ms |

Open path, binary and peak memory are untouched by this slice (editor code only, no change to open-time
allocation), so the per-fixture open table above is unchanged. Remaining losses are as in slice 4: xlsx S time
(tie) and peak memory on xlsx S and docx/pptx small fixtures (wasm compile floor).


## Slice 7: docx peak_mem — compile-floor attribution (2026-10-21)

Same method as the xlsx S attribution: `peak-mem.mjs` scores **post-`initSync` VmHWM growth**.
Docx S/M (and often L) still **LOSS** vs TS after [#352](https://github.com/jgridifier/nitro-xml/pull/352).
Format-split [#359](https://github.com/jgridifier/nitro-xml/pull/359) (undrafted) shrinks first-open compile a
bit by not TurboFan-compiling xlsx/pptx bodies during a docx open; it does not remove the floor.

Stage HWM (median of 9 fresh processes; tip `fd11a8c` main vs #359 tip `165a1e7`):

| Tier | initSync Δ MiB | first open Δ (main) | first open Δ (#359) | second open Δ (main) | second open Δ (#359) |
|---|--:|--:|--:|--:|--:|
| S | 17.8 | 15.1 | **13.3** | 0.27 | 0.48 |
| M | 17.8 | 16.7 | **15.7** | 0.30 | 0.41 |
| L | 17.8 | 25.2 | 25.1 | 7.13 | 6.50 |

Gate-shaped `peak-mem.mjs --runs=9 --format=docx` (same box):

| Tier | TS peak MiB | WASM peak main | WASM peak #359 | Second-open Δ (doc alloc) |
|---|--:|--:|--:|--:|
| S | 2.3–2.6 | 13.7 | 13.5 | ~0.3–0.5 |
| M | 6.1–6.3 | 14.3 | 14.3 | ~0.3–0.4 |
| L | 21.4–21.8 | 19.9 | **18.9** (WIN vs TS) | ~6.5–7.1 |

Reading: on S/M, **warm second open (~0.3–0.5 MiB) already beats TS peak** — retained docx heap is not the
LOSS. Almost all of the gate cell is first-touch V8 compile (+ the ~18 MiB `initSync` module floor outside
the post-init baseline). Application cuts tried after #359 (parse_normalized arm split, conditional
`Option` clears, skip empty chart/picture collect, paint `#[inline(never)]`, skip styles on first-view)
did not move interleaved n=25 medians beyond noise.

Remaining levers: [#359](https://github.com/jgridifier/nitro-xml/pull/359) land; draft [#332](https://github.com/jgridifier/nitro-xml/pull/332)
`wasm-opt -O1` only if it shows **no** open/edit regression vs main; further format-boundary splits of
shared monoliths that still run on docx open. Do not raise `perf/budgets.json` or demote fixtures.

## Slice 8 (rejected): throwaway open+free does not cut whole-process peak

Draft [#364](https://github.com/jgridifier/nitro-xml/pull/364) tried a throwaway open+free after
`initSync` (and matching worker stub opens) so first-touch TurboFan sat outside the post-init
VmHWM delta. That moves the compile/alloc peak out of the measured window rather than removing
it, and adds React-embed startup work.

Honest re-measure (VmHWM growth from **process start**, n=50; React-embed mount→paint + TTI
n=50): whole-process peak **rises** with prewarm (xlsx S WASM **37.1 → 38.8 MiB** p50) and
startup **slows** (xlsx S mount **129.6 → 159.0 ms**, TTI **190.2 → 220.1 ms** p50). PR closed;
code reverted. Confirms xlsx S (and the same pattern for small fixtures) as the **wasm compile
floor** — see `docs/M1-REPORT.md` §5.4. Full tables live in the closed PR body and that report
section.

## Slice 9: docx `take_text` for one-shot XML parts

`read_package_with_opts` used `Package::text`, which keeps inflated XML in `text_cache` for the
life of the open. Now `take_text` for `word/document.xml`, `styles.xml`, `theme1.xml`,
`numbering.xml`, `settings.xml`, and comments parts so the raw String drops after parse (same
pattern as xlsx `sheetData` take).

Gate fixtures (generated pages_*) have tiny XML (~2–47 KB document) so peak is still dominated by
the wasm compile floor. On a large real fixture the retained XML was measurable:

| Fixture | WASM open ms before | after | WASM peak MiB before | after |
|---|--:|--:|--:|--:|
| docx S (`pages_10`) | 20.11 | 21.52 | 12.98 | **12.09** |
| docx M (`pages_100`) | 23.07 | 23.00 | 14.74 | 14.67 |
| docx L (`pages_500`) | 30.01 | 31.09 | 19.15 | 19.67 |
| `poi-bug65649.docx` (~12.6 MB document + 2.5 MB styles) | 1954 | **1895** | 147.8 | **135.3** (−12.6) |

Method: interleaved fresh-process `peak-mem.mjs --child=wasm:…` (n=25 generated / n=11 real), main
`3422fbf` vs this branch. No wasm-opt / budget / fixture changes.

## Slice 10: docx non-picture drawing outline + notes `take_text`

Gate `pages_*` always parse a picture (`w:drawing` + `a:blip`) but never chart/SmartArt/text-box.
`parse_modern_drawing` returned the picture early inside one fat function, so `wasm-opt -O3`
one-caller inlining still TurboFan-compiled `parse_diagram_graphic` / `parse_chart_graphic` /
text-box collect on first image open (#373 shape for xlsx).

Change: thin picture path stays in `parse_modern_drawing`; non-picture body moves to
`#[inline(never)] parse_non_picture_modern_drawing`; `#[inline(never)]` on
`parse_diagram_graphic`, `text_box_paragraphs`, and `chart_host::parse_chart_graphic`.
Also `take_text` for `word/footnotes.xml` / `word/endnotes.xml` (leftover dual-retain after
slice 9; gate fixtures have no notes parts).

Interleaved A/B vs main tip (`baeefcd` pkg), `peak-mem.mjs` wasm child, **n=21**, B=2000:

| Fixture | peak before | after | ratio [CI] | open verdict |
|---|--:|--:|--:|---|
| docx S | 12.75 | 12.72 | 0.998 [0.966, 1.053] noise | open noise |
| docx M | 15.39 | **14.66** | **0.953 [0.920, 0.998] WIN** | open noise |
| docx L | 20.62 | 19.67 | 0.954 [0.920, 1.035] noise | open noise |
| xlsx S (no-reg) | 16.39 | 16.39 | 1.000 [0.989, 1.009] | open noise |

docx S/M remain **LOSS vs TS** (compile floor). No budgets / fixtures / `#332` / `#113`.

## Slice 11: pptx `take_text` for one-shot XML parts

`Presentation::open` / first-slide parse used `Package::text`, retaining inflated XML in
`text_cache` alongside Document trees in `master_xml`/`layout_xml` and the Slide model.
Now `take_text` for presentation/tableStyles/theme/masters/layouts/slides/notes (and
chart/ink one-shots) so the raw String drops after parse — sibling of docx #369.
Also stop re-reading `ppt/presentation.xml` for slide paths.

Median(branch)/median(main) with 95% unpaired bootstrap CI (B=2000). Verdict REGRESS
only if CI entirely > 1. Point-estimate bumps on pptx S peak and `poi-bug64693` open/peak
are **noise** (CI includes 1).

| Fixture | Warm open main→branch p50 | ratio [95% CI] | Peak main→branch p50 | ratio [95% CI] |
|---|--:|---|--:|---|
| pptx S (`slides_10`) | 0.565→0.573 | 1.013 [0.956, 1.078] | 5.066→5.072 | 1.001 [0.993, 1.009] |
| pptx M (`slides_50`) | 0.965→0.958 | 0.993 [0.935, 1.080] | 10.756→**6.934** | **0.645 [0.598, 0.669]** WIN |
| pptx L (`slides_200`) | 1.620→1.487 | 0.918 [0.874, 1.002] | 18.238→18.129 | 0.994 [0.955, 1.018] |
| `poi-bug64693.pptx` | 9.521→9.713 | 1.020 [0.971, 1.059] | 21.756→21.150 | 0.972 [0.950, 1.011] |
| `poi-customGeo.pptx` | 4.973→4.920 | 0.989 [0.962, 1.002] | 20.633→21.256 | 1.030 [0.971, 1.067] |

Method: warm open = fresh process per pkg, 3 warmup + 21 runs, median of 21 processes
interleaved; peak = `peak-mem.mjs --child=wasm:…` n=50 interleaved vs main `68260ca`.
No wasm-opt / budget / fixture changes. Zero REGRESS rows.

## Slice 12: xlsx formula-bar chrome `hf` (issue #30)

On xlsx keystrokes and `setCellInput`, when only the `.xfb` formula-bar HTML changes, WASM now emits
short key `hf` (~1–2 KB) instead of full `h` (~54 KB ribbon+shell). Loader patches `.xfb` in place
(`lazy/chrome-patch.js` on tick/mut-frame and input-proxy); eager `element.js` wires
`doc._chromeEl`. Follow-up to omit-unchanged `h` (#357).

React-embed `edit-cell` interleaved (n=40): xlsx S ~11→8.5 ms, L ~12→10.8 ms.

## Slice 14: xlsx/L edit-cell tail — sticky `_nxPainted` (issue #30)

### Profile (from Slice 13)

~90 ms session-p50 outliers: host rAF `tick` raced mut-frame — either took a
concurrent `prepareFrame` during flush, or cleared `_nxPainted` so the caller's
`await tick()` rebuilt `CellIndex` over the full post-`ensure_edit` grid
(~150–250 ms). Not GC / `memory.grow`. #386's `cell_pos` + `drop_lazy` removed
outliers but **+0.6 MiB edit heap** — rejected.

### Fix (loader + Rust, heap-neutral)

**Loader** (`packages/nitro-xml/loader/lazy/wp.js`):
1. Concurrent ticks **await** the in-flight edit flush (`_efDone`).
2. Spreadsheet `_nxPainted` stays set across empty ticks until pending
   scroll/resize/pointer events (`_events.length`).

**Rust** (#386, without `drop_lazy_view` on `ensure_edit`):
- Maintained `cell_pos` on viewer + host sheets; paint prefers it over
  `CellIndex::build` on every frame; O(1) `used_range` via index bbox.
- `sync_lazy_sheet` checks `covers_viewport` before cloning the sheet.
- While `edit.is_some()`, `paint` skips `sync_lazy_sheet` (lazy inert; edit grid paints).

### A/B (React-embed `edit-cell` S+L, n=21×21 interleaved, main `0f2f0db` vs tip `28caa84`)

| Tier | main p50/p95/p99/max ms | branch p50/p95/p99/max ms | p50 ratio [95% CI] | latency | outlier sessions (p50≥40ms) | heap main→branch MiB | heap |
|---|--:|--:|---|---|--:|--:|---|
| S | 9.4 / 17.1 / 62.8 / 102 | 9.4 / 17.7 / 64.2 / 86 | 1.000 [0.969, 1.043] | NOISE | 0→0 | 7.79→7.49 | NOISE |
| L | 12.3 / 20.3 / 896 / 1262 | 12.2 / 18.8 / 924 / 1262 | 0.992 [0.917, 1.025] | NOISE | **6→0** | 16.67→16.76 | NOISE [0.990, 1.039] |

**Win (session medians):** L ~90 ms session medians gone; p50/p95 flat; edit heap not REGRESS (no `drop_lazy`).

**Not a per-edit p99/max win:** with 21 samples/session, session p99 ≈ sample 0. Every session’s first `setCellInput` still runs cold `ensure_edit` → `XlsxHost::open` (`load_workbook` + `parse_all_worksheets` + `ensure_dump_cells` on 1M cells) + `sync_model_from_edit` clone ≈ **0.9–1.3 s**; later samples are ~12–20 ms (`mut-frame` ~6–10 ms, sticky `_nxPainted` tick ~0 ms). Outlier-session count (session **median** ≥ 40 ms) therefore dropped 6→0 while reported p99/max stayed ~900/1260. Soft-share / reuse viewer grid on `ensure_edit` (no second full parse) is the follow-up that can move p99/max; #389 stays draft until that lands with 95% CI.

## Slice 15: xlsx/L edit-cell cold path — lazy soft-share (`ensure_edit`)

**Goal:** first `setCellInput` on large xlsx should not re-run `XlsxHost::open` (second zip parse + `parse_all_worksheets` + `ensure_dump_cells`) when the viewer still holds [`FirstView`](rs/crates/nitro-xml-xlsx/src/lazy/mod.rs).

**Rust:** `FirstView::materialize_worksheets_for_edit` streams each worksheet via keep-all mode in [`lazy/sheet.rs`](rs/crates/nitro-xml-xlsx/src/lazy/sheet.rs), resolves the full SST once, then `install_materialized_for_edit` (viewer grid, no `sync_model_from_edit`) + `XlsxHost::from_parts` with a single host-side sheet clone. Lazy stays alive (`drop_lazy_view` not called on edit).

**Loader:** editable open runs `setReadOnly(false)` + `loadEditorChrome` **before** `prepareFrame` / `postMessage` on the first take-frame (`frame-build.js`). Posting paint first let `waitFirstPaint` return while soft-share was still running, so sample0 raced the materialise; unlock-before-post moves that cost into the first tick (embed first-paint), leaving sample0 warm. Peak-mem / wasm open path unchanged.

**A/B** (vs #389 tip `d7c1a54`, React-embed `edit-cell` L, n=21×21, 95% bootstrap CI of median ratios):

| Metric | main (#389) | soft-share | ratio CI | Verdict |
|---|---:|---:|---|---|
| sample0 (first edit) | 1122.7 ms | 45.0 ms | [0.038, 0.043] | **WIN** |
| session p50 | 12.4 ms | 11.8 ms | [0.914, 0.983] | WIN |
| session p95 | 17.3 ms | 17.6 ms | — | flat |
| edit heap L | 34.16 MiB | 34.07 MiB | [0.991, 0.999] | WIN (≈flat) |
| peak-mem open L | 32.1 ms | 29.5 ms | [0.855, 1.042] | NOISE |
| peak-mem peak L | 28.0 MiB | 25.0 MiB | [0.803, 1.050] | NOISE |

Materialise (~0.9 s soft-share) now sits in the first editable `take-frame` (embed first-paint / open tick), not in timed sample0. Peak-mem wasm open path unchanged.

## Slice 16: xlsx/L edit-cell — host owns grid (soft-share heap)

**Goal:** drop the second full sheet clone on lazy `ensure_edit` (viewer grid + `XlsxHost::from_parts` clone) so edit-session peak heap falls toward one materialised grid.

**Rust (stub):** `materialize_worksheets_for_edit` → move sheets into `XlsxHost::from_parts`; viewer `sheets` cleared; paint / selection / nudge read through `edit.host().sheets` while `host_owns_edit_grid`; `sync_cells_from_edit` no-ops cell patches on that path; fallback `XlsxHost::open` keeps `sync_model_from_edit`.

**Measure:** React-embed `edit-cell` L peak heap + sample0 after land; parity unchanged.

