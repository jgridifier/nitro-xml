# Issue #10 evidence subset (sanitized)

Why this exists: the raw L/XL sweep output lives in gitignored `perf/results/lxl-sweep/` and the operator events in a private
Claude session transcript. This directory is the minimal, committed part of that evidence needed to check every status in
`perf/baselines/failure-provenance.json` and the numbers in `perf/baselines/ISSUE-10-AUDIT.md`. The originals are untouched.

## Contents

| path | what | how it relates to the original |
| --- | --- | --- |
| `MANIFEST.json` | sha256 + size of each file, its source path, and the sha256 of that source | |
| `cell-evidence.json` | one row per classified cell (272): status claim and the evidence locations in this directory | derived from `failure-provenance.json` |
| `raw/sweep.log`, `raw/box-conditions.jsonl`, `raw/ts-lxl-8961e71.failures.jsonl`, `raw/ts-4451709.failures.jsonl`, `raw/ts-4451709.rerun-failures.jsonl` | full small files | byte-identical copies (same line numbers) |
| `extracts/<chunk>.json` (8) | chunk result files that list failures | `meta` + `failures[]` verbatim; the large per-iteration arrays (`results[]`) are dropped and replaced by an index of fixture ids |
| `extracts/warm-1x-001.gen-pptx-slides-200-b.row.jsonl` | the single sample row of the partially measured cell | one line verbatim (`source_lines` in the manifest) |
| `extracts/markers.json` | the six 0-byte marker files that make the sweep skip a chunk, with source mtimes | listing only |
| `history-lxl-pre-host-fix.jsonl` | the 42 L/XL lines of `perf/history.jsonl` exactly as first appended (importing machine's host), before the host correction | verbatim copy of lines 953-994 of the file with sha256 `3e30e7a2...edefd` (see `MANIFEST.json` `history_correction`) |
| `operator-events.jsonl` | 11 Bash commands (sweep launches, `kill`, marker creation, annotation appends) and their results, with source timestamp and transcript line | extract of the transcript; no assistant or user prose |

## Redactions and exclusions

- Only Bash commands and their tool results are extracted, and only the transcript lines listed in `MANIFEST.json`
  (`source_lines`); no conversation text, no environment dumps, no other tool calls.
- The builder redacts e-mail addresses, `/home/<user>` paths (to `~`), common token formats (GitHub, `sk-`, AWS, Slack),
  `Bearer` values and JWTs, counts them in `MANIFEST.json` (`redactions`), and refuses to write if a suspicious string
  (including `password:`/`secret=`/`api key` assignments) is still present. Count in this build: 0 of each.
- Result text is truncated to 1500 characters per event. Host id `box-4467396174` is the hashed host id already published in `BASELINE.md`.

## Verify (no private files needed)

```
node perf/verify-issue10-evidence.mjs            # hashes, evidence present for every cell, statuses re-derived, totals equal failure-provenance.json
node perf/verify-issue10-evidence.mjs --sources  # additionally compares the recorded source sha256 with the originals when they exist locally
```

Statuses are re-derived from the evidence, not copied: observed_timeout/observed_error from the harness error text in a chunk
extract or raw failure row; operator_aborted when the fixture has a start sample in `box-conditions.jsonl`, its chunk line in
`sweep.log` says `exit null` and an operator kill event exists; not_run when there is no start sample and the chunk was either
killed earlier (`exit null`) or never started (no `sweep.log` line, marker present).

Rebuild (needs the originals): `node perf/build-issue10-evidence.mjs`, then `node perf/verify-issue10-evidence.mjs`.

## Limitations

- **Thin samples.** The L/XL sweep ran 3 iterations per cell (launch command in `operator-events.jsonl`, transcript L732, `--iterations 3`); every measured L/XL cell is below the harness design count (cold 10, warm 20).
- **Not-run and operator-aborted cells are not measurements.** 19 L/XL cells were never started; 4 were killed by the operator
  (`kill -9`, no timeout observed, result file never written). Operator notes such as "render avg 226s" in the failures file are predictions with no raw source.
- **Host pressure is unresolved.** `box-conditions.jsonl` records load and free memory at fixture starts (load average 241 when pptx XL cold 4x started),
  but the 4x timeouts cannot be split into TS slowness vs box pressure from this data.
- **No main-branch L/XL regression baseline.** The L/XL history entries carry branch `issue-10-lxl-real`, record no fixture hash, and the gate only
  baselines main entries with an identical protocol key, so L/XL rows are `no comparable baseline` until appended from main.
- **History host correction.** The verifier puts the snapshot back into `perf/history.jsonl` and requires the pre-correction file hash, so it proves no other history line changed; it also requires each corrected host to equal what its source rows in `perf/baselines/ts-lxl-8961e71.jsonl` record. `date` and `runner` of those entries are still the importer's (not protocol fields).
- **S/M evidence** is the raw harness failure rows only (written from completed chunk results); the S/M operator transcript was not audited for kills.
- Marker mtimes show when a marker was created (e.g. `cold-4x-002.jsonl` was created at 02:07:37Z, after that chunk had already started at 02:02Z, so it skipped nothing).
