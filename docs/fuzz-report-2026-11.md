<!-- SPDX-License-Identifier: MIT -->
# Fuzz report, November 2026 (Part of #30)

Campaign for the M1 parser-hardening bar: every `cargo fuzz` target in `rs/fuzz/fuzz_targets`
for 1 CPU-hour. Status: **9 of 10 targets reached 1 CPU-hour with zero open crashes;
`docx_load` reached 1 CPU-hour only by raising the memory limit to 2 GiB for its last 2842 s, and has an open memory finding at 1 GiB** (below). Not claimed as "zero open crashes".

## Setup

- Base `a32d819` (origin/main). Nightly libFuzzer + ASAN, `--features libfuzzer`, release.
- Two fuzzers at a time, `nice -n 10`, `-rss_limit_mb=1024`, `-timeout=60`, one process per target.
- CPU time = user+sys of the fuzzer process (`wait4`), summed over slices. Wall time was longer
  because the VM was paused several times (see "Host pauses").
- Seeds: `rs/fuzz/seed/<target>` plus `fixtures/fuzz/*` (same as `scripts/ci/fuzz-smoke.sh`);
  `edit_*` embed their fixture and start from an empty corpus. `edit_ci` is not a libFuzzer
  target (CI-sized runner) and is not part of this table.
- The box has no C++ compiler; libFuzzer was built with a user-space zig toolchain (build
  environment only, nothing in the repo changes).

## Results

| Target | CPU time | Execs | Corpus (files / KiB) | Open crashes |
|--------|---------:|------:|---------------------:|-------------:|
| `zip_open` | 3600 s | 908,930 | 3,629 / 304,999 | 0 |
| `xml_parse` | 3600 s | 9,571,197 | 16,049 / 25,338 | 0 |
| `opc_open` | 3866 s | 5,296,878 | 10,780 / 91,817 | 0 |
| `docx_load` | 3604 s (762 s at 1 GiB limit, 2842 s at 2 GiB) | 395,719 | 2611 / 66196 | **open memory finding at 1 GiB; 0 other** |
| `xlsx_load` | 3622 s | 339,373 | 3,314 / 613,662 | 0 |
| `pptx_load` | 4070 s | 255,048 | 3,051 / 939,493 | 0 (1 non-reproducing OOM) |
| `safe_href` | 4100 s | 152,394,031 | 1,732 / 368 | 0 |
| `edit_xlsx` | 3603 s | 54,958 | 683 / 12 | 0 |
| `edit_docx` | 3602 s | 178,095 | 629 / 15 | 0 |
| `edit_pptx` | 3601 s | 156,310 | 1,072 / 27 | 0 |

Corpora are not checked in (`rs/fuzz/corpus/` is gitignored; they total about 2.4 GiB).
Exec counts are the sum of libFuzzer's final `number_of_executed_units` per slice.

## Open finding: docx_load memory amplification

A small, policy-legal zip makes `docx_load` allocate close to or above 1 GiB under ASAN:
a 28 KiB package whose `word/document.xml` is 21,000,181 bytes of repeated identical paragraphs
(ratio about 257x, under the 1000x cap; 150k paragraphs). The first hit was byte-identical to the
checked-in must-open fixture `fixtures/fuzz/docx/repeated-paragraphs-150k.docx`.

- Native release: 19 s and 938 MB peak before this PR, 2.3 s and 715 MB after the fix below.
  Under ASAN a fresh-process replay peaks at 861 MB to 1.5 GB depending on the build/variant, so
  4 in-fuzz OOM artifacts plus 1 timeout (replay > 10 s under load) were recorded.
- Fixed here (partial): `read_package` cloned every body block to scan headers/footers/notes
  for images and diagrams. The clone is gone (`body.rs`, `warnings.rs`).
- Not fixed: the remaining cost is the per-paragraph JSON model plus the DOM
  (about 4.7 KB per paragraph). A mutant with a larger `document.xml` stays policy-legal
  (`max_part_bytes` 200 MiB), so no loader tweak bounds this; it needs either a block/paragraph
  cap (a limit change) or a higher fuzz `-rss_limit_mb`. Both are Jared's call.
- `repeated-paragraphs-150k.docx` was left out of the `docx_load` fuzz corpus for the remaining
  run (the fixture itself is untouched and still opens in tests).
- The remaining `docx_load` CPU (2842 s) was run with `-rss_limit_mb=2048` and `-timeout=120`
  to look for non-memory crashes: none (peak RSS 1194 MB; the only artifacts were host-pause
  timeouts that replay in under 2 s). This does **not** satisfy the 1 GiB bar; it only shows
  no other crash class in that hour.

## Part of #30: memory-reduction follow-up (2026-11-12)

