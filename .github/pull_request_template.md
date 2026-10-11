Closes #

## What changed
<!-- One or two sentences. Keep the PR to one issue. -->

## Scope check
- [ ] Only files listed in the issue's "Files and modules" were changed (or the PR explains why)
- [ ] Nothing under `.github/workflows/` was edited
- [ ] New files have SPDX headers (`MIT`, or `Apache-2.0` plus provenance line and a PROVENANCE.md row for ported files)
- [ ] No fixture of unknown licence added
- [ ] Testing rule (AGENTS.md §0): bugs fixed here are covered by a new corpus file; any new unit test starts with a line naming the bug it catches; no coverage-only or code-restating tests; no wholesale port of upstream tests

## Gates (paste summaries)
- [ ] `scripts/ci/rust.sh` / `wasm.sh`
- [ ] `scripts/ci/parity.sh`
- [ ] `scripts/ci/perf.sh gate` (and `perf_beats_ts.sh <format>` once that format's gate is required)
- [ ] `scripts/ci/bundle_budget.sh`
- [ ] Other gates named in the issue:

## Stub lines removed / required checks to add
<!-- If this PR implements a gate: which `NOT IMPLEMENTED — see #N` line was deleted, and which job Jared should add to required checks. -->

## Handoff
I posted the handoff comment on the issue (what changed, what's left, gotchas).
