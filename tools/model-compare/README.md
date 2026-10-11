<!-- SPDX-License-Identifier: MIT -->
# Model comparison (#103)

Question: is **an orchestrator plus a cheap implementer** worth adopting over **a single strong model** for Nitro XML issues?

## Arms
| Arm | Agent | Models | Command (see `run.sh`) |
|---|---|---|---|
| `A-opus` | Claude Code | Opus only (the `Task` tool is disabled, so no subagents) | `claude -p --model opus --disallowedTools Task` |
| `B-opus-haiku` | Claude Code | Opus orchestrates and reviews; the `implementer` subagent (`.claude/agents/implementer.md`, `model: haiku`) writes the code | `claude -p --model opus` |
| `C-codex` | Codex CLI | `6.1 sol` only (no other Codex model may be used) | `codex exec -m "$CODEX_MODEL"` |
| `D-opencode` | OpenCode 1.19 | NVIDIA NIM `z-ai/glm-5.3` (free) as implementer | `opencode run --auto -m nvidia/z-ai/glm-5.3` |

## Winning rule (fixed before the first run, 2026-10-11)
Adopt orchestrator + cheap implementer (arm B; arm D is its free variant) **only if all three hold** over the planned issues:
1. its first-try gate pass rate is **≥** arm A's;
2. its plan usage is **noticeably lower**: at least **25 % less** than arm A's (measured as list-price-equivalent cost and total tokens reported by the CLI; for D, NIM tokens, which cost no plan usage);
3. **no extra review rounds**: its mean review rounds per issue are **≤** arm A's.
Otherwise keep the single strong model. Arm C is reported alongside but does not change the rule.

## Protocol
- Every arm starts from the **same base commit** in a **fresh worktree** (`../mc-<issue>-<arm>`, branch `mc/issue-<N>/<arm>`).
- Every arm gets the **same task prompt** (`prompts/common.md` + `prompts/issue-<N>.md`) plus only its arm-specific delegation line.
- **Arm order rotates** per issue (`run.sh` derives the start order from the issue number), so no arm always goes first against the shared plan limits.
- The **gate is fixed before the run** (`gates/issue-<N>.sh`): a differential check against the upstream TS behaviour on the real corpus plus crafted malformed inputs, the wasm32 build, fmt and clippy.
- **First-try pass** = the gate passes on the agent's first hand-back. If it fails, the gate output is sent back to the same session (a **review round**), at most 2 rounds.
- **Blind review**: `blind.sh <N>` relabels each arm's diff as `X/Y/Z/W` in random order and seals the key; the reviewer scores correctness, scope, licence headers and tests before opening the key.
- **Logged per run** (`results/runs.jsonl`): first-try pass, review rounds, lines of code added (excluding fixtures and lockfiles), tests added, rejected tests (unit tests whose first line is not `// Catches: …`, AGENTS.md §0), wall time, plan usage (tokens and cost as reported), and for OpenCode the NIM request latency and 503 count.
- Winner's branch becomes the issue PR (re-reviewed normally). Losing branches are kept for reference, not merged.

## Planned issues
| Issue | Differential gate |
|---|---|
| #11 zip reader | part names, sizes and sha256 equal upstream `readZipParts` (JSZip) on every corpus file; malformed and zip-bomb inputs rejected by both, never a panic |
| #12 XML streamer | same element/attribute/text events as upstream's lite reader on every XML part of the corpus; DOCTYPE/ENTITY rejected |
| #13 OPC | content types and relationships (Id, Type, Target, TargetMode) equal upstream on the corpus |
| #15 xlsx shared strings + styles | shared-string table and resolved cell styles equal upstream |
| #17 number formats | formatted cell text equal upstream for every (value, format) pair in the corpus |

## Usage
```
bash tools/model-compare/run.sh <arm> <issue> <base-sha>   # one arm
bash tools/model-compare/run-issue.sh <issue> [base-sha]    # all arms, rotated order
bash tools/model-compare/blind.sh <issue>
```
OpenCode needs `set -a; . /home/box/.config/nvidia-nim.env; set +a` (run.sh does it). Codex needs `CODEX_MODEL` set to the exact `6.1 sol` id; if the catalog has no such model the arm is logged as `unavailable` and not substituted.
