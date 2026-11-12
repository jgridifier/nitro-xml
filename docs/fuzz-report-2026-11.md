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
