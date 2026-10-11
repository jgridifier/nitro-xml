# AGENTS.md: coordination contract

This file is the contract for every coding agent (Cursor, Claude Code, Codex, OpenCode, and humans) working on Nitro XML in parallel. `CLAUDE.md` and `.cursor/rules/` point here. If anything here conflicts with an issue, this file wins; if it conflicts with **[docs/PRODUCT_SPEC.md](docs/PRODUCT_SPEC.md)** (the source of every parity, performance, bundle, accessibility and editor gate), the spec wins.

Board: https://github.com/users/jgridifier/projects/2 · Plan: https://jgridifier.github.io/nitro-xml/

## 1. Pick work
1. Take an **unassigned** issue labelled `status:ready` (board view **Ready now**) whose `Depends on #X` issues are **all closed**.
2. Never take an issue labelled `blocked-m2` (editor milestone, waits for the viewer gate and Jared's go-ahead) or one assigned to Jared (design decisions such as the R0 renderer record #33). Renderer issues are not claimable until #33 is merged.
3. Prefer the issue's *Suggested agent*, but any agent may take any ready issue. Agent types that may claim: **Cursor, Claude Code, Codex and OpenCode** (OpenCode runs free NVIDIA-hosted models), plus human Developers. The protocol below is identical for all of them.

## 2. Claim it
- Comment exactly `claiming` on the issue (add your agent type, e.g. `claiming (opencode)`), then **assign yourself** or add the matching label `agent:<name>`: `agent:cursor`, `agent:claude-code`, `agent:codex` or `agent:opencode`. A new agent type gets a new `agent:<name>` label; nothing else changes.
- Move the board item to **In progress**. If someone claimed it first (earlier comment or label), pick another issue.
- One agent per issue. If you stop, comment `unclaiming` with your handoff notes and remove your label.

## 3. Branch and PR
- One branch per issue: `issue-<N>-<short-slug>` (e.g. `issue-11-zip-reader`).
- Keep each PR small and focused on that one issue. The PR body contains `Closes #N`.
- Use the PR template. Move the board item to **In review** when the PR is open.

## 4. Before you push
- Run the tests and the gates locally: `bash scripts/ci/rust.sh`, `bash scripts/ci/wasm.sh`, `bash scripts/ci/parity.sh`, `bash scripts/ci/perf.sh gate`, plus any gate script your issue names (`perf_beats_ts.sh <format>`, `bundle_budget.sh`, `ship_check.sh`, `selection_parity.sh`, `a11y.sh`, `bidi.sh`, `crash_recovery.sh`, `roundtrip.sh`, `edit_parity.sh`, `undo_parity.sh`, `ime.sh`, `spellcheck.sh`, `scoreboard.sh`).
- The gates themselves (parity, visual at DPR 1 and 2 plus print-to-PDF, unsupported content at 0 per format at its stage gate, WASM-beats-TS perf, bundle and loader budgets, accessibility, IME, bidi, selection and caret, crash recovery, editor round-trip and undo parity) are defined in **[docs/PRODUCT_SPEC.md](docs/PRODUCT_SPEC.md) §5**.
- Paste the gate summary into the PR. Every commit posts its perf numbers; losses are reported as prominently as wins.
- A perf regression can only merge with the label `perf-regression-accepted` and a written reason; that label never overrides the **WASM-beats-TS** gate of a format once it is armed (from the commit where that format has a full open-to-first-paint path in Rust; never relaxed). Foundation PRs (zip, XML streamer, OPC) are tracked but not gated against TS.

## 5. Stay in scope
- **Do not touch files outside the issue's scope** ("Files and modules" in the issue). Leave a `TODO(#N)` and open or mention a follow-up issue instead.
- **Never edit anything under `.github/workflows/`.** Agent tokens cannot write workflows, and the YAML is a thin skeleton that only calls `scripts/ci/*.sh`. Change behaviour in the scripts. If you truly need a YAML change, describe it in the PR and ask Jared.
- Stub scripts print `NOT IMPLEMENTED — see #N` and exit 0. The issue that implements a gate deletes that line and, in the PR, asks for the job to be added to the branch-protection required checks (done by Jared or the Chief of Staff's gh login, not by you).
- Do not vendor upstream fixtures of unknown licence. Record source and licence for every fixture in `fixtures/manifest.json`.
- Nothing in TypeScript ships. The upstream TS engine lives only in the test and bench harness as the correctness reference and speed baseline. The only shipped JS is the tiny loader (WASM bootstrap and event forwarding), which has its own size budget.

## 6. Licence headers
- Nitro XML is MIT-licensed. New original files start with `// SPDX-License-Identifier: MIT` (or the comment syntax of the file type).
- Any file **ported or derived from upstream ooxml** keeps `SPDX-License-Identifier: Apache-2.0` plus a provenance line, e.g. `// Ported from ChristopherVR/ooxml@e66aaec src/core/opc/relationships.ts; changes listed in PROVENANCE.md.`, and gets a row in `PROVENANCE.md` in the same PR.
- Dependencies: permissive only (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode). The `licenses` job enforces it.

## 7. Finish with a handoff comment
When the PR is open (and again if anything changes before merge), post on the issue:

```
### Handoff
**What changed:** …
**What's left / follow-ups:** … (link issues)
**Gotchas:** … (surprises, flaky tests, upstream quirks, perf caveats)
**Gate results:** parity … · perf … · bundle … · other …
```

## 8. Useful facts
- Upstream reference: ChristopherVR/ooxml pinned at `e66aaec24adc0b68472eae1fc42aa4598e09c352`.
- Rust crates: `nitro-xml-opc` (zip, XML, OPC, DrawingML), `nitro-xml-xlsx`, `nitro-xml-docx`, `nitro-xml-pptx`, `nitro-xml-wasm`, umbrella `nitro-xml`. npm package: `nitro-xml`.
- Paths in issues are upstream paths unless prefixed with `rs/`, `perf/`, `packages/`, `fixtures/` or `scripts/`.
