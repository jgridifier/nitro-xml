# Blind review #11

Scored 1-5 before opening key.sealed.json (reviewer: orchestrator, 2026-10-11).

| Label | Correctness | Scope | Licence headers | Tests (non-obvious, Catches line) | Notes |
|---|---|---|---|---|---|
| X | 5 | 4 | 5 | 4 | Own bounded raw-inflate loop over flate2 next to the zip crate: second code path to maintain; proptest dev-dep; 725-line diff. Limits checked at open and while inflating. |
| Y | 4 | 5 | 5 | 4 | Leanest: zip crate only, lazy per-part read with chunked byte budget; `is_over_declared` matches the zip crate's error text (brittle across zip upgrades; pin or replace later). Two targeted tests with Catches lines (hand-built stored zips). |
| Z | 1 | 1 | n/a | 0 | Gate never passed: no zip_dump example after 2 review rounds; only Cargo.toml touched. |

Pick: **Y** (passes the gate, smallest surface, no extra inflate path). Follow-up on merge: replace the error-string match with the declared-size `take` guard alone.