A second attempt was made to push `docx_load` under the 1 GiB ASAN limit without a paragraph cap,
a limit raise, or fixture demotion. All reductions were merged into `docx-load-mem` (Part of #30).

**Optimisations applied (native 150k-paragraph result)**

| Change | File | Native VmHWM |
|--------|------|-------------|
| Baseline (clone removal, M1) | `body.rs`, `warnings.rs` | 715 MB |
| Consuming parse: free XML subtrees after each paragraph/table | `body.rs` | 475 MB |
| Child-Vec starts at 1 slot, grows geometrically | `opc/xml/parse.rs` | 460 MB |
| Iterator scan: no temporary Vec per element scan | `xml_util.rs` | 455 MB |
| Block-accumulator pre-sized to `total_story` | `body.rs` | 452 MB |
| Non-field run avoids unused `"text"` key alloc | `field_runs.rs` | **435 MB** |

Measured with the custom `docx_load_mem` example (`--bin docx_load_mem`, native release):
150k-par → VmHWM 435 MB, peak_bytes 310 MB, 8.4 M allocs.
50k-par → VmHWM 143 MB, peak_bytes 98.8 MB, 2.8 M allocs.

**ASAN fuzz results (two fresh-process replays of `repeated-paragraphs-150k.docx`)**

```
run 1: peak_rss_mb=1026  live=237449731 B (2531972 chunks)  quarantine=235482563 B (1887636 chunks)
run 2: peak_rss_mb=1037  live=263221887 B (2461770 chunks)  quarantine=226928039 B (2329107 chunks)
```

Both runs exceed the 1024 MB limit by 2–13 MB. The ±10 MB variance is ASAN-quarantine
non-determinism (position in the corpus sequence changes how many freed bytes are queued).

**Root cause: 1 GiB is currently unreachable without a cap**

After consuming parse, the peak live allocation is ~240–265 MB (the JSON model).
The ASAN overhead alone (shadow memory, redzones, process baseline) adds ~550 MB, putting the
floor at ~800 MB. The gap between 1 GiB and that floor is consumed by ASAN quarantine
(~230–235 MB of freed-but-not-returned XML memory).

The quarantine is dominated by ~3 M `String` allocations for XML element local tag names (one per
node). Each small String has a 32–64-byte ASAN redzone. When `parse_container_consuming` frees
paragraph subtrees incrementally, those ~3 M Strings flow into ASAN quarantine as ~192 MB of
redzoned memory that ASAN holds until the quarantine overflows. No amount of live-allocation
reduction can shrink this — the quarantine is sized to the number of distinct heap allocations freed.

**The fix that would close this finding** is replacing `Element.local: String` with `Arc<str>` and
interning tag names at parse time. This reduces ~3 M allocations (one per element) to ~50 (one per
unique tag name, e.g. `"w:p"`, `"w:r"`, `"w:t"`, …). That saves ~192 MB of ASAN redzone/quarantine
overhead, bringing the ASAN peak to roughly 850 MB — well under 1 GiB.

That refactor touches `Element::new` in `opc/xml/tree.rs`, the parser in `opc/xml/parse.rs`, and
300+ comparison sites across `nitro-xml-opc`, `nitro-xml-docx`, `nitro-xml-xlsx`, `nitro-xml-pptx`
and `nitro-xml-render` (mostly through `named_w` and similar helpers). It is a correctness-neutral
refactor with no behaviour change, but its blast radius is too large for this PR. Opening as a
follow-up issue.

**Stop condition invoked**: the two ASAN fuzz runs (1026 MB, 1037 MB) confirm the 1 GiB bar is
unreachable without the `Arc<str>` interning refactor. All reductions that are green (native
715 MB → 435 MB; correct parse; no regressions) are kept. No paragraph/part cap was added, no fuzz
memory limit was raised, no fixture was demoted.

## Non-reproducing artifacts (not bugs)

Every artifact libFuzzer wrote was replayed standalone; the runner kept a crash only when the
replay failed or took more than 10 s.

- **Host pauses:** `timeout` artifacts on `zip_open`, `xml_parse`, `opc_open`, `xlsx_load`,
  `pptx_load`, `safe_href` (and `slow-unit` files on `opc_open`, `xlsx_load`, `pptx_load`)
  came from VM pauses: libFuzzer reported e.g. "working on the last Unit for 6459 seconds" on
  `safe_href`; each replays in 0 to 12 ms.
- **`pptx_load` OOM:** one artifact (875 KB) replays in 57 ms at 52 MB peak. Not reproducible;
  cumulative in-process growth is the likely cause. Listed, not fixed.
- **docx_load:** many OOM artifacts replayed fine (small peak); they are counted in the finding above.

## Crashes fixed

None found other than the docx finding above (no panic, abort, or ASAN error in any target).
The docx memory change is covered by the existing corpus file
`fixtures/fuzz/docx/repeated-paragraphs-150k.docx` (opened by the docx tests); no new unit test
because no smaller corpus file isolates it.

## Reproduce

```bash
cd rs/fuzz
cargo fuzz build --release --features libfuzzer
target/x86_64-unknown-linux-gnu/release/<target> corpus/<target> -rss_limit_mb=1024 -timeout=60 -max_total_time=3600
```
